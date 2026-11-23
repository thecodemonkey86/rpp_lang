# rppc – erste Implementierung von R++

Rust-Workspace der ersten R++-Implementierung (siehe „R++ Implementierungsplan“). Stand: Meilensteine M1 (Syntax), M2 (Namen und Typen), M3 (Flussanalyse; Vergleich mit dem FR++-Modell) M4 (Debug-Interpreter mit Prüfmodus) und M5 (eingebaute Standardbibliothek in `crates/rpp-interp/src/stdlib.rs`, Typargumente zur Laufzeit).

## Inhalt

| Pfad | Zweck |
|---|---|
| `crates/rpp-syntax/src/preprocess.rs` | bedingte Kompilierung (`#if` … `#endif`), wie `rpp-grammatik/tools/preprocess.py` |
| `crates/rpp-syntax/src/lexer.rs` | Lexer nach den Lexer-Regeln von `RPP.g4` |
| `crates/rpp-syntax/src/ast.rs` | abstrakter Syntaxbaum |
| `crates/rpp-syntax/src/parser.rs` | Parser: rekursiver Abstieg, Vorrangparser für Ausdrücke, Rücksetzen wo ANTLR mit voller Vorausschau entscheidet |
| `crates/rpp-syntax/src/lib.rs` | Einstieg `rpp_syntax::parse`, Prüfung der Ausdrucksanweisungen (R-8.12), Unit-Tests zum Vorrang |
| `crates/rpp-sema/src/` | Projekte (`project.rs`, `yaml.rs`), Deklarationen und Klassenregeln (`defs.rs`), Typen (`ty.rs`), Rümpfe mit Flussanalyse (`body.rs`), Flusszustand, Orte und Loans (`flow.rs`) |
| `crates/rpp-sema/src/rt.rs` | Angaben der Prüfung für die Ausführung (`Program::rt`) |
| `crates/rpp-interp/src/` | Debug-Interpreter: Werte und Besitzformen (`value.rs`), Ausführung, Freigabe, Exceptions, Sperren (`lib.rs`) |
| `std/rpp/…` | Standardbibliothek als R++-Quelltext (Kapitel 13); eingebaute Teile führen die Methoden mit `fail("eingebaut")` im Interpreter aus (`crates/rpp-interp/src/stdlib.rs`), `rpp.random`, `rpp.collections.Vectors`, `rpp.collections.Deque`, `rpp.text.Text` und der Rundungs-Teil von `rpp.math` sind reiner R++-Quelltext |
| `crates/rppc/src/conftest.rs` | semantischer Konformitätstestlauf `rppc test` |
| `crates/rppc/src/main.rs` | Kommandozeile |
| `crates/rppc/src/syntaxtest.rs` | Syntax-Testlauf mit denselben Fällen wie `rpp-grammatik/tools/run_tests.py` |
| `testbericht.txt` | Ausgabe des letzten Syntax-Testlaufs |
| `testlauf.txt` | Ausgabe des letzten semantischen Testlaufs |

## Bauen und testen

Voraussetzung: Rust (getestet mit 1.83). Es gibt keine fremden Crates.

```
cargo build
cargo test
cargo run -- syntaxtest ..
```

`syntaxtest` liest aus dem Projektverzeichnis (`..`):

- `rpp-grammatik/tests/pos/*.rpp` – müssen akzeptiert werden, teils mit mehreren Symbolsätzen
- `rpp-grammatik/tests/neg/*.rpp` – müssen abgelehnt werden
- `rpp-grammatik/tests/lesarten/*.rpp` – die erste Anweisung muss als die angegebene Grammatikregel gelesen werden
- `konformitaet/**` – müssen syntaktisch gültig sein; ein abgelehnter Test darf nur in der mit `FEHLER` markierten Zeile scheitern

Einzelne Dateien parsen oder prüfen:

```
cargo run -- parse --symbols debug,linux DATEI.rpp
cargo run -- check DATEI.rpp|PROJEKTVERZEICHNIS
```

Konformitätstests semantisch prüfen (übersetzen oder ablehnen, Warnungen nur in `WARNUNG`-Zeilen; `-v` zeigt jeden Test):

```
cargo run -- test ..
```

Laufzeittests werden dabei in beiden Modi und im Prüfmodus ausgeführt (Ausgabe, Exit-Code, Fehlerausgabe). Jeder Laufzeittest läuft in einem frischen, leeren Arbeitsverzeichnis (V-14.3). Stand 23.11.2026: 293 bestanden, 0 fehlgeschlagen.

Ein Programm ausführen (Standardeingabe wird gelesen; Argumente nach `--`):

```
cargo run -- run DATEI.rpp|PROJEKTVERZEICHNIS -- eins zwei
cargo run -- run --release DATEI.rpp
cargo run -- run --pruefen DATEI.rpp      # Prüfmodus: Invarianten I-1, I-2, I-4, I-5 aus Kapitel 5
cargo run -- tokens DATEI.rpp              # Tokens, eine Zeile je Token (Vergleich mit rpp-compiler)
cargo run -- pre debug,linux DATEI.rpp     # bedingte Kompilierung angewendet
cargo run -- ast debug,linux DATEI.rpp     # Syntaxbaum in Textform (rpp-syntax/src/dump.rs)
cargo run -- projekt std PFAD              # geladene Projekte, Dateien, Meldungen (--release vor std)
cargo run -- defs std PFAD                 # Deklarationstabellen und Meldungen der Deklarationsphase
cargo run -- pruefe std PFAD               # alle Meldungen der Prüfung mit Warnungen und Begründungen
```

Eine Meldung nennt die verletzte Regel und, wo es hilft, die Regeln, die den Verstoß begründen:

```
n03_zuweisung_hebt_auf.rpp:26:8: Fehler [R-6.2, R-6.11] z hat den nullbaren Typ Node? und ist hier nicht geprüft
```

## Gleichwertigkeit mit der Grammatik

Der Python-Testlauf (`rpp-grammatik/tools/run_tests.py`) prüft dieselben Fälle mit dem aus `RPP.g4` erzeugten ANTLR-Parser. Zusätzlich vergleicht der Differenztest beide Parser an zufällig mutierten Programmen:

```
cd ..
py rpp-grammatik/tools/difftest.py 3000 2
```

Stand 21.11.2026: 9 600 Mutanten mit drei Seeds, kein Unterschied in Annahme oder Ablehnung, die erste Fehlerzeile stimmt in allen abgelehnten Fällen überein.

## Vergleich mit dem FR++-Modell

`fr-modell/vergleich.py` übersetzt FR++-Programme in R++ und prüft, ob `rppc` sie genauso annimmt oder ablehnt wie das Modell (braucht `target/debug/rppc`):

```
cd ../fr-modell
py -X utf8 vergleich.py 5000
```

Stand 21.11.2026: keine Abweichung bei allen übersetzbaren handgeschriebenen Programmen und 4466 übersetzbaren von 5000 Zufallsprogrammen (`fr-modell/vergleichsbericht.txt`).
