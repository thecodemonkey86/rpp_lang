# rpp-compiler – der R++-Compiler in R++

Meilenstein M6 (siehe „R++ Implementierungsplan“): Der Compiler wird Schritt für Schritt in R++ geschrieben, mit der ersten Implementierung `rppc` ausgeführt und mit ihr verglichen, bis er sich selbst übersetzt.

Stand 22.11.2026: die ganze Prüfung (Lexer, Präprozessor, Parser, Projekte, Deklarationen, Rümpfe mit Flussanalyse).

## Inhalt

| Pfad | Zweck |
|---|---|
| `rpp.yaml` | Projekt `rppcc` (ausführbar) |
| `src/compiler/lexer/` | Token-Arten, Tokens und Lexer, gleichwertig zu `rppc/crates/rpp-syntax/src/lexer.rs` |
| `src/compiler/pre/` | bedingte Kompilierung (R-11.23 bis R-11.25), gleichwertig zu `preprocess.rs` |
| `src/compiler/syntax/Node.rpp` | Knoten des Syntaxbaums (Art, Text, Position, Kinder) und seine Textform |
| `src/compiler/syntax/Parser.rpp` | Parser (rekursiver Abstieg, Vorrangparser, Rücksetzen über eine Exception), gleichwertig zu `parser.rs`, mit der Prüfung der Ausdrucksanweisungen (R-8.12) |
| `src/compiler/syntax/Source.rpp` | eine Quelldatei lesen (Präprozessor, Lexer, Parser, R-8.12); der erste Fehler als `SyntaxError` |
| `src/compiler/project/Yaml.rpp` | der YAML-Teil aus R-11.13, gleichwertig zu `yaml.rs` |
| `src/compiler/project/Project.rpp` | Projekte, Quellordner, lokale Abhängigkeiten, Standardbibliothek (R-11.13, R-11.15 bis R-11.17), gleichwertig zu `project.rs` |
| `src/compiler/sema/Ty.rpp` | semantische Typen (unveränderlich, als `shared Ty` geteilt), gleichwertig zu `ty.rs` |
| `src/compiler/sema/Defs.rpp` | Deklarationstabellen: Typen, Felder, Methoden, Konstruktoren, Varianten |
| `src/compiler/sema/Program.rpp` | Deklarationsphase: Typen sammeln, Namen auflösen, Membertabellen, Constraints, Regeln auf Ebene von Dateien und Klassen; gleichwertig zu `Program::build` in `defs.rs` |
| `src/compiler/sema/Flow.rpp` | Zustand der Flussanalyse: Verengung, Initialisierung, Moves, Orte und Loans; gleichwertig zu `flow.rs` |
| `src/compiler/sema/Checker.rpp` | Prüfung der Rümpfe: Anweisungen, Ausdrücke, Aufrufe, Lambdas, Eingebautes, Flussanalyse; gleichwertig zu `body.rs` (ohne die Angaben für die Ausführung) |
| `src/main.rpp` | Befehle `tokens`, `pre`, `ast`, `projekt`, `defs`, `pruefe` |
| `vergleich.py` | Vergleich mit `rppc` |

Der Syntaxbaum ist einheitlich aufgebaut statt mit einer Klasse je Knotenart. Seine Textform (ein Knoten je Zeile mit Art, Text und Position) erzeugt auf der Rust-Seite `rppc/crates/rpp-syntax/src/dump.rs` aus dem AST von `rppc`.

## Ausführen

```
cd ../rppc
cargo run -- run ../rpp-compiler -- tokens DATEI.rpp
cargo run -- run ../rpp-compiler -- pre debug,linux DATEI.rpp
cargo run -- run ../rpp-compiler -- ast debug,linux DATEI.rpp
cargo run -- run ../rpp-compiler -- projekt [--release] STD PFAD
cargo run -- run ../rpp-compiler -- defs STD PFAD
cargo run -- run ../rpp-compiler -- pruefe STD PFAD
cargo run -- tokens DATEI.rpp          # dieselben Ausgaben von rppc
cargo run -- pre debug,linux DATEI.rpp
cargo run -- ast debug,linux DATEI.rpp
cargo run -- projekt [--release] std PFAD
cargo run -- defs std PFAD
cargo run -- pruefe std PFAD
```

## Vergleich

```
py -X utf8 vergleich.py                # alle Teile; -v zeigt jeden Fall; tokens|pre|ast|projekt|defs|pruefe wählt Teile; --fr N: Anzahl der FR++-Zufallsprogramme
```

Eingaben sind alle `.rpp`-Dateien des Projekts (Konformitätstests, Grammatiktests einschließlich der negativen Tests und Lesarten, Standardbibliothek, `rpp-compiler` selbst) und Grenzfälle je Teil; `pre` und `ast` laufen mit den Symbolsätzen `debug,linux` und `release,windows`. `projekt` lädt alle Projektverzeichnisse und Einzeldateien der Konformitätstests, `rpp-compiler` selbst und 39 Grenzfälle für `rpp.yaml` im Debug- und Releasemodus; die Ausgabe nennt Projekte, Dateien mit deklariertem und erwartetem Paket und Typnamen sowie die Meldungen. `defs` läuft über dieselben Eingaben, die Grammatiktests und 17 Grenzfälle für Deklarationsregeln; die Ausgabe nennt alle Typen mit ihren Membern und Signaturen und die Meldungen der Deklarationsphase. `pruefe` vergleicht alle Meldungen der ganzen Prüfung über dieselben Eingaben ohne `rpp-compiler` selbst (im Interpreter zu langsam) und über die nach R++ übersetzten FR++-Programme.

Stand 22.11.2026:

| Teil | gleich | verschieden | Gegenprobe |
|---|---|---|---|
| `tokens` | 521 | 0 | ohne `??` in der Satzzeichenliste: 18 Abweichungen |
| `pre` | 1056 | 0 | – |
| `ast` (Baum oder erster Fehler mit Position) | 1036 | 0 | `+`/`-` stärker als `*`: 10 Abweichungen |
| `projekt` | 592 | 0 | `name:app` ohne Leerzeichen angenommen: 1 Abweichung |
| `defs` | 501 | 0 | `protected` mit dem Rang von `public`: 1 Abweichung |
| `pruefe` | 782 | 0 | R-6.9 statt R-6.8 nach `if`: 1 Abweichung |

Beim Aufbau des Parsers ist ein Fehler im Interpreter aufgefallen und behoben: Ein `own`-Argument, das über `??`, `?:` oder Klammern kam, wurde geliehen statt verschoben (neuer Laufzeittest `kap05_ownership/laufzeit/l02_own_argument_ausdruck.rpp`).
