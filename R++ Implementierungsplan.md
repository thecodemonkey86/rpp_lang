# R++ Implementierungsplan

Stand: 21.11.2026, Entwurf 0.2. Dieses Dokument beschreibt die erste Implementierung von R++: Wirtssprache, Aufbau, Prüfstrategie und Meilensteine. Es ist kein Teil der Spezifikation; maßgeblich bleiben die Kapitel 1 bis 14 und die Grammatik `RPP.g4`.

Wirtssprache, Aufbau und Meilensteine sind entschieden (Designentscheidungen, Abschnitt 27). Meilenstein M1 ist abgeschlossen (Abschnitt 6).

## 1. Rolle der ersten Implementierung

Die erste Implementierung hat drei Aufgaben:

1. **Referenz.** Ihr Debug-Interpreter ist die Referenzimplementierung (R-12.3). Lesbarkeit und Nähe zur Spezifikation gehen vor Geschwindigkeit.
2. **Konformitätstests ausführen.** Erst mit ihr werden die 242 Konformitätstests semantisch geprüft, nicht nur syntaktisch.
3. **Bootstrap.** Sie übersetzt den späteren R++-Compiler, bis dieser sich selbst übersetzt. Deshalb muss der Debugmodus schnell genug sein, um einen Compiler auszuführen.

## 2. Wirtssprache: Rust

| Gesichtspunkt | Warum Rust |
|---|---|
| Sicherheitsanspruch | Ein Compiler für eine speichersichere Sprache sollte selbst speichersicher sein. |
| Compilerbau | `enum` und `match` passen zu AST, Typen und Zwischenrepräsentation; Erschöpfungsprüfung findet vergessene Fälle. |
| Zahlen | `wrapping_*`, `checked_*` und `saturating_*` bilden Überlauf und `cast` beider Modi direkt ab (R-4.9, R-4.10, R-4.14). `f32`/`f64` sind IEEE 754, Rust fasst ohne ausdrückliche Anweisung nicht zu FMA zusammen (R-12.7). |
| Text | `char` ist ein Unicode-Skalarwert, `String` ist UTF-8, wie in R++ (R-4.4, R-4.18). |
| JIT und Release | Cranelift für einen späteren JIT im Debugmodus, LLVM (über `inkwell`) für den Releasemodus, beide aus Rust nutzbar. |
| Laufzeit | Kein Garbage Collector, der mit dem Besitzmodell von R++ konkurriert. |

Rust hat kein offizielles ANTLR-Ziel. Der Parser wird deshalb von Hand geschrieben (Abschnitt 3.1). `RPP.g4` bleibt normativ; der Python-Testlauf prüft weiter die Grammatik selbst.

Installiert ist Rust 1.83. Für die Arbeit genügt das; eine neuere stabile Version kann später mit `rustup update` folgen.

## 3. Aufbau

Ein Cargo-Workspace im Verzeichnis `rppc/`:

| Crate | Aufgabe | Kapitel |
|---|---|---|
| `rpp-syntax` | Präprozessor, Lexer, Parser, AST mit Quellpositionen | 2, 3, 11 (Direktiven) |
| `rpp-project` | `rpp.yaml`, `rpp.lock`, Pakete, Abhängigkeiten, Namensauflösung | 11 |
| `rpp-sema` | Typprüfung, Überladung, Generics, Lambdas, Null-Flussanalyse, Ownership und Borrows, Warnungen | 4–8, 10 |
| `rpp-ir` | gemeinsame Zwischenrepräsentation beider Modi (R-12.2) | 5, 9, 12 |
| `rpp-interp` | Debug-Interpreter über der Zwischenrepräsentation | 9, 12 |
| `rpp-std` | eingebaute Teile der Standardbibliothek (Ausgabe, Dateien, Umgebung); der Rest ist R++-Quelltext | 13 |
| `rppc` | Kommandozeile: `rppc check`, `rppc run`, `rppc test` | – |

Später kommen `rpp-jit` (Cranelift) und `rpp-llvm` (Releasemodus) hinzu. Fremde Crates nur, wo sie viel Arbeit sparen; den eingeschränkten YAML-Teil aus R-11.13 liest ein eigener kleiner Parser.

### 3.1 Parser

Rekursiver Abstieg für Deklarationen und Anweisungen, Vorrangparser (Pratt) für Ausdrücke nach der Tabelle in Gerüst 3.2. Die geprüften Lesarten aus Gerüst 3.3 (`a < b > c;` ist eine Deklaration, `(a).run();` ein Ausdruck usw.) werden durch begrenztes Vorausschauen mit Rücksetzen entschieden, wie ANTLR es tut.

Die Testdateien in `rpp-grammatik/tests/pos`, `tests/neg` und `tests/lesarten` gelten für beide Parser; beide Testläufe lesen dieselben Dateien. Wo mehrere Alternativen scheitern, meldet der Parser den Fehler, der am weitesten im Quelltext liegt.

### 3.2 Semantische Prüfung

Die Prüfungen laufen auf einer aufgelösten Fassung des AST (Namen gebunden, Typen bekannt). Die Ownership-Prüfung folgt dem Kernkalkül aus Kapitel 5: Umgebungen mit Verfügbarkeit, Loan-Mengen, Zusammenführen an Verzweigungen und Fixpunkt für Schleifen, so wie `fr-modell/frpp/checker.py`. Die Null-Prüfung (Kapitel 6) teilt sich mit ihr die Flussanalyse.

Jeder Fehler nennt Datei, Zeile und Regelnummer (R-1.5, R-14.7).

### 3.3 Zwischenrepräsentation und Interpreter

Die Zwischenrepräsentation ist ein Kontrollflussgraph mit ausdrücklichen Anweisungen für alles, was die Spezifikation an Laufzeitverhalten festlegt: Freigabe und Drop-Flags (V-5, V-7), Sperren für `shared`-Objekte (V-1), Unwinding und `finally` (Kapitel 9), Prüfungen auf Index, Überlauf und `cast` mit der Angabe, in welchem Modus sie gelten (R-12.5).

Der Interpreter führt diese Anweisungen direkt aus. In einem **Prüfmodus** kontrolliert er zusätzlich die Invarianten I-1 bis I-5 aus Kapitel 5, wie das FR++-Modell. Damit wird ein Fehler in der Ownership-Prüfung als interner Fehler sichtbar statt als falsches Verhalten.

## 4. Prüfstrategie

| Prüfung | Womit |
|---|---|
| Parser gleichwertig zur Grammatik | positive und negative Grammatiktests, Lesarten (Abschnitt 3.1) |
| Konformität | `rppc test konformitaet/` liest die Kopfzeilen nach R-14.3 bis R-14.5: Fehler mit Regel und Zeile, Warnungen, Ausgabe, Exit-Code, Fehlerausgabe, Argumente und Eingabe |
| Ownership | die FR++-Tests und die Zufallsprogramme aus `fr-modell/fuzz.py`, übersetzt nach R++; beide Prüfer müssen gleich urteilen |
| Laufzeit | Prüfmodus des Interpreters bei allen Tests |

## 5. Meilensteine

| Nr. | Inhalt | Fertig, wenn |
|---|---|---|
| M1 | Präprozessor, Lexer, Parser, AST | alle Grammatiktests und Konformitätstests werden wie vom ANTLR-Parser gelesen |
| M2 | Projekte und Pakete, Namensauflösung, Typprüfung ohne Ownership und Null-Prüfung | die abgelehnten Tests der Kapitel 4, 7, 8, 10, 11 und 13 werden mit der richtigen Regel abgelehnt, alle akzeptierten übersetzt |
| M3 | Null-Flussanalyse, Ownership und Borrows, Warnungen | auch die abgelehnten Tests der Kapitel 5, 6 und 9; FR++-Tests und Zufallsprogramme stimmen überein |
| M4 | Zwischenrepräsentation, Interpreter, Exceptions und Programmfehler | alle Laufzeittests mit `print` und `rpp.core` bestehen |
| M5 | Standardbibliothek für den Compiler: `rpp.text`, `rpp.collections`, `rpp.io`, `rpp.env`, `rpp.fmt` | alle Konformitätstests bestehen im Debugmodus |
| M6 | Beginn des R++-Compilers in R++ | – |

JIT und Releasemodus folgen nach M5. Bis dahin ist die Implementierung nur für den Debugmodus konform (R-14.7).

## 6. Stand

**M1 abgeschlossen (21.11.2026).** `rppc/crates/rpp-syntax` enthält Präprozessor, Lexer, AST und Parser, ohne fremde Crates.

| Prüfung | Ergebnis |
|---|---|
| `cargo run -- syntaxtest ..` | 18 positive Dateien (22 Symbolsätze), 127 negative Tests, 80 Lesarten, 242 Konformitätstests: 0 Fehler |
| `cargo test` | Unit-Tests für Präprozessor, Lexer und Vorrang der Operatoren |
| `rpp-grammatik/tools/difftest.py` | 9 600 mutierte Programme (Seeds 2 bis 4): kein Unterschied zum ANTLR-Parser in Annahme oder Ablehnung; erste Fehlerzeile in allen abgelehnten Fällen gleich |

Als Nächstes folgt M2: Projekte und Pakete, Namensauflösung und Typprüfung.

