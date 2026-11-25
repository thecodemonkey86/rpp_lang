# rpp-compiler – der R++-Compiler in R++

Meilenstein M6 (siehe „R++ Implementierungsplan“): Der Compiler wird Schritt für Schritt in R++ geschrieben, mit der ersten Implementierung `rppc` ausgeführt und mit ihr verglichen, bis er sich selbst übersetzt.

Stand 22.11.2026: die ganze Prüfung (Lexer, Präprozessor, Parser, Projekte, Deklarationen, Rümpfe mit Flussanalyse) und ein Codegenerator nach C (`c`).

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
| `src/compiler/sema/Checker.rpp` | Prüfung der Rümpfe: Anweisungen, Ausdrücke, Aufrufe, Lambdas, Eingebautes, Flussanalyse; gleichwertig zu `body.rs`, dazu die Angaben für die Ausführung |
| `src/compiler/sema/Rt.rpp` | Angaben für die Ausführung je Knoten (wie `rt.rs`): Typen, Zieltypen, Moves, Aufrufziele, Erfassungen der Lambdas, Basiskonstruktoren, Typen der Deklarationen |
| `src/compiler/gen/CGen.rpp` | Codegenerator nach C: setzt die Auswertung des Debug-Interpreters von `rppc` (`rpp-interp`) Stelle für Stelle in C um |
| `runtime/rpp_rt.h`, `runtime/rpp_rt.c` | Laufzeit für den erzeugten Code (C17, MSVC und gcc): Werte mit Besitzform wie `value.rs`, Freigabe, Exceptions und Programmfehler, Zahlen beider Modi, Strings, Vektoren, eingebaute Teile der Standardbibliothek |
| `src/main.rpp` | Befehle `tokens`, `pre`, `ast`, `project`, `defs`, `diagnose`, `c` |
| `vergleich.py` | Vergleich mit `rppc` |
| `vergleich_c.py`, `tests_c/` | Vergleich der übersetzten Programme mit `rppc run` |

Der Syntaxbaum ist einheitlich aufgebaut statt mit einer Klasse je Knotenart. Seine Textform (ein Knoten je Zeile mit Art, Text und Position) erzeugt auf der Rust-Seite `rppc/crates/rpp-syntax/src/dump.rs` aus dem AST von `rppc`.

## Ausführen

```
cd ../rppc
cargo run -- run ../rpp-compiler -- tokens FILE.rpp
cargo run -- run ../rpp-compiler -- pre debug,linux FILE.rpp
cargo run -- run ../rpp-compiler -- ast debug,linux FILE.rpp
cargo run -- run ../rpp-compiler -- project [--release] STD PATH
cargo run -- run ../rpp-compiler -- defs STD PATH
cargo run -- run ../rpp-compiler -- diagnose [--timings] STD PATH
cargo run -- run ../rpp-compiler -- c [--release] STD PATH OUTPUT.c
cargo run -- tokens FILE.rpp          # dieselben Ausgaben von rppc
cargo run -- pre debug,linux FILE.rpp
cargo run -- ast debug,linux FILE.rpp
cargo run -- project [--release] std PFAD
cargo run -- defs std PFAD
cargo run -- diagnose std PFAD
```

## Vergleich

`rppcc diagnose --timings STD PATH` gibt die Dauer von Projektladen, Deklarationsprüfung und Rumpfprüfung in drei Zeilen auf der Fehlerausgabe aus. Die Werte sind ganze Millisekunden. Sortierung und Ausgabe der Diagnosen liegen außerhalb der Messung. Ohne `--timings` erscheinen nur die regulären Diagnosen.

`rppcc project --help` und `rppcc tokens --help` zeigen die jeweilige Hilfe auf der Standardausgabe (Exit-Code 0). Bei fehlenden oder überzähligen Argumenten und unbekannten Optionen erscheinen Meldung und Aufruf auf der Fehlerausgabe (Exit-Code 2). Mit `rppcc tokens -- -beispiel.rpp` lassen sich auch Dateinamen mit führendem Minus übergeben.

`project` zeigt Haupt- und Quelldateien relativ zum Verzeichnis ihres jeweiligen Projekts an. Dateipositionen in Meldungen von `project`, `defs` und `diagnose` beziehen sich auf das Eingabeprojekt, bei Einzeldateien auf deren Verzeichnis. Für Dateien außerhalb dieser Basis erscheint der absolute Pfad.

```
py -X utf8 vergleich.py                # alle Teile; -v zeigt jeden Fall; tokens|pre|ast|project|defs|diagnose wählt Teile; --fr N: Anzahl der FR++-Zufallsprogramme
```

Eingaben sind alle `.rpp`-Dateien des Projekts (Konformitätstests, Grammatiktests einschließlich der negativen Tests und Lesarten, Standardbibliothek, `rpp-compiler` selbst) und Grenzfälle je Teil; `pre` und `ast` laufen mit den Symbolsätzen `debug,linux` und `release,windows`. `project` lädt alle Projektverzeichnisse und Einzeldateien der Konformitätstests, `rpp-compiler` selbst und 39 Grenzfälle für `rpp.yaml` im Debug- und Releasemodus; die Ausgabe nennt Projekte, Dateien mit deklariertem und erwartetem Paket und Typnamen sowie die Meldungen. `defs` läuft über dieselben Eingaben, die Grammatiktests und 17 Grenzfälle für Deklarationsregeln; die Ausgabe nennt alle Typen mit ihren Membern und Signaturen und die Meldungen der Deklarationsphase. `diagnose` vergleicht alle Meldungen der ganzen Prüfung über dieselben Eingaben ohne `rpp-compiler` selbst (im Interpreter zu langsam) und über die nach R++ übersetzten FR++-Programme.

Stand 22.11.2026:

| Teil | gleich | verschieden | Gegenprobe |
|---|---|---|---|
| `tokens` | 521 | 0 | ohne `??` in der Satzzeichenliste: 18 Abweichungen |
| `pre` | 1056 | 0 | – |
| `ast` (Baum oder erster Fehler mit Position) | 1036 | 0 | `+`/`-` stärker als `*`: 10 Abweichungen |
| `project` | 592 | 0 | `name:app` ohne Leerzeichen angenommen: 1 Abweichung |
| `defs` | 501 | 0 | `protected` mit dem Rang von `public`: 1 Abweichung |
| `diagnose` | 782 | 0 | R-6.9 statt R-6.8 nach `if`: 1 Abweichung |

Beim Aufbau des Parsers ist ein Fehler im Interpreter aufgefallen und behoben: Ein `own`-Argument, das über `??`, `?:` oder Klammern kam, wurde geliehen statt verschoben (neuer Laufzeittest `kap05_ownership/laufzeit/l02_own_argument_ausdruck.rpp`).

## Codegenerator nach C

`rppcc c [--release] STD PATH OUTPUT.c` prüft das Programm einschließlich der Standardbibliothek und erzeugt eine C-Datei; sie wird zusammen mit `runtime/rpp_rt.c` übersetzt:

```
cl /std:c17 /O2 /I runtime OUTPUT.c runtime/rpp_rt.c /link /STACK:16777216
gcc -std=c17 -O2 -I runtime OUTPUT.c runtime/rpp_rt.c -lm
```

Der Codegenerator folgt dem Debug-Interpreter von `rppc`, der Referenz nach R-12.3: Jeder Ausdruck wird zu C-Code, der dieselben Schritte in derselben Reihenfolge ausführt (`eval`, `eval_store`, `transfer`, `temp`, `convert`, `args`). Ein Wert (`rv`) trägt wie `Value` im Interpreter seine Art und bei Objekten die Besitzform am Verweis; Freigabe, Destruktoren (V-7), Sperren (V-1), Unwinding mit `finally`, Programmfehler, Stacktrace und Exit-Codes (R-12.9) verhalten sich wie dort. Fehler laufen als Status durch die Aufrufe; jede Anweisung und jeder Gültigkeitsbereich hat eine Aufräummarke. Generics werden nicht vervielfältigt; Typargumente stehen wie im Interpreter am Objekt (R-7.26). Der Modus (debug oder release) wird beim Erzeugen gewählt.

Die Angaben, die der Interpreter aus `Program::rt` liest, erhebt `Checker.rpp` an denselben Stellen wie `body.rs` (`Rt.rpp`); die Knoten des Syntaxbaums sind dafür nummeriert (`Node.id`).

Vergleich: `py -X utf8 vergleich_c.py [--fr N] [--only-fr] [--gcc] [--exe RPPCC] [--cflags "…"] [PATTERN]` erzeugt, übersetzt (MSVC; mit `--gcc` gcc) und startet jeden Laufzeittest der Konformitätstests und jedes Programm in `tests_c/` in beiden Modi in einem frischen Arbeitsverzeichnis und vergleicht Ausgabe, Fehlerausgabe und Exit-Code mit `rppc run`; mit `--fr N` auch die nach R++ übersetzten FR++-Programme. `--only-fr` beschränkt den Vergleich auf FR++-Programme.

Unicode-Tabellen (`runtime/rpp_unicode.h`, `rpp_unicode_ign.h`) entstehen mit `runtime/gen_unicode.rs` aus der Standardbibliothek von Rust und mit `gen_unicode_ignorable.py`; damit verhalten sich `char` und Strings wie im Interpreter. Texte von Betriebssystemfehlern in `IOException` kommen aus der C-Bibliothek (nach V-13.12 nicht festgelegt). Pfade und Kommandozeilenargumente sind UTF-8; unter Windows gehen Dateizugriffe über die `wchar_t`-Fassungen.

Mit `--exe` erzeugt ein übersetzter `rppcc` den C-Code statt des Interpreters; `--cflags "-DRPP_NATCHECK"` übersetzt so, dass jeder native Lesezugriff des Generators (M7) die Art des Werts prüft.

Native Grundwerte (M7): Reine Ausdrücke über `i8`–`i64`, `char` und `bool` ohne Aufrufe wertet der Generator direkt in C als `int64_t` aus; der schnelle Fall steht in `rpp_rt.h` (`rn_…`), alle anderen Fälle gehen über die rv-Pfade wie im Interpreter. Lokale Variablen und Parameter dieser Typen sind C-Variablen (`int64_t x…`), außer ein Lambda erfasst sie oder `take`/`replace` bzw. eine Tupelzuweisung braucht sie als Ort. Kleine Blöcke verwaltet die Laufzeit selbst (Freilisten je Größenklasse, `xmalloc`/`xfree`).

Umgebungsvariablen der Laufzeit: `RPP_NOFREE=1` gibt keinen Speicher frei (zum Eingrenzen von Fehlern bei der Freigabe); `RPP_PROFILE=DATEI` schreibt jede Millisekunde den Befehlszähler und bis zu sieben Aufrufer als Adressen relativ zum Modul, eine Stichprobe je Zeile (Windows; auflösen mit der Map-Datei des Linkers, `/MAP`); `RPP_MAINTHREAD=1` läuft ohne eigenen Thread mit großem Stack.

Geschwindigkeit (25.11.2026, übersetzter `rppcc.exe`, MSVC `/O2`): `diagnose rpp-compiler` 1,4 s, Selbstübersetzung 1,7 s (22.11.2026: 9,2 s und 10,4 s); im Interpreter jeweils rund 20 Minuten, `rppc diagnose` (Release) 0,54 s.
