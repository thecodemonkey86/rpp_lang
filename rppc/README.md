# rppc – erste Implementierung von R++

Rust-Workspace der ersten R++-Implementierung (siehe „R++ Implementierungsplan“). Stand: Meilenstein M1, die Syntax.

## Inhalt

| Pfad | Zweck |
|---|---|
| `crates/rpp-syntax/src/preprocess.rs` | bedingte Kompilierung (`#if` … `#endif`), wie `rpp-grammatik/tools/preprocess.py` |
| `crates/rpp-syntax/src/lexer.rs` | Lexer nach den Lexer-Regeln von `RPP.g4` |
| `crates/rpp-syntax/src/ast.rs` | abstrakter Syntaxbaum |
| `crates/rpp-syntax/src/parser.rs` | Parser: rekursiver Abstieg, Vorrangparser für Ausdrücke, Rücksetzen wo ANTLR mit voller Vorausschau entscheidet |
| `crates/rpp-syntax/src/lib.rs` | Einstieg `rpp_syntax::parse`, Prüfung der Ausdrucksanweisungen (R-8.12), Unit-Tests zum Vorrang |
| `crates/rppc/src/main.rs` | Kommandozeile |
| `crates/rppc/src/syntaxtest.rs` | Syntax-Testlauf mit denselben Fällen wie `rpp-grammatik/tools/run_tests.py` |
| `testbericht.txt` | Ausgabe des letzten Syntax-Testlaufs |

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

Einzelne Dateien prüfen:

```
cargo run -- parse --symbols debug,linux DATEI.rpp
```

## Gleichwertigkeit mit der Grammatik

Der Python-Testlauf (`rpp-grammatik/tools/run_tests.py`) prüft dieselben Fälle mit dem aus `RPP.g4` erzeugten ANTLR-Parser. Zusätzlich vergleicht der Differenztest beide Parser an zufällig mutierten Programmen:

```
cd ..
py rpp-grammatik/tools/difftest.py 3000 2
```

Stand 21.11.2026: 9 600 Mutanten mit drei Seeds, kein Unterschied in Annahme oder Ablehnung, die erste Fehlerzeile stimmt in allen abgelehnten Fällen überein.
