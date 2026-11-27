"""Testlauf für die R++-Grammatik.

Aufruf (aus dem Projektverzeichnis):  python3 tools/run_tests.py

Vier Arten von Tests:
  - tests/pos/*.rpp      müssen fehlerfrei geparst werden
  - NEGATIVE (unten)     müssen abgelehnt werden
  - PROBES (unten)       müssen auf eine bestimmte Weise gelesen werden
  - ../conformance/**  Konformitätstests: einzelne Dateien oder
                         Projekttests (Verzeichnisse mit rpp.yaml). Sie
                         müssen fehlerfrei geparst werden; ein abgelehnter
                         Test darf nur einen Syntaxfehler in der mit FEHLER
                         markierten Zeile haben.
Dazu meldet er die Regelabdeckung (Kapitel 14, R-14.6).
Zusätzlich meldet der Lauf jede Stelle, an der ANTLR eine echte
Mehrdeutigkeit der Grammatik feststellt.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CONFORMANCE = ROOT.parent / "conformance"
sys.path.insert(0, str(ROOT / "gen"))
sys.path.insert(0, str(ROOT / "tools"))

from antlr4 import CommonTokenStream, InputStream  # noqa: E402
from antlr4.atn.PredictionMode import PredictionMode  # noqa: E402
from antlr4.error.ErrorListener import ErrorListener  # noqa: E402

from preprocess import DirectiveError, preprocess  # noqa: E402
from RPPLexer import RPPLexer  # noqa: E402
from RPPParser import RPPParser  # noqa: E402


class Collector(ErrorListener):
    def __init__(self):
        self.errors = []
        self.ambiguities = []

    def syntaxError(self, recognizer, offendingSymbol, line, column, msg, e):
        self.errors.append(f"Line {line}:{column} {msg}")

    def reportAmbiguity(self, recognizer, dfa, startIndex, stopIndex, exact,
                        ambigAlts, configs):
        rule = recognizer.ruleNames[dfa.atnStartState.ruleIndex]
        tokens = recognizer.getTokenStream()
        text = tokens.getText(startIndex, stopIndex)
        line = tokens.get(startIndex).line
        self.ambiguities.append(f"Rule {rule}, line {line}: {text!r}")


def check_expr_statements(tree, errors):
    """Eine Ausdrucksanweisung ohne Zuweisung muss mit Aufruf, ++ oder -- enden."""
    if isinstance(tree, RPPParser.ExprStatementContext) and tree.getChildCount() == 1:
        ops = tree.postfixExpr().postfixOp()
        last = ops[-1].getChild(0).getText() if ops else None
        if last not in ("(", "++", "--"):
            errors.append(
                f"Line {tree.start.line}: expression {tree.getText()!r} "
                "cannot be used as a statement")
    for i in range(tree.getChildCount()):
        child = tree.getChild(i)
        if hasattr(child, "getChildCount") and child.getChildCount() > 0:
            check_expr_statements(child, errors)


def parse(source, symbols=frozenset({"debug", "linux"})):
    collector = Collector()
    try:
        source = preprocess(source, set(symbols))
    except DirectiveError as exc:
        collector.errors.append(f"Direktive: {exc}")
        return None, collector
    lexer = RPPLexer(InputStream(source))
    lexer.removeErrorListeners()
    lexer.addErrorListener(collector)
    parser = RPPParser(CommonTokenStream(lexer))
    parser.removeErrorListeners()
    parser.addErrorListener(collector)
    parser._interp.predictionMode = PredictionMode.LL_EXACT_AMBIG_DETECTION
    tree = parser.compilationUnit()
    if not collector.errors:
        check_expr_statements(tree, collector.errors)
    return tree, collector


def in_method(statements):
    return "class T { void m() { " + statements + " } }"


def in_class(members):
    return "class T { " + members + " }"


def load_cases(directory):
    """Liest Testfälle aus Dateien: erste Zeile "// Name: …", bei Lesarten zweite Zeile
    "// Lesart: <Regel>". Der Rust-Parser in rppc/ liest dieselben Dateien."""
    cases = {}
    for path in sorted((ROOT / "tests" / directory).glob("*.rpp")):
        text = path.read_text(encoding="utf-8")
        name = re.match(r"// Name: (.*)", text).group(1)
        rule = re.search(r"^// Lesart: (\w+)", text, re.M)
        cases[name] = (text, rule.group(1)) if rule else text
    return cases


# Name -> Quelltext, der abgelehnt werden muss (tests/neg).
NEGATIVE = load_cases("neg")

# Name -> (Quelltext, erwartete Regel der ersten Anweisung im Methodenrumpf) (tests/parse_probes)
PROBES = load_cases("parse_probes")

# Lesarten, bei denen eine Mehrdeutigkeit erwartet und per Regel aufgelöst ist.
EXPECTED_AMBIGUITY = {
    "else gehört zum inneren if": "else bindet an das nächste if (wie in C, Java, C#)",
    "Tupel-Deklaration mit generischen Typen": "die Deklaration geht vor (Typ gefolgt von Name, wie bei a < b > c;)",
}

# `(A<B> x, C<D> y) = e;` ließe sich auch als Zuweisung an ein Tupel aus Vergleichen lesen; die
# Deklaration geht vor. In den Konformitätstests gilt diese Mehrdeutigkeit als bekannt.
GENERIC_TUPLE = re.compile(r"Rule statement, line \d+: '\(\w[\w.]*<")


def first_statement_rule(tree):
    method = tree.typeDecl(0).classDecl().classMember(0).methodDecl()
    statement = method.block().statement(0)
    child = statement.getChild(0)
    return RPPParser.ruleNames[child.getRuleIndex()]


RULE_DEF = re.compile(r"^(?:\*\*|)R-(\d+)\.(\d+)[ (*]", re.M)
RULE_REF = re.compile(r"R-(\d+)\.(\d+)")


def exempt_rules():
    """Regeln, die sich nicht mit einem einzelnen Testprogramm prüfen lassen; sie stehen
    mit Begründung in Abschnitt 5 von „R++ Konformitätstests“ (R-14.6)."""
    doc = (ROOT.parent / "R++ Conformance Tests.md").read_text(encoding="utf-8")
    start = doc.find("## 5.")
    if start < 0:
        return set()
    end = doc.find("\n## ", start + 1)
    section = doc[start:end if end > 0 else len(doc)]
    return {(int(a), int(b)) for a, b in re.findall(r"^\| R-(\d+)\.(\d+) \|", section, re.M)}


def conformance_units():
    """Einzeltests (eine Datei) und Projekttests (ein Verzeichnis mit rpp.yaml, R-14.1).
    Liefert (Testpfad, Art, Quelldateien, Kopfdatei, Projektdateien)."""
    if not CONFORMANCE.is_dir():
        return []
    kinds = ("accepted", "rejected", "runtime")
    projects = [p.parent for p in CONFORMANCE.rglob("rpp.yaml") if p.parent.parent.name in kinds]
    units = []
    for path in CONFORMANCE.rglob("*.rpp"):
        if not any(proj in path.parents for proj in projects):
            units.append((path, path.parent.name, [path], path, [path]))
    for proj in projects:
        files = sorted(f for f in proj.rglob("*") if f.is_file())
        units.append((proj, proj.parent.name, sorted(proj.rglob("*.rpp")), proj / "rpp.yaml", files))
    return sorted(units, key=lambda u: u[0])


MARKER = re.compile(r"(//|#) FEHLER\s*$")
ERROR_LINE = re.compile(r"Line (\d+)")


def check_unit(path, kind, sources, header, files):
    """Prüft einen Konformitätstest, soweit das ohne Implementierung geht."""
    problems, ambiguities = [], []
    markers = [(f, n) for f in files
               for n, line in enumerate(f.read_text(encoding="utf-8").splitlines(), start=1)
               if MARKER.search(line)]
    for src in sources:
        tree, collector = parse(src.read_text(encoding="utf-8"))
        ambiguities.extend(collector.ambiguities)
        if collector.errors:
            # Ein abgelehntes Programm darf einen Syntaxfehler haben, wenn der erste Fehler in der
            # markierten Zeile liegt; weitere Meldungen sind Folgefehler der Fehlerbehandlung.
            m = ERROR_LINE.search(collector.errors[0])
            at_marker = kind == "rejected" and m and (src, int(m.group(1))) in markers
            if not at_marker:
                problems.extend(f"{src.name}: {err}" for err in collector.errors)
    if kind == "rejected" and len(markers) != 1:
        problems.append(f"expected one 'FEHLER' marker, found {len(markers)}")
    if kind != "rejected" and markers:
        problems.append(f"test in '{kind}' contains a 'FEHLER' marker")
    head = "\n".join(header.read_text(encoding="utf-8").splitlines()[:8])
    if kind == "runtime" and not re.search(r"^(//|#) Erwartet( \((debug|release)\))?: ", head, re.M):
        problems.append("runtime test is missing the 'Erwartet: …' header")
    return problems[:6], ambiguities


def rule_coverage():
    """Vergleicht die in den Kapiteldokumenten definierten Regeln R-x.y mit den
    Regeln, die in den Kopfzeilen der Testprogramme genannt werden (Kapitel 14,
    R-14.6). Fehlende Regeln sind kein Fehlschlag, sie werden nur gemeldet."""
    defined = {}
    for doc in sorted(ROOT.parent.glob("R++ *.md")):
        if doc.name.endswith(".de.md"):
            continue
        for m in RULE_DEF.finditer(doc.read_text(encoding="utf-8")):
            defined.setdefault(int(m.group(1)), set()).add(int(m.group(2)))
    mentioned = set()
    sources = (list(CONFORMANCE.rglob("*.rpp")) + list(CONFORMANCE.rglob("rpp.yaml"))
               + list((ROOT.parent / "fr-model" / "tests").rglob("*.fr")))
    for path in sources:
        head = "\n".join(path.read_text(encoding="utf-8").splitlines()[:6])
        for m in RULE_REF.finditer(head):
            mentioned.add((int(m.group(1)), int(m.group(2))))
    exempt = exempt_rules()
    out, total, covered, excused = [], 0, 0, 0
    for chapter in sorted(defined):
        if chapter in (1, 14):      # Regeln über die Spezifikation und die Testsuite selbst
            continue
        rules = sorted(defined[chapter])
        tested = [r for r in rules if (chapter, r) in mentioned]
        free = [r for r in rules if (chapter, r) not in mentioned and (chapter, r) in exempt]
        missing = [r for r in rules if (chapter, r) not in mentioned and (chapter, r) not in exempt]
        total += len(rules)
        covered += len(tested)
        excused += len(free)
        text = ", ".join(f"R-{chapter}.{r}" for r in missing) if missing else "none"
        note = f", {len(free)} exempt" if free else ""
        out.append(f"  Chapter {chapter:2}: {len(tested):3} of {len(rules):3} rules covered{note}. Missing: {text}")
    out.append(f"  total: {covered} of {total} rules listed in at least one test header, "
               f"{excused} exempt, {total - covered - excused} missing")
    return out


def main():
    failures = 0
    ambiguities = []
    known = []
    lines = []

    lines.append("== Positive tests (must be accepted) ==")
    for path in sorted((ROOT / "tests" / "pos").glob("*.rpp")):
        if "bedingte" in path.name:
            symbols_list = [{"debug"}, {"release"}]
        elif "bedingungen" in path.name:
            symbols_list = [{"debug", "linux"}, {"debug", "windows"},
                            {"release", "macos", "TRACE"}, {"release", "windows"}]
        else:
            symbols_list = [{"debug", "linux"}]
        for symbols in symbols_list:
            label = path.name + (f" [{'/'.join(sorted(symbols))}]" if len(symbols_list) > 1 else "")
            tree, collector = parse(path.read_text(encoding="utf-8"), symbols)
            expect_ok = not ("bedingte" in path.name and symbols == {"release"})
            ok = not collector.errors
            if ok == expect_ok:
                note = "" if expect_ok else "  (release branch contains an intentional error)"
                lines.append(f"  ok    {label}{note}")
            else:
                failures += 1
                lines.append(f"  FAIL  {label}")
                lines.extend(f"         {e}" for e in collector.errors[:5])
            ambiguities.extend(f"{label}: {a}" for a in collector.ambiguities)

    lines.append("")
    lines.append("== Negative tests (must be rejected) ==")
    for name, source in NEGATIVE.items():
        tree, collector = parse(source)
        if collector.errors:
            lines.append(f"  ok    {name}")
        else:
            failures += 1
            lines.append(f"  FAIL  {name}: unexpectedly accepted")

    lines.append("")
    lines.append("== Parse interpretations ==")
    for name, (source, expected) in PROBES.items():
        tree, collector = parse(source)
        if collector.errors:
            failures += 1
            lines.append(f"  FAIL  {name}: {collector.errors[0]}")
            continue
        actual = first_statement_rule(tree)
        if actual == expected:
            lines.append(f"  ok    {name}  ->  {actual}")
        else:
            failures += 1
            lines.append(f"  FAIL  {name}: expected {expected}, got {actual}")
        if name in EXPECTED_AMBIGUITY and collector.ambiguities:
            known.append(f"{name}: {EXPECTED_AMBIGUITY[name]}")
        else:
            ambiguities.extend(f"{name}: {a}" for a in collector.ambiguities)

    lines.append("")
    lines.append("== Conformance tests (syntax only) ==")
    conformance = conformance_units()
    for path, kind, sources, header, files in conformance:
        label = path.relative_to(CONFORMANCE).as_posix() + ("/" if path.is_dir() else "")
        problems, found = check_unit(path, kind, sources, header, files)
        if problems:
            failures += 1
            lines.append(f"  FAIL  {label}")
            lines.extend(f"         {e}" for e in problems)
        else:
            lines.append(f"  ok    {label}")
        for a in found:
            if GENERIC_TUPLE.search(a):
                entry = "Tupel-Deklaration mit generischen Typen: " + EXPECTED_AMBIGUITY["Tupel-Deklaration mit generischen Typen"]
                if entry not in known:
                    known.append(entry)
            else:
                ambiguities.append(f"{label}: {a}")

    lines.append("")
    lines.append("== Ambiguities reported by ANTLR ==")
    lines.extend(f"  known and resolved: {k}" for k in known)
    if ambiguities:
        lines.extend(f"  UNEXPECTED: {a}" for a in ambiguities)
    else:
        lines.append("  no unexpected ambiguities")

    lines.append("")
    lines.append("== Rules not listed in any test header ==")
    lines.extend(rule_coverage())

    lines.append("")
    counts = (len(list((ROOT / "tests" / "pos").glob("*.rpp"))), len(NEGATIVE), len(PROBES))
    lines.append(f"Scope: {counts[0]} positive files, {counts[1]} negative tests, {counts[2]} parse interpretations, "
                 f"{len(conformance)} conformance tests")
    lines.append(f"Result: {failures} failures, {len(ambiguities)} unexpected ambiguities")
    print("\n".join(lines))
    return 1 if failures or ambiguities else 0


if __name__ == "__main__":
    sys.exit(main())
