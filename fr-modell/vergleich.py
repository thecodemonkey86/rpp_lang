"""Vergleich des FR++-Modells mit der ersten Implementierung rppc (Meilenstein M3).

Aufruf (aus diesem Verzeichnis):  py -X utf8 vergleich.py [Anzahl] [--start N] [-v] [--ausfuehren]
                                   py -X utf8 vergleich.py 300 --abschwaechen loans

Mit --ausfuehren werden Programme, die beide annehmen, auch ausgeführt: Das Modell
(frpp/interp.py) und `rppc run` müssen dieselbe Ausgabe liefern (einschließlich der
Destruktoren) bzw. beide mit BorrowError enden (V-1). Programme, bei denen das Modell
sein Schrittbudget erschöpft, werden nicht ausgeführt.

Mit --abschwaechen prüft das Modell ohne eine Regel (wie in fuzz.py); dann müssen
Abweichungen auftreten. Das zeigt, dass der Vergleich die Regel erreicht.

Jedes FR++-Programm wird nach Kapitel 5, Abschnitt 7 in R++ übersetzt
(frpp/to_rpp.py) und von rppc geprüft. Verglichen wird das Urteil: angenommen
oder abgelehnt. Bei `move x` für int-, weak- und shared-Variablen wird die
Normalform verglichen: An der Oberfläche ist das eine Kopie, deshalb steht auch im
FR++-Programm `copy x` (siehe frpp/to_rpp.py). Geprüft werden die handgeschriebenen Programme in tests/ und
Zufallsprogramme aus frpp/gen.py. Programme mit Konstrukten ohne Entsprechung an
der Oberfläche zählen als „nicht übersetzbar“. Abweichungen werden mit beiden
Texten in vergleich-befunde/ abgelegt.
"""

import argparse
import copy
import pathlib
import subprocess
import sys
import tempfile
from collections import Counter

ROOT = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from frpp import CheckError, Checker, ParseError, parse  # noqa: E402
from frpp import ast as A  # noqa: E402
from frpp.gen import generate  # noqa: E402
from frpp.to_rpp import Untranslatable, translate  # noqa: E402
from frpp.interp import BorrowError, Interp, InvariantViolation, OutOfFuel, Stuck  # noqa: E402

AUSFUEHREN = False


def fr_run(prog):
    """Ergebnis des Modells: ("Ausgabe", [Zahlen]) oder ("BorrowError", None); None, wenn nicht vergleichbar."""
    it = Interp(prog, fuel=3000, check_invariants=False, checker=Checker(prog))
    try:
        it.run()
        return ("Ausgabe", [str(x) for x in it.out])
    except BorrowError:
        return ("BorrowError", None)
    except (OutOfFuel, Stuck, InvariantViolation, RecursionError):
        return None


def rpp_run(work):
    f = work / "prog.rpp"
    try:
        r = subprocess.run([str(RPPC), "run", "--pruefen", str(f)], capture_output=True, text=True, encoding="utf-8", timeout=20, input="")
    except subprocess.TimeoutExpired:
        return ("Zeitüberschreitung", None)
    if r.returncode == 2 and "BorrowError" in r.stderr:
        return ("BorrowError", None)
    if r.returncode != 0:
        return (f"Exit-Code {r.returncode}: {r.stderr.strip()[:200]}", None)
    return ("Ausgabe", r.stdout.split())

RPPC = ROOT.parent / "rppc" / "target" / "debug" / ("rppc.exe" if sys.platform == "win32" else "rppc")


CHECKER = Checker


def fr_verdict(prog):
    try:
        CHECKER(prog).check()
        return True, ""
    except CheckError as exc:
        return False, exc.rule


def rpp_verdict(text, work):
    f = work / "prog.rpp"
    f.write_text(text, encoding="utf-8")
    r = subprocess.run([str(RPPC), "check", str(f)], capture_output=True, text=True, encoding="utf-8")
    errors = [l for l in r.stdout.splitlines() if ": Fehler [" in l]
    return r.returncode == 0, errors


def compare(name, prog, work, findings, verbose):
    prog = copy.deepcopy(prog)
    try:
        text, normalized = translate(prog)
    except Untranslatable as exc:
        if verbose:
            print(f"  –      {name}: nicht übersetzbar ({exc})")
        return "nicht übersetzbar", fr_verdict(prog)[0]
    # geprüft wird die Normalform (move eines int-, weak- oder shared-Werts als copy)
    fr_ok, fr_rule = fr_verdict(prog)
    rpp_ok, errors = rpp_verdict(text, work)
    if fr_ok == rpp_ok:
        if fr_ok and AUSFUEHREN:
            want = fr_run(prog)
            if want is not None:
                got = rpp_run(work)
                if got != want:
                    print(f"  LAUFZEIT {name}: Modell {want[0]} {' '.join(want[1] or [])} – rppc {got[0]} {' '.join(got[1] or [])}")
                    if findings is not None:
                        findings.mkdir(exist_ok=True)
                        stem = name.replace("/", "_").replace(".fr", "")
                        (findings / f"{stem}.fr").write_text(A.program_str(prog), encoding="utf-8")
                        (findings / f"{stem}.rpp").write_text(text, encoding="utf-8")
                    return "Laufzeit verschieden", fr_ok
                return "gleich (ausgeführt)", fr_ok
        if verbose:
            print(f"  ok     {name}: {'angenommen' if fr_ok else 'abgelehnt (' + fr_rule + ')'}")
        return "gleich", fr_ok
    kind = "nur rppc lehnt ab" if fr_ok else "nur FR++ lehnt ab"
    print(f"  ABWEICHUNG {name}: {kind} {fr_rule}")
    for e in errors[:3]:
        print(f"         {e.split(': ', 1)[-1]}")
    if findings is not None:
        findings.mkdir(exist_ok=True)
        stem = name.replace("/", "_").replace(".fr", "")
        (findings / f"{stem}.fr").write_text(A.program_str(prog), encoding="utf-8")
        (findings / f"{stem}.rpp").write_text(text + "\n// " + "\n// ".join(errors) + "\n", encoding="utf-8")
    return kind, fr_ok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("count", nargs="?", type=int, default=500)
    ap.add_argument("--start", type=int, default=0)
    ap.add_argument("-v", action="store_true")
    ap.add_argument("--ausfuehren", action="store_true", help="angenommene Programme auch ausführen und die Ausgaben vergleichen")
    ap.add_argument("--abschwaechen", help="Gegenprobe: FR++-Prüfer ohne diese Regel (siehe fuzz.py)")
    a = ap.parse_args()
    global AUSFUEHREN
    AUSFUEHREN = a.ausfuehren
    if a.abschwaechen:
        from fuzz import ABSCHWAECHUNGEN
        global CHECKER
        CHECKER = ABSCHWAECHUNGEN[a.abschwaechen]
    if not RPPC.exists():
        sys.exit(f"{RPPC} fehlt; zuerst in rppc/ cargo build ausführen")
    findings = None if a.abschwaechen else ROOT / "vergleich-befunde"
    stats = {"Testprogramme": Counter(), "Zufallsprogramme": Counter()}
    with tempfile.TemporaryDirectory() as tmp:
        work = pathlib.Path(tmp)
        print("Handgeschriebene Programme (tests/):")
        for path in sorted((ROOT / "tests").glob("*/*.fr")):
            try:
                prog = parse(path.read_text(encoding="utf-8"))
            except ParseError:
                continue
            res, fr_ok = compare(f"{path.parent.name}/{path.name}", prog, work, findings, a.v)
            stats["Testprogramme"][(res, fr_ok)] += 1
        print(f"Zufallsprogramme (Seeds {a.start} bis {a.start + a.count - 1}):")
        for seed in range(a.start, a.start + a.count):
            res, fr_ok = compare(f"seed{seed}", generate(seed), work, findings, a.v)
            stats["Zufallsprogramme"][(res, fr_ok)] += 1
    print()
    print(f"{'':18} {'gleich (ang./abg.)':>20} {'nur rppc lehnt ab':>18} {'nur FR++ lehnt ab':>18} {'nicht übersetzbar':>18}")
    bad = 0
    for k, c in stats.items():
        ran = c[("gleich (ausgeführt)", True)]
        same = f"{c[('gleich', True)] + ran} / {c[('gleich', False)]}"
        r1 = c[("nur rppc lehnt ab", True)]
        r2 = c[("nur FR++ lehnt ab", False)]
        nu = c[("nicht übersetzbar", True)] + c[("nicht übersetzbar", False)]
        rt = c[("Laufzeit verschieden", True)]
        bad += r1 + r2 + rt
        print(f"{k:18} {same:>20} {r1:>18} {r2:>18} {nu:>18}")
        if AUSFUEHREN:
            print(f"{'':18} ausgeführt und gleich: {ran}, verschieden: {rt}")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
