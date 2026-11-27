"""Testlauf für das ausführbare FR++-Modell.

Aufruf (aus diesem Verzeichnis):  py -X utf8 run_tests.py

tests/accepted/*.fr  müssen die Typprüfung bestehen und dann genau das tun,
                       was die Kopfzeile "// erwartet:" sagt ("Ausgabe ..." oder
                       "BorrowError"). Dabei werden nach jeder Anweisung die
                       Invarianten I-1, I-2, I-4, I-5 geprüft; ein Fehlerzustand
                       (Stuck) ist immer ein Fehlschlag.
tests/rejected/*.fr   müssen mit der in der Kopfzeile genannten Regel in der
                       Zeile abgelehnt werden, die mit "// FEHLER" markiert ist.
                       Steht zusätzlich "// ohne Typprüfung: X" im Kopf, wird das
                       Programm trotzdem ausgeführt und muss mit X enden
                       (Fehlerzustand oder BorrowError). Das belegt, dass die
                       Regel einen echten Fehler verhindert.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from frpp import (BorrowError, CheckError, Checker, InvariantViolation,  # noqa: E402
                  Interp, OutOfFuel, ParseError, Stuck, parse)


def expectation(src):
    m = re.search(r"^// erwartet: (.*)$", src, re.M)
    return m.group(1).strip() if m else ""


def run_accepted(path):
    src = path.read_text(encoding="utf-8")
    want = expectation(src)
    try:
        prog = parse(src)
        checker = Checker(prog)
        checker.check()
    except (ParseError, CheckError) as exc:
        return f"rejected: {exc}"
    it = Interp(prog, check_invariants=True, checker=checker)
    try:
        it.run()
        got = "Ausgabe " + " ".join(map(str, it.out))
    except BorrowError:
        got = "BorrowError"
    except (Stuck, InvariantViolation) as exc:
        return f"Invalid state: {type(exc).__name__}: {exc}"
    except OutOfFuel:
        return "Schrittbudget erschöpft"
    if got.strip() != want:
        return f"expected {want!r}, got {got.strip()!r}"
    if got.startswith("Ausgabe") and not it.shared_used and it.live_objects():
        return f"Leak without shared: {it.live_objects()}"
    return None


def run_rejected(path):
    src = path.read_text(encoding="utf-8")
    want = expectation(src)
    m = re.match(r"Fehler (\S+)", want)
    rule = m.group(1) if m else "?"
    marked = [i + 1 for i, line in enumerate(src.splitlines()) if "// FEHLER" in line]
    if len(marked) != 1:
        return f"expected one '// FEHLER' marker, found {len(marked)}"
    try:
        prog = parse(src)
        Checker(prog).check()
    except ParseError as exc:
        return f"Syntax error: {exc}"
    except CheckError as exc:
        if exc.rule != rule or exc.line != marked[0]:
            return f"expected {rule} on line {marked[0]}, got {exc.rule} on line {exc.line}: {exc.msg}"
        return run_unchecked(prog, src)
    return "unexpectedly accepted"


def run_unchecked(prog, src):
    """Programme mit Kopfzeile "// ohne Typprüfung: X" werden trotz Ablehnung
    ausgeführt und müssen dann mit X enden: Die Regel verhindert einen echten Fehler."""
    m = re.search(r"^// ohne Typprüfung: (\S+)", src, re.M)
    if not m:
        return None
    try:
        Interp(prog).run()
        got = "normales Ende"
    except (Stuck, InvariantViolation):
        got = "Fehlerzustand"
    except BorrowError:
        got = "BorrowError"
    except OutOfFuel:
        got = "Schrittbudget erschöpft"
    if got != m.group(1):
        return f"without type checking: expected {m.group(1)}, got {got}"
    return None


def main():
    failures = 0
    lines = ["== Accepted programs (type checking, execution, invariants) =="]
    acc = sorted((ROOT / "tests" / "accepted").glob("*.fr"))
    for p in acc:
        err = run_accepted(p)
        failures += err is not None
        lines.append(f"  {'ok   ' if err is None else 'FAIL '} {p.name}" + (f": {err}" if err else ""))
    lines.append("")
    lines.append("== Rejected programs (rule and line) ==")
    rej = sorted((ROOT / "tests" / "rejected").glob("*.fr"))
    for p in rej:
        err = run_rejected(p)
        failures += err is not None
        text = p.read_text(encoding="utf-8")
        rule = re.match(r"Fehler (\S+)", expectation(text))
        unsafe = re.search(r"^// ohne Typprüfung: (\S+)", text, re.M)
        label = f"{p.name} [{rule.group(1) if rule else '?'}]" + \
            (f"  without type checking: {unsafe.group(1)}" if unsafe else "")
        lines.append(f"  {'ok   ' if err is None else 'FAIL '} {label}" + (f": {err}" if err else ""))
    lines.append("")
    lines.append(f"Scope: {len(acc)} accepted, {len(rej)} rejected programs")
    lines.append(f"Result: {failures} failures")
    print("\n".join(lines))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
