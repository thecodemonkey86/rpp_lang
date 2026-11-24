"""Vergleich des Compilers in R++ (rpp-compiler) mit der ersten Implementierung (rppc).

Aufruf (aus diesem Verzeichnis):  py -X utf8 vergleich.py [-v] [tokens|pre ...]

Verglichen werden je Teil die Ausgaben beider Seiten:
  tokens  Lexer: eine Zeile je Token (`rppc tokens DATEI` bzw. `rppcc tokens DATEI`)
  pre     Präprozessor: Text oder "Zeile N: Meldung" (`rppc pre SYMBOLE DATEI`), mit den
          Symbolsätzen debug,linux und release,windows
  ast     Parser einschließlich Präprozessor, Lexer und Prüfung R-8.12: Syntaxbaum in
          Textform oder der erste Fehler (`rppc ast SYMBOLE DATEI`), mit denselben Symbolsätzen
  projekt Projekte laden (Kapitel 11): Projekte, Dateien mit Paketen, Meldungen
          (`rppc projekt [--release] STD PFAD`), im Debug- und Releasemodus. Eingaben sind
          die Projektverzeichnisse und Einzeldateien der Konformitätstests, rpp-compiler
          selbst und Grenzfälle für rpp.yaml
  defs    Deklarationsphase (Typen sammeln, Namen auflösen, Membertabellen, Regeln auf Ebene
          von Dateien und Klassen): Tabellen in Textform und Meldungen (`rppc defs STD PFAD`).
          Eingaben wie bei projekt, dazu die Grammatiktests und Grenzfälle
  pruefe  ganze Prüfung einschließlich der Rümpfe (Typen der Ausdrücke, Null-Sicherheit,
          Ownership, Flussanalyse): alle Meldungen mit Warnungen und Begründungsregeln
          (`rppc pruefe STD PFAD`). Eingaben wie bei defs ohne rpp-compiler selbst, dazu
          die nach R++ übersetzten FR++-Programme (fr-modell: tests/ und Zufallsprogramme;
          Anzahl mit --fr N, Voreinstellung 300)

Eingaben sind alle .rpp-Dateien des Projekts (Konformitätstests, Grammatiktests,
Standardbibliothek, rpp-compiler selbst) und Grenzfälle je Teil. Die Ausgaben müssen gleich
sein; Abweichungen werden mit der ersten verschiedenen Zeile gemeldet.
"""

import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
RPPC = ROOT / "rppc" / "target" / "debug" / ("rppc.exe" if sys.platform == "win32" else "rppc")

LEXER_FAELLE = {
    "kommentar_offen": "class A { }\n/* nie zu",
    "zeichen_unerwartet": "i32 x = 1 # 2;",
    "string_offen": 'string s = "abc\n;',
    "string_escape_am_ende": 'string s = "abc\\',
    "zeichen_zu_lang": "char c = 'ab';",
    "zeichen_leer": "char c = '';",
    "zeichen_escape": "char c = '\\'';\nchar d = '\\\\';",
    "zahlen": "1..3 1.0e10 2.5E-3 1.0e 1_ 0xFF_FF 1e10 0x 7.e2 3.14_15 0x_1 1__2",
    "satzzeichen": "a?.b ?? c :: .. ++ -- && || == != <= >= += -= *= /= %= => >> << ? : ~ ^",
    "umlaute": "string grüße = \"Grüße, Welt\"; // Kommentar mit ä\nchar ß = 'ß';",
    "crlf": "class A {\r\n    i32 x;\r\n}\r\n",
    "leer": "",
    "nur_kommentar": "// nichts\n/* auch nichts */",
    "schluesselwoerter": "main mainx class classy own owner i8 i16 ui64 null nullable",
}

PRE_FAELLE = {
    "verschachtelt": "a\n#if debug && !windows\nb\n#elif linux\nc\n#else\nd\n#endif\ne",
    "elif_kette": "#if windows\nw\n#elif linux || macos\nl\n#elif debug\nd\n#else\ns\n#endif",
    "innen_inaktiv": "#if release\n#if debug\nx\n#else\ny\n#endif\n#endif\nz",
    "klammern": "#if (debug || release) && !(windows && linux)\nja\n#endif",
    "kommentar_nach_direktive": "#if debug // nur im Debugmodus\nx\n#endif // Ende",
    "eingerueckt": "  #if debug\n    x\n  #endif",
    "define": "#define X 1",
    "ohne_endif": "x\n#if debug\ny",
    "klammer_offen": "#if (debug && linux\n#endif",
    "doppeltes_und": "#if debug && && linux\n#endif",
    "bedingung_fehlt": "#if\n#endif",
    "ungueltiges_zeichen": "#if debug + linux\n#endif",
    "elif_nach_else": "#if debug\n#else\n#elif linux\n#endif",
    "else_mit_text": "#if debug\n#else linux\n#endif",
    "endif_allein": "#endif",
    "endif_mit_text": "#if debug\n#endif x",
    "nur_raute": "#\nx",
    "raute_mit_zahl": "#1 if",
    "unerwartet_rest": "#if debug linux\n#endif",
    "crlf": "#if debug\r\nx\r\n#endif\r\n",
    "leer": "",
}

AST_FAELLE = {
    "leer": "",
    "nur_package": "package a.b;",
    "lexerfehler": "class A { i32 x = 1 # 2; }",
    "direktive": "#if debug\nclass A { }\n",
    "ausdruck_als_anweisung": "class A { void m() { 1 + 2; } }",
    "for_kopf": "class A { void m() { for (i32 i = 0; i < 3; i + 1) { } } }",
    "lambda_im_for": "class A { void m() { for (;;) { f((i32 x) => { x; }); } } }",
    "as_und_bedingung": "class A { void m() { bool b = x as Foo ? true : false; } }",
    "verschachtelte_generics": "class A { Map<string, List<i32>> m; void f() { x = a < b > c; } }",
    "schiebung": "class A { void m() { i32 x = a >> 2 << 1; } }",
}

# Grenzfälle für das Laden von Projekten: Inhalt der rpp.yaml und weitere Dateien
APP = "name: app\nversion: 0.1.0\ntype: executable\nmain: src/main.rpp\nsources:\n  - src\n"
MAIN = {"src/main.rpp": "i32 main(string[] args) {\n    return 0;\n}\n"}
LIB_A = {"a/rpp.yaml": "name: acme.a\nversion: 1.0.0\ntype: library\nsources: [src]\n",
         "a/src/acme/a/A.rpp": "package acme.a;\nexport class A { }\n"}
PROJEKT_FAELLE = {
    "unicode_quellordner": ('name: app\nversion: 1\ntype: library\nsources: ["grün🙂 # eins", ,]\n',
                            {"grün🙂 # eins/app/T.rpp": "package app;\nexport class T { }\n"}),
    "einfach": (APP, MAIN),
    "kommentare_und_anfuehrung": ("# Kopf\nname: \"app\"   # Name\nversion: '0.1.0'\ntype: executable # x\nmain: src/main.rpp\nsources: [ src , ]\n", MAIN),
    "tabs_und_crlf": (APP.replace("\n", "\r\n").replace("  - src", "\t- src"), MAIN),
    "leer": ("", {}),
    "nur_kommentare": ("# nichts\n\n   # auch nichts\n", {}),
    "unerwartete_einrueckung": ("name: app\n  version: 1\n", {}),
    "einrueckung_am_ende": ("name:\n    - a\n  - b\n", {}),
    "schluessel_erwartet": ("name app\nversion: 1\n", {}),
    "wert_mit_doppelpunkt": (APP.replace("type: executable", "type: executable:extra"), MAIN),
    "leerer_wert": (APP.replace("version: 0.1.0", "version:"), MAIN),
    "fehlender_doppelpunkt": (APP.replace("version: 0.1.0", "version"), MAIN),
    "schluessel_ohne_leerzeichen": ("name:app\n", {}),
    "schluessel_mit_leerzeichen": ("my name: app\n", {}),
    "liste_schluessel_erwartet": ("name: app\ndependencies:\n  - name: acme.a\n    nur text\n", {}),
    "unbekannt_und_fehlend": ("name: app\nfoo: 1\nbar:\n  - x\n", {}),
    "name_schluesselwort": ("name: acme.class\nversion: 1\ntype: library\nsources: [src]\n", {"src/X.rpp": "class X { }\n"}),
    "name_leerer_teil": ("name: acme..b\nversion: 1\ntype: library\nsources: [src]\n", {"src/X.rpp": "class X { }\n"}),
    "name_als_liste": ("name: [a, b]\nversion: 1\ntype: library\nsources: [src]\n", {"src/.keep.rpp": ""}),
    "besitzer_rpp": ("name: rpp.x\nversion: 1\ntype: library\nsources: []\n", {}),
    "type_falsch": ("name: app\nversion: 1\ntype: programm\nsources: []\n", {}),
    "type_als_liste": ("name: app\nversion: 1\ntype:\n  - executable\nsources: []\n", {}),
    "programm_ohne_main": ("name: app\nversion: 1\ntype: executable\nsources: [src]\n", MAIN),
    "main_mit_punkten": (APP.replace("main: src/main.rpp", "main: ./x/../src/main.rpp"), MAIN),
    "abhaengigkeit": (APP + "dependencies:\n  - name: acme.a\n    path: a\n", {**MAIN, **LIB_A}),
    "abhaengigkeit_falscher_name": (APP + "dependencies:\n  - name: acme.b\n    path: ./a/\n", {**MAIN, **LIB_A}),
    "abhaengigkeit_fehlt": (APP + "dependencies:\n  - name: acme.z\n    path: z/../zz\n", MAIN),
    "abhaengigkeit_version": (APP + "dependencies:\n  - name: acme.v\n    version: 1.2.0\n", MAIN),
    "abhaengigkeit_ohne_angabe": (APP + "dependencies:\n  - name: acme.v\n  - acme.w\n", MAIN),
    "abhaengigkeit_programm": (APP + "dependencies:\n  - name: app2\n    path: p\n",
                               {**MAIN, "p/rpp.yaml": APP.replace("app", "app2"), "p/src/main.rpp": MAIN["src/main.rpp"]}),
    "abhaengigkeit_doppelt": (APP + "dependencies:\n  - name: acme.a\n    path: a\n  - name: acme.a\n    path: ./a\n", {**MAIN, **LIB_A}),
    "selbstbezug": (APP + "dependencies:\n  - name: app\n    path: .\n", MAIN),
    "leerer_listeneintrag": (APP + "dependencies:\n  -\n    name: acme.a\n    path: a\n  -\n", {**MAIN, **LIB_A}),
    "quellordner_fehlt": (APP.replace("  - src", "  - src\n  - gibtsnicht\n  - [x]"), MAIN),
    "quellen_als_text": (APP.replace("sources:\n  - src\n", "sources: src\n"), MAIN),
    "symbole": (APP + "symbols:\n  debug: [TRACE, 'X']\n  release: [FAST]\n",
                {"src/main.rpp": "#if TRACE && X\nclass T { }\n#elif FAST\nclass F { }\n#else\nclass N { }\n#endif\ni32 main(string[] args) {\n    return 0;\n}\n"}),
    "symbole_als_text": (APP + "symbols:\n  debug: TRACE\n", MAIN),
    "symbole_falsche_form": (APP + "symbols:\n  debug: [A]\n  test: [B]\n  release:\n    - C\n    - [D]\n", MAIN),
    "symbole_als_liste": (APP + "symbols: [A]\n", MAIN),
    "abhaengigkeiten_als_text": (APP + "dependencies: acme.a\n", MAIN),
    "pfad_als_liste": (APP + "dependencies:\n  - name: acme.a\n    path: [a]\n", {**MAIN, **LIB_A}),
    "unterordner_mit_projekt": (APP, {"src/z.rpp": "class Z { }\n", "src/sub/deep/X.rpp": "class X { }\n",
                                       "src/sub/rpp.yaml": "name: x\n", "src/sub/Y.rpp": "class Y { }\n",
                                       "src/pkg/inner/Y.rpp": "package pkg.inner;\nclass Y { }\n",
                                       "src/pkg/notiz.txt": "x", **MAIN, "src/a.rpp": "class A { }\n"}),
    "syntaxfehler": (APP, {**MAIN, "src/a/Bad.rpp": "package a;\nclass Bad { i32 x = 1 # 2; }\n",
                           "src/a/Dir.rpp": "package a;\n#if debug\nclass D { }\n", "src/a/Expr.rpp": "package a;\nclass E { void m() { 1 + 2; } }\n"}),
}

# Grenzfälle für die Deklarationsphase (Einzeldateien)
DEFS_FAELLE = {
    "enum_werte": "enum E {\n    A = 0x10,\n    B,\n    C = -3,\n    D = 1_000,\n    E2 = 17,\n    F = 3000000000,\n}\n",
    "enum_doppelt": "enum E {\n    A = 1,\n    B = 1,\n    A,\n    C = 0,\n}\nenum M {\n    X(i32 a),\n    Y = 2,\n    Z(string s, E e),\n}\n",
    "typen": "class A<T> {\npublic:\n    Func<i32, own A<T>, A<T>&, void> f;\n    Func<void, i32> g;\n    Func<i32, string&> h;\n    Slice<i32> s;\n    i32[4][] m;\n    (i32, string?)[] t;\n    shared A<T>? sa;\n    weak A<T> wa;\n    List<void> l;\n    A<i32&> r;\n    A<i32, i32> zu_viele;\n    Unbekannt u;\n    rpp.core.Exception e;\n    rpp.io.Gibts n;\n    static const T st = x;\n    static const i32 N = 4;\n    i32[N] arr;\n}\n",
    "zyklus": "class A : C { }\nclass B : A { }\nclass C : B { }\nclass D : D { }\nclass E : A { }\n",
    "basis_und_import": "mixin M { }\nenum E { X }\nclass A : M { }\nclass B : E { }\nclass C {\n    import A;\n    import M;\n}\n",
    "constraints": "mixin Box<T : Hashable & Equatable<T>> { }\nclass N<T : new()> { }\nabstract class Abs { }\nclass P {\n    constructor(i32 x) { }\n}\nenum Farbe { Rot }\nenum Daten { D(i32 x) }\nclass U {\n    N<Abs> a;\n    N<P> b;\n    N<U> c;\n    N<i32> d;\n    Box<i32> e;\n    Box<bool> f;\n    Box<Farbe> g;\n    Box<Daten> h;\n    Box<U> i;\n    Box<string?> j;\n}\nclass V<T> {\n    N<T> x;\n    Box<T> y;\n}\n",
    "comparable": "class W<T : Comparable<T>> { }\nenum Farbe { Rot }\nclass K {\n    import Comparable<K>;\npublic:\n    i32 compareTo(K o) { return 0; }\n    bool equals(K o) { return true; }\n    W<i32> a;\n    W<bool> b;\n    W<Farbe> c;\n    W<K> d;\n    W<string> e;\n}\n",
    "ueberschreiben": "class Basis {\npublic:\n    void a() { }\n    virtual void b() { }\n    virtual void c() { }\n    virtual void d(i32 x) { }\n    nodiscard virtual i32 e() { return 1; }\nprotected:\n    virtual void f() { }\npublic:\n    virtual void g() & { }\n    virtual void p() { }\n}\nclass Kind : Basis {\npublic:\n    override void a() { }\n    void b() { }\n    override void c() { }\n    override void d(i32& x) { }\n    override i32 e() { return 2; }\nprivate:\n    override void f() { }\npublic:\n    override void g() { }\n    override void h() { }\n    static void b() { }\nprotected:\n    override void p() { }\n}\n",
    "ueberladen": "class A {\npublic:\n    void m(i32 x) { }\n    void m(i32& x) { }\n    i32 m(i32 y) { return 0; }\n    void m(i32 z) { }\n    void n(own A a) { }\n    void n(A a) { }\n    static void m(i32 x) { }\n    constructor(i32 a) { }\n    constructor(i32 b) { }\n    i32 m;\n    i32 k;\n    i32 k;\n    nodiscard void v() { }\n}\n",
    "mixins": "mixin M1 {\n    void a() { }\n    i32 req();\n}\nmixin M2 {\n    void a() { }\n    import M1;\n}\nmixin M3 {\n    static void s() { }\n    nodiscard void v();\n}\nclass A {\n    import M1;\n    import M2;\n}\nclass B {\n    import M1;\npublic:\n    i32 req() { return 1; }\n    void a() { }\n}\nclass C {\n    import M1;\n    import M2;\npublic:\n    override void a() { }\n    string req() { return \"\"; }\n}\nabstract class D {\n    import M1;\n}\n",
    "abstrakt_copy_ctor": "abstract class A {\npublic:\n    abstract void f();\n    copy(A other) { }\n}\nclass B : A { }\nclass C {\npublic:\n    abstract void g();\n    constructor(i32 x) { }\n}\nclass D : C {\n    constructor() { }\n    copy(D o) { }\n}\nclass E : C { }\nclass F : C {\n    constructor() : base(1) { }\n}\n",
    "export": "class Intern { }\nenum IE { X }\nexport class Aussen : Intern {\npublic:\n    Intern a;\n    Intern b;\n    Intern? c(IE x) { return null; }\n    constructor(Intern[] xs) { }\nprivate:\n    Intern d;\n    Intern e() { return new Intern(); }\n}\nexport enum EE {\n    A(Intern i),\n}\nexport mixin EM {\n    Intern m();\n}\n",
    "usings": "package a.b;\nusing rpp.io.File;\nusing rpp.io.Fehlt;\nusing F = rpp.io.Dir;\nusing X = rpp.io.Path;\nusing Y = rpp.io.Path;\nusing Eigen = rpp.text.StringBuilder;\nusing Z = rpp.core.Exception;\nusing W = rpp.core.Exception;\nclass Eigen { }\nclass Nutzer {\n    File f;\n    F d;\n    Path p;\n    Z z;\n    W w;\n}\n",
    "mehrdeutig": "using A = rpp.io.File;\nusing A = rpp.io.Dir;\nclass K {\n    A a;\n    A b;\n}\n",
    "methoden_generisch": "class G {\npublic:\n    T id<T>(T x) { return x; }\n    void f<T : Hashable, U : new()>(T a, U b) { }\n    void g<T : Gibts>(T a) { }\n    ref T& h<T>(T[] xs) & { return xs[0]; }\n    void k(Func<own G, G&, void> f) { }\n}\n",
    "zwei_main": "i32 main(string[] args) {\n    return 0;\n}\ni32 main(string[] args) {\n    return 1;\n}\n",
    "doppelter_typ": "class A { }\nclass A { }\nenum A { X }\n",
}

SYMBOLE = ["debug,linux", "release,windows"]


def project_inputs():
    dirs = sorted(p.parent for p in (ROOT / "konformitaet").rglob("rpp.yaml"))
    # nur die äußeren Projekte; Abhängigkeiten werden über sie geladen
    outer = [d for d in dirs if not any(o != d and o in d.parents for o in dirs)]
    singles = sorted((ROOT / "konformitaet").rglob("*.rpp"))
    singles = [f for f in singles if not any(o in f.parents for o in dirs)]
    return outer + [ROOT / "rpp-compiler"] + singles


def defs_inputs():
    grammar = sorted((ROOT / "rpp-grammatik" / "tests").rglob("*.rpp"))
    return project_inputs() + grammar


def files():
    out = []
    for sub in ["konformitaet", "rpp-grammatik/tests", "rppc/std", "rpp-compiler/src"]:
        out += sorted((ROOT / sub).rglob("*.rpp"))
    return out


def run(cmd):
    r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", input="")
    if r.returncode != 0:
        return f"Exit-Code {r.returncode}: {r.stderr.strip()[:300]}\n" + r.stdout
    return r.stdout


# --exe PFAD: der mit dem Codegenerator übersetzte rppcc statt des Interpreters
EXE = None


def both(part, args):
    rust = run([str(RPPC), part, *args])
    if EXE:
        rpp = run([EXE, part, *args])
    else:
        rpp = run([str(RPPC), "run", str(ROOT / "rpp-compiler"), "--", part, *args])
    return rust, rpp


def report(name, a, b, verbose):
    if a == b:
        if verbose:
            print(f"  ok     {name}")
        return True
    la, lb = a.splitlines(), b.splitlines()
    i = next((k for k in range(max(len(la), len(lb))) if k >= len(la) or k >= len(lb) or la[k] != lb[k]), 0)
    print(f"  ABWEICHUNG {name}: Zeile {i + 1}")
    print(f"         Rust: {la[i] if i < len(la) else '(Ende)'}")
    print(f"         R++:  {lb[i] if i < len(lb) else '(Ende)'}")
    return False


def projekt(tmp, verbose):
    ok = bad = 0
    std = str(ROOT / "rppc" / "std")
    inputs = [(str(p.relative_to(ROOT)).replace("\\", "/"), p) for p in project_inputs()]
    for name, (yaml, extra) in PROJEKT_FAELLE.items():
        d = tmp / "projekte" / name
        d.mkdir(parents=True)
        (d / "rpp.yaml").write_bytes(yaml.encode("utf-8"))
        for rel, text in extra.items():
            f = d / rel
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_bytes(text.encode("utf-8"))
        inputs.append((f"Grenzfall {name}", d))
    for name, path in inputs:
        for mode in ([], ["--release"]):
            args = [*mode, std, str(path)]
            a, b = both("projekt", args)
            if name == "Grenzfall unterordner_mit_projekt":
                files = [line.split(" projekt=", 1)[0] for line in b.splitlines()
                         if line.startswith("datei ") and " projekt=1 " in line]
                expected = ["datei src/a.rpp", "datei src/main.rpp", "datei src/pkg/inner/Y.rpp", "datei src/z.rpp"]
                if files != expected:
                    print(f"  ABWEICHUNG {name}: erwartete Quelldateien {expected}, erhalten {files}")
                    bad += 1
                    continue
            if report(f"{name} {' '.join(mode)}".strip(), a, b, verbose):
                ok += 1
            else:
                bad += 1
    return ok, bad


def defs(tmp, verbose):
    ok = bad = 0
    std = str(ROOT / "rppc" / "std")
    inputs = [(str(p.relative_to(ROOT)).replace("\\", "/"), p) for p in defs_inputs()]
    for name, text in DEFS_FAELLE.items():
        f = tmp / f"defs_{name}.rpp"
        f.write_bytes(text.encode("utf-8"))
        inputs.append((f"Grenzfall {name}", f))
    for name, path in inputs:
        a, b = both("defs", [std, str(path)])
        if report(name, a, b, verbose):
            ok += 1
        else:
            bad += 1
    return ok, bad


def fr_programs(tmp, count):
    """Übersetzte FR++-Programme als Dateien (wie in fr-modell/vergleich.py)."""
    sys.path.insert(0, str(ROOT / "fr-modell"))
    from frpp import ParseError, parse
    from frpp.gen import generate
    from frpp.to_rpp import Untranslatable, translate
    progs = []
    for path in sorted((ROOT / "fr-modell" / "tests").glob("*/*.fr")):
        try:
            progs.append((f"FR++ {path.parent.name}/{path.name}", parse(path.read_text(encoding="utf-8"))))
        except ParseError:
            continue
    for seed in range(count):
        progs.append((f"FR++ seed{seed}", generate(seed)))
    out = []
    for name, prog in progs:
        try:
            text, _ = translate(prog)
        except Untranslatable:
            continue
        f = tmp / f"fr_{len(out)}.rpp"
        f.write_bytes(text.encode("utf-8"))
        out.append((name, f))
    return out


def pruefe(tmp, verbose, fr_count):
    ok = bad = 0
    std = str(ROOT / "rppc" / "std")
    # rpp-compiler selbst ist im Interpreter zu langsam für diesen Vergleich (nur mit --exe)
    inputs = [(str(p.relative_to(ROOT)).replace("\\", "/"), p) for p in defs_inputs() if p != ROOT / "rpp-compiler" or EXE]
    inputs += fr_programs(tmp, fr_count)
    for name, path in inputs:
        a, b = both("pruefe", [std, str(path)])
        if report(name, a, b, verbose):
            ok += 1
        else:
            bad += 1
    return ok, bad


def main():
    verbose = "-v" in sys.argv
    fr_count = 300
    if "--exe" in sys.argv:
        global EXE
        EXE = str(pathlib.Path(sys.argv[sys.argv.index("--exe") + 1]).resolve())
    if "--fr" in sys.argv:
        fr_count = int(sys.argv[sys.argv.index("--fr") + 1])
    alle = ["tokens", "pre", "ast", "projekt", "defs", "pruefe"]
    parts = [a for a in sys.argv[1:] if a in alle] or alle
    if not RPPC.exists():
        sys.exit(f"{RPPC} fehlt; zuerst in rppc/ cargo build ausführen")
    total = {}
    with tempfile.TemporaryDirectory() as tmp:
        cases = {"tokens": LEXER_FAELLE, "pre": PRE_FAELLE, "ast": AST_FAELLE}
        for part in parts:
            ok = bad = 0
            print(f"{part}:")
            if part == "projekt":
                ok, bad = projekt(pathlib.Path(tmp), verbose)
                total[part] = (ok, bad)
                continue
            if part == "defs":
                ok, bad = defs(pathlib.Path(tmp), verbose)
                total[part] = (ok, bad)
                continue
            if part == "pruefe":
                ok, bad = pruefe(pathlib.Path(tmp), verbose, fr_count)
                total[part] = (ok, bad)
                continue
            inputs = [(str(f.relative_to(ROOT)).replace("\\", "/"), f) for f in files()]
            for name, text in cases[part].items():
                f = pathlib.Path(tmp) / f"{part}_{name}.rpp"
                f.write_bytes(text.encode("utf-8"))
                inputs.append((f"Grenzfall {name}", f))
            for name, f in inputs:
                variants = [[str(f)]] if part == "tokens" else [[s, str(f)] for s in SYMBOLE]
                for args in variants:
                    label = name if part == "tokens" else f"{name} [{args[0]}]"
                    a, b = both(part, args)
                    if report(label, a, b, verbose):
                        ok += 1
                    else:
                        bad += 1
            total[part] = (ok, bad)
    print()
    for part, (ok, bad) in total.items():
        print(f"{part:8} gleich: {ok}, verschieden: {bad}")
    sys.exit(1 if any(bad for _, bad in total.values()) else 0)


if __name__ == "__main__":
    main()
