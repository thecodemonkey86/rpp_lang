"""Zufallstests für die Sätze 5.1 bis 5.3 des Ownership-Kernkalküls.

Aufruf:  py -X utf8 fuzz.py [Anzahl] [--abschwaechen NAME] [--start N]

Für jedes zufällige Programm:
  - Typprüfung. Abgelehnte Programme werden trotzdem ausgeführt; gezählt wird,
    wie viele davon tatsächlich in einen Fehlerzustand laufen (zeigt, dass die
    Regeln echte Fehler verhindern).
  - Angenommene Programme laufen mit Invariantenprüfung. Geprüft wird:
      Satz 5.1  kein Fehlerzustand (Stuck) und keine verletzte Invariante
      Satz 5.3  kein BorrowError, wenn nie ein Objekt geteilt wurde
      Zusatz    ohne shared bleibt am Ende kein Objekt übrig (kein Leck)
    Jeder Verstoß wird mit Programmtext in fuzz-befunde/ abgelegt.

Mit --abschwaechen wird eine Regel des Typprüfers absichtlich abgeschaltet
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
    "schleife": OhneSchleifenregel,
    "shared-ueberlappung": OhneSharedUeberlappung,
    "schreibrecht": OhneSchreibrecht,
    "empfaenger-loan": OhneEmpfaengerLoan,
}


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
        res["outcome"] = "Fehlerzustand"
        res["detail"] = f"{type(exc).__name__}: {exc}"
    except RecursionError:
        res["outcome"] = "Budget"
    except Exception as exc:  # noqa: BLE001 - Fehler im Modell selbst
        res["outcome"] = "Modellfehler"
        res["detail"] = f"{type(exc).__name__}: {exc}"
    res["stats"] = it.stats
    if res["accepted"]:
        problem = None
        if res["outcome"] in ("Fehlerzustand", "Modellfehler"):
            problem = "Satz 5.1: " + res["detail"]
        elif res["outcome"] == "BorrowError" and not it.shared_used:
            problem = "Satz 5.3: BorrowError ohne geteiltes Objekt"
        elif res["outcome"] == "ok" and not it.shared_used and it.live_objects():
            problem = f"Leck ohne shared: {it.live_objects()}"
        res["problem"] = problem
        if problem and findings_dir is not None:
            findings_dir.mkdir(exist_ok=True)
            (findings_dir / f"seed{seed}.fr").write_text(
                f"// {problem}\n" + A.program_str(prog), encoding="utf-8")
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("anzahl", type=int, nargs="?", default=2000)
    ap.add_argument("--start", type=int, default=0)
    ap.add_argument("--groesse", type=float, default=1.0)
    ap.add_argument("--abschwaechen", choices=sorted(ABSCHWAECHUNGEN))
    args = ap.parse_args()
    sys.setrecursionlimit(5000)

    checker_cls = ABSCHWAECHUNGEN.get(args.abschwaechen, Checker)
    findings = None if args.abschwaechen else ROOT / "fuzz-befunde"
    t0 = time.time()
    acc_out, rej_rule, rej_crash, problems, stats = Counter(), Counter(), Counter(), [], Counter()
    for seed in range(args.start, args.start + args.anzahl):
        r = run_one(seed, checker_cls, findings, args.groesse)
        if r["accepted"]:
            acc_out[r["outcome"]] += 1
            stats.update(r["stats"])
            if r["problem"]:
                problems.append((seed, r["problem"]))
        else:
            rej_rule[r["rule"]] += 1
            if r["outcome"] == "Fehlerzustand":
                rej_crash[r["rule"]] += 1

    n_acc = sum(acc_out.values())
    print(f"Programme: {args.anzahl} (Seeds {args.start}..{args.start + args.anzahl - 1}), "
          f"Typprüfer: {args.abschwaechen and 'ohne ' + args.abschwaechen or 'vollständig'}, "
          f"{time.time() - t0:.0f} s")
    print(f"\nAngenommen: {n_acc}")
    for k in ("ok", "BorrowError", "Budget", "Fehlerzustand", "Modellfehler"):
        print(f"  {k:14} {acc_out[k]}")
    print("  Abdeckung:   " + ", ".join(f"{k} {stats[k]}" for k in
                                         ("new", "free", "lock", "borrow_error", "upgrade_ok", "upgrade_fail",
                                          "borrow_call")))
    print(f"\nAbgelehnt: {sum(rej_rule.values())}  (Regel: abgelehnt / davon zur Laufzeit im Fehlerzustand)")
    for rule, n in sorted(rej_rule.items(), key=lambda kv: -kv[1]):
        print(f"  {rule:8} {n:6} / {rej_crash[rule]}")
    print(f"\nVerstöße bei angenommenen Programmen: {len(problems)}")
    for seed, p in problems[:15]:
        print(f"  Seed {seed}: {p}")
    return 1 if problems and not args.abschwaechen else 0


if __name__ == "__main__":
    sys.exit(main())
