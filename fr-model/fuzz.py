"""Zufallstests für die Sätze 5.1 bis 5.3 des Ownership-Kernkalküls.

Aufruf:  py -X utf8 fuzz.py [COUNT] [--disable-rule RULE] [--start N] [--size N]

Für jedes zufällige Programm:
  - Typprüfung. Abgelehnte Programme werden trotzdem ausgeführt; gezählt wird,
    wie viele davon tatsächlich in einen Invalid state laufen (zeigt, dass die
    Regeln echte Fehler verhindern).
  - Angenommene Programme laufen mit Invariantenprüfung. Geprüft wird:
      Satz 5.1  kein Invalid state (Stuck) und keine verletzte Invariante
      Satz 5.3  kein BorrowError, wenn nie ein Objekt geteilt wurde
      Zusatz    ohne shared bleibt am Ende kein Objekt übrig (kein Leck)
    Jeder Verstoß wird mit Programmtext in fuzz-findings/ abgelegt.

Mit --disable-rule wird eine Regel des Typprüfers absichtlich abgeschaltet
(siehe ABSCHWAECHUNGEN). Dann sollen Verstöße auftreten: Das zeigt, dass die
Regel nötig ist und der Zufallstest sie erreicht.
"""

import argparse
import pathlib
import sys
import time
from collections import Counter

ROOT = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from frpp import ast as A  # noqa: E402
from frpp.checker import Checker, CheckError  # noqa: E402
from frpp.gen import generate  # noqa: E402
from frpp.interp import BorrowError, Interp, InvariantViolation, OutOfFuel, Stuck  # noqa: E402


class OhneLoans(Checker):
    """Loans werden nicht geprüft (erlaubt_L ist immer wahr)."""
    def require_allowed(self, *a, **k):
        pass


class OhneMoveZustand(Checker):
    """Verschobene Variablen gelten weiter als verfügbar (R-5.3 trägt ○ nicht ein)."""
    def expr(self, env, loans, e):
        t, env2 = super().expr(env, loans, e)
        return t, {k: (v[0], True) for k, v in env2.items()}


class OhneJoin(Checker):
    """R-5.15 übernimmt nur den Zustand des then-Zweigs."""
    @staticmethod
    def join(e1, e2):
        return e1


class OhneSchleifenregel(Checker):
    """R-5.16 prüft nicht, ob der Rumpf Variablen verschoben zurücklässt."""
    def stmt(self, env, loans, s):
        if isinstance(s, A.While):
            self.cond(env, loans, s.cond, "R-5.16", s.line)
            self.block(env, loans, s.body)
            return env
        return super().stmt(env, loans, s)


class OhneSharedUeberlappung(Checker):
    """Orte mit shared-Wurzel überlappen nur bei Präfix (statt immer)."""
    @staticmethod
    def overlap(env, p, q):
        return p[0] == q[0] and (p[:len(q)] == q or q[:len(p)] == p)


class OhneSchreibrecht(Checker):
    """ref-Wurzeln gelten als schreibbar."""
    @staticmethod
    def writable(env, p):
        return True


class OhneEmpfaengerLoan(Checker):
    """R-5.24 trägt für den Block keinen Loan auf den Empfänger ein."""
    def stmt(self, env, loans, s):
        if isinstance(s, A.BorrowCall):
            t, env2, m = self.call(env, loans, s.call)
            inner = dict(env2)
            inner[s.var] = (t, True)
            out = self.stmts(inner, loans, s.body)
            return {k: out[k] for k in env2}
        return super().stmt(env, loans, s)


ABSCHWAECHUNGEN = {
    "loans": OhneLoans,
    "move": OhneMoveZustand,
    "join": OhneJoin,
    "loop": OhneSchleifenregel,
    "shared-overlap": OhneSharedUeberlappung,
    "write-permission": OhneSchreibrecht,
    "receiver-loan": OhneEmpfaengerLoan,
}

LEGACY_RULE_NAMES = {
    "schleife": "loop",
    "shared-ueberlappung": "shared-overlap",
    "schreibrecht": "write-permission",
    "empfaenger-loan": "receiver-loan",
}


def canonical_rule_name(name):
    return LEGACY_RULE_NAMES.get(name, name)


def run_one(seed, checker_cls, findings_dir, size):
    prog = generate(seed, size)
    ck = checker_cls(prog)
    res = {"seed": seed}
    try:
        ck.check()
        res["accepted"] = True
    except CheckError as exc:
        res["accepted"] = False
        res["rule"] = exc.rule
    it = Interp(prog, fuel=3000, check_invariants=res["accepted"], checker=Checker(prog))
    try:
        it.run()
        res["outcome"] = "ok"
    except BorrowError:
        res["outcome"] = "BorrowError"
    except OutOfFuel:
        res["outcome"] = "Budget"
    except (Stuck, InvariantViolation) as exc:
        res["outcome"] = "Invalid state"
        res["detail"] = f"{type(exc).__name__}: {exc}"
    except RecursionError:
        res["outcome"] = "Budget"
    except Exception as exc:  # noqa: BLE001 - Fehler im Modell selbst
        res["outcome"] = "Model error"
        res["detail"] = f"{type(exc).__name__}: {exc}"
    res["stats"] = it.stats
    if res["accepted"]:
        problem = None
        if res["outcome"] in ("Invalid state", "Model error"):
            problem = "Theorem 5.1: " + res["detail"]
        elif res["outcome"] == "BorrowError" and not it.shared_used:
            problem = "Theorem 5.3: BorrowError without a shared object"
        elif res["outcome"] == "ok" and not it.shared_used and it.live_objects():
            problem = f"Leak without shared: {it.live_objects()}"
        res["problem"] = problem
        if problem and findings_dir is not None:
            findings_dir.mkdir(exist_ok=True)
            (findings_dir / f"seed{seed}.fr").write_text(
                f"// {problem}\n" + A.program_str(prog), encoding="utf-8")
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("count", type=int, nargs="?", default=2000)
    ap.add_argument("--start", type=int, default=0)
    ap.add_argument("--size", type=float, default=1.0)
    ap.add_argument("--groesse", dest="size", type=float, help=argparse.SUPPRESS)
    ap.add_argument("--disable-rule", type=canonical_rule_name, choices=sorted(ABSCHWAECHUNGEN))
    ap.add_argument("--abschwaechen", dest="disable_rule", type=canonical_rule_name,
                    choices=sorted(ABSCHWAECHUNGEN), help=argparse.SUPPRESS)
    args = ap.parse_args()
    sys.setrecursionlimit(5000)

    checker_cls = ABSCHWAECHUNGEN.get(args.disable_rule, Checker)
    findings = None if args.disable_rule else ROOT / "fuzz-findings"
    t0 = time.time()
    acc_out, rej_rule, rej_crash, problems, stats = Counter(), Counter(), Counter(), [], Counter()
    for seed in range(args.start, args.start + args.count):
        r = run_one(seed, checker_cls, findings, args.size)
        if r["accepted"]:
            acc_out[r["outcome"]] += 1
            stats.update(r["stats"])
            if r["problem"]:
                problems.append((seed, r["problem"]))
        else:
            rej_rule[r["rule"]] += 1
            if r["outcome"] == "Invalid state":
                rej_crash[r["rule"]] += 1

    n_acc = sum(acc_out.values())
    print(f"Programs: {args.count} (Seeds {args.start}..{args.start + args.count - 1}), "
          f"Type checker: {args.disable_rule and 'without ' + args.disable_rule or 'all rules enabled'}, "
          f"{time.time() - t0:.0f} s")
    print(f"\nAccepted: {n_acc}")
    for k in ("ok", "BorrowError", "Budget", "Invalid state", "Model error"):
        print(f"  {k:14} {acc_out[k]}")
    print("  Coverage:   " + ", ".join(f"{k} {stats[k]}" for k in
                                         ("new", "free", "lock", "borrow_error", "upgrade_ok", "upgrade_fail",
                                          "borrow_call")))
    print(f"\nRejected: {sum(rej_rule.values())}  (Rule: rejected / invalid at runtime)")
    for rule, n in sorted(rej_rule.items(), key=lambda kv: -kv[1]):
        print(f"  {rule:8} {n:6} / {rej_crash[rule]}")
    print(f"\nViolations in accepted programs: {len(problems)}")
    for seed, p in problems[:15]:
        print(f"  Seed {seed}: {p}")
    return 1 if problems and not args.disable_rule else 0


if __name__ == "__main__":
    sys.exit(main())
