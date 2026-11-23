# R++ Implementierungsplan

Stand: 22.11.2026, Entwurf 0.6. Dieses Dokument beschreibt die erste Implementierung von R++: Wirtssprache, Aufbau, Prüfstrategie und Meilensteine. Es ist kein Teil der Spezifikation; maßgeblich bleiben die Kapitel 1 bis 14 und die Grammatik `RPP.g4`.

Wirtssprache, Aufbau und Meilensteine sind entschieden (Designentscheidungen, Abschnitt 27). Die Meilensteine M1 bis M6 sind abgeschlossen (Abschnitt 6); M7 ist die Geschwindigkeit des übersetzten Codes.

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

Geändert am 22.11.2026 (Abschnitt 6, M4): Der Debug-Interpreter führt zunächst den geprüften AST mit den Angaben der Prüfung aus (`Program::rt`); die Zwischenrepräsentation entsteht mit dem JIT bzw. dem Releasemodus und wird gegen diesen Interpreter getestet.

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
| M6 | Beginn des R++-Compilers in R++ | der R++-Compiler übersetzt sich selbst (Fixpunkt) und stimmt in allen Vergleichen mit `rppc` überein (festgelegt am 23.11.2026) |
| M7 | Geschwindigkeit des übersetzten Codes | – (Ziel und Weg des Releasemodus werden noch besprochen) |

JIT und Releasemodus folgen nach M5. Bis dahin ist die Implementierung nur für den Debugmodus konform (R-14.7).

## 6. Stand

**M1 abgeschlossen (21.11.2026).** `rppc/crates/rpp-syntax` enthält Präprozessor, Lexer, AST und Parser, ohne fremde Crates.

| Prüfung | Ergebnis |
|---|---|
| `cargo run -- syntaxtest ..` | 18 positive Dateien (22 Symbolsätze), 127 negative Tests, 80 Lesarten, 242 Konformitätstests: 0 Fehler |
| `cargo test` | Unit-Tests für Präprozessor, Lexer und Vorrang der Operatoren |
| `rpp-grammatik/tools/difftest.py` | 9 600 mutierte Programme (Seeds 2 bis 4): kein Unterschied zum ANTLR-Parser in Annahme oder Ablehnung; erste Fehlerzeile in allen abgelehnten Fällen gleich |

**M2 abgeschlossen (21.11.2026).** Neu sind `rppc/crates/rpp-sema` und die Standardbibliothek als Quelltext in `rppc/std/rpp/…`. Die Bibliothek enthält die Signaturen aus Kapitel 13; ihre Rümpfe sind bis M4/M5 Platzhalter.

| Teil | Inhalt |
|---|---|
| `project.rs`, `yaml.rs` | `rpp.yaml`, Quellordner, lokale Abhängigkeiten, Zyklen, Bibliotheksnamen (R-11.2, R-11.13, R-11.15 bis R-11.18) |
| `defs.rs` | Typen sammeln, Namen auflösen (R-11.6 bis R-11.12), `using`, `main`, `export`, Membertabellen, Klassen- und Mixin-Regeln (Kapitel 7), Enums (R-4.25, R-4.26), Constraints (R-7.25, R-7.26) |
| `body.rs` | Anweisungen und Ausdrücke: Zahlregeln und Literale (Kapitel 4), Überladung (R-7.17), generische Methoden (R-4.44), Lambdas (Kapitel 10), eingebaute Methoden (Kapitel 13), Rückgabe (R-8.26), switch (R-8.24) |

`rppc test ..` (Konformitätstests):

| Ergebnis | Anzahl |
|---|---|
| bestanden: alle akzeptierten und Laufzeittests übersetzen, die abgelehnten der Kapitel 4, 7, 8, 10, 11 und 13 werden in der markierten Zeile mit der erwarteten Regel abgelehnt | 192 |
| fehlgeschlagen | 0 |
| offen für M3: abgelehnte Tests zu Besitz und Borrows (Kapitel 5), Typverengung (Kapitel 6), sicherer Initialisierung (R-4.46) und den Ownership-Regeln für Lambdas und Exceptions | 50 |

Bis M3 behandelt die Prüfung nullbare Werte wie nicht nullbare (keine Verengung) und prüft keine Besitzformen.

Beim Schreiben von M2 sind Fehler in Tests aufgefallen und korrigiert worden:
- zwei Tests verwendeten die Klasse `Error` statt `Exception`;
- ein Test verwendete den nicht vorhandenen Typ `List`;
- zwei Tests aus Kapitel 7 definierten eigene Mixins `Equatable` und `Comparable` statt der aus `rpp.core`;
- in drei Kapitel-6-Tests blieb das Feld `Node?[] slots` ungesetzt, und ein Kapitel-5-Test rief `copy` für eine Klasse ohne `copy` auf;
- in n16 (Kapitel 4) verdeckte eine lokale Variable einen Parameter;
- die `using`-Tests n01 und n02 (Kapitel 11) verwiesen auf nicht vorhandene Pakete und sind jetzt Projekttests mit diesen Paketen;
- n19 (Kapitel 7) prüfte dasselbe wie n28 und prüft jetzt einen Mixin-Constraint;
- die Reihenfolge der Regeln in drei Testköpfen passt jetzt zur gemeldeten Regel.

**M3, erster Teil abgeschlossen (21.11.2026).** Die Prüfung der Rümpfe (`body.rs`) führt jetzt einen Flusszustand mit (`flow.rs`). Es gibt dafür keinen eigenen Durchlauf; der Zustand wird mitgeführt, während die Typen geprüft werden.

| Teil | Inhalt |
|---|---|
| Flusszustand | je Stelle: verengte Variablen (R-6.5), möglicherweise nicht zugewiesene (R-4.46), möglicherweise verschobene (R-5.3), dazu je Variable die Regel, die einen Verlust begründet. An Verzweigungen kopiert, danach vereinigt; Wege, die nicht normal enden, tragen nichts bei (V-12) |
| Schleifen | Fixpunkt am Schleifenkopf (R-6.9, R-5.16): Vorläufe ohne Meldungen, bis sich der Stand nicht mehr ändert, dann ein Lauf mit Meldungen; `break` und `continue` bringen ihre Stände mit |
| try | `catch` und `finally` sehen die Vereinigung aller Stände des `try`-Blocks (R-6.12, R-9.17) |
| Loans | Orte (Variable mit Feldpfad, `[]` für Elemente) und Loans `(κ, p)` wie in Kapitel 5, Abschnitt 3.2, mit Dauer Aufruf (Empfänger und Argumente, R-5.9, R-5.10), Anweisung (Slice, Borrow-Rückgabe, R-5.24) oder Block (`ref`, Schleife, lokaler Slice, R-5.17); shared-Wurzeln überlappen immer |
| Moves | Verwendung als Wert für einen Besitzer (Abschnitt 7 von Kapitel 5): Initialisierer, Zuweisung, `return`, `own`-Argument, `throw`, Tupel- und Array-Elemente, Enum-Daten, `own`-Lambda; aus Feldern, Elementen und Borrows wird nie verschoben (V-3) |
| Lambdas | Erfassungen werden gesammelt; ein normales Lambda leiht sie an der Stelle, an der es übergeben wird (R-10.7), ein `own`-Lambda verschiebt `own`-Werte in sich (R-10.8) und darf keine Borrows erfassen (R-10.9) oder Erfasstes verändern (R-10.11) |
| Null-Sicherheit | Fakten `W(c)`, `F(c)` (R-6.6) für `if`, `?:`, `&&`, `\|\|`, Schleifen und `assert`; Dereferenzierung, Arithmetik und Vergleiche verlangen verengte Werte (R-6.2, R-6.23) |
| Warnungen | unerreichbarer Code (R-8.27), `catch` hinter dem `catch` einer Oberklasse (R-9.7) |

Eine Meldung nennt die verletzte Regel und die Regeln, die den Verstoß begründen, etwa `[R-6.2, R-6.11]` für einen Zugriff nach `x = null;` (V-14.2).

`rppc test ..`: alle 242 Konformitätstests bestehen (übersetzen bzw. ablehnen, Warnungen genau in den markierten Zeilen); die Kategorie „offen“ gibt es nicht mehr.

Beim Umsetzen sind drei Klarstellungen entstanden, am 21.11.2026 angenommen: V-12 (Kapitel 5, Zweige ohne normales Ende), V-6.1 (Kapitel 6, deklarierter Typ für `?.`, `??` und `== null`) und V-14.2 (Kapitel 14, mehrere Regeln in einer Meldung). Außerdem sind Tests korrigiert worden, die gegen Kapitel 5 verstießen:
- in 13 abgelehnten Kapitel-6-Tests blieb das Feld `Node?[] slots` ungesetzt (R-7.7);
- `a03_felder.rpp` (Kapitel 6) rief `take(&n.child)`, während der Borrow `ref Node? c = n.child` noch bestand; der Borrow steht jetzt in einem eigenen Block (R-5.17);
- `a04_lambdas_generics.rpp` (Kapitel 6) verschob in `T? get() { return value; }` aus einem Feld; die Methode liefert jetzt `ref T?` (V-3, V-4);
- `a06_generics.rpp` (Kapitel 7) verschob in `Util::first` aus einem Element; die generische Methode ist jetzt `Util::count` (V-3);
- `n04_nach_weiterwerfen_benutzt.rpp` (Kapitel 9) war nach V-12 korrekt, weil der Zweig mit `throw e;` nicht normal endet; der Test prüft jetzt, dass die catch-Variable nach `Exception kept = e;` verschoben ist (R-9.8);
- `n05_loan_konflikt.rpp` (Kapitel 10) enthielt zusätzlich einen Typfehler (`i64` an `i32`).

Offen in M3: der Vergleich mit dem FR++-Modell. Die handgeschriebenen FR++-Programme (`fr-modell/tests/`) und Zufallsprogramme aus `fr-modell/frpp/gen.py` sollen nach Abschnitt 7 von Kapitel 5 in R++ übersetzt und von `rppc` genauso angenommen oder abgelehnt werden wie vom Modell.

**M3 abgeschlossen (21.11.2026).** Der Vergleich mit dem FR++-Modell (`fr-modell/vergleich.py`, Kapitel 5, Abschnitt 10.5) übersetzt FR++-Programme in R++ und vergleicht das Urteil. Ergebnis: alle 32 übersetzbaren handgeschriebenen Programme und 4466 übersetzbare von 5000 Zufallsprogrammen gleich beurteilt, keine Abweichung; die Gegenprobe mit abgeschalteten Modellregeln findet für jede Regel Abweichungen. Dabei wurden sechs Lücken in `rppc` gefunden und behoben (`&` am Argument, Ersetzen hinter `Foo&`, Move an `shared`-/`weak`-Parameter, Borrow-Rückgabe auf ein `shared`-Feld, Move einer Borrow-Rückgabe, Reihenfolge in `x.f = g(x)`). Die 242 Konformitätstests bestehen weiter.

V-13 (Kapitel 5) ist entschieden: Ein `own`-Wert an einem `weak`-Ziel ist ein Fehler (R-4.41); dazu der Test `kap05_ownership/abgelehnt/n21_own_an_weak.rpp`. Die Suite hat damit 243 Tests, alle bestehen.

**M4, erster Teil (22.11.2026).** Neu ist das Crate `rppc/crates/rpp-interp` (Debug-Interpreter) und der Befehl `rppc run [--release] DATEI|PROJEKT [-- ARGUMENTE]`.

| Teil | Inhalt |
|---|---|
| Angaben der Prüfung (`rpp-sema/src/rt.rs`) | je Ausdruck (Schlüssel: Datei und Nummer des Ausdrucks, neu im AST): statischer Typ, Zieltyp beim Speichern, ob der Wert an einen Besitzer geht (Move bzw. gezählte Kopie), Ziel jedes Aufrufs (gewählte Überladung, Konstruktor, eingebaute Funktion, Enum-Wert), Erfassungen von own-Lambdas, Basiskonstruktoren. Die Standardbibliothek wird dafür mitgeprüft. |
| Modus | Der Workspace lädt im Debug- oder Releasemodus (Symbol `debug` bzw. `release`, R-11.24). |
| Werte (`rpp-interp/src/value.rs`) | Objekte mit Besitzform am Verweis (`own`, `shared` mit Zähler, `weak`, Borrow), Vektoren, Tupel, Enums, Funktionswerte; Variablen als Zellen, die normale Lambdas erfassen |
| Freigabe | am Blockende in umgekehrter Reihenfolge, bei Zuweisung (alter Wert), für temporäre Werte am Ende der Anweisung; Destruktoren nach V-7 in der Lesart V-14; shared mit Referenzzähler, weak über `alive` |
| Konstruktion | Basisanteil, Feldinitialisierer, Rumpf; virtuelle Aufrufe gehen schon im Konstruktor an die Klasse des Objekts; wirft ein Konstruktor, wird nach R-9.13 aufgeräumt |
| Exceptions | Unwinding mit Freigabe und `finally`; Programmfehler (`IndexError`, `OverflowError`, `DivisionError`, `CastError`, `AssertError`, `FailError`, `DestructorError`, `StackOverflowError`, `BorrowError`) räumen ebenso auf, werden aber nicht gefangen (R-9.14); Stacktrace im Debugmodus |
| Zahlen | Typen exakt, Überlauf, Schieben und `cast` nach R-12.5 je Modus, Division durch null in beiden Modi ein Programmfehler |
| Sperren | V-1 wie im FR++-Modell: Orte mit shared-Wurzelvariable; Empfänger und geliehene Argumente für die Dauer des Aufrufs, `ref` und Schleifen bis zum Blockende |
| Ende | Exit-Code aus `main` nach Freigabe der Variablen, 1 bei einer Exception (`describe()` auf der Fehlerausgabe), 2 bei einem Programmfehler (R-12.9); mindestens 10 000 verschachtelte Aufrufe (Grenze 12 000) |

`rppc test ..` führt die Laufzeittests jetzt in beiden Modi aus: 241 bestanden, 0 fehlgeschlagen, 2 offen. Die zwei offenen Tests aus Kapitel 13 (`l01_text`, `l03_set_reihenfolge`) brauchen eingebaute Teile der Bibliothek (`StringBuilder`, `Set`), die nach dem Plan zu M5 gehören.

Erster Stand des Vergleichs mit dem FR++-Modell (`fr-modell/vergleich.py --ausfuehren`): Von 1000 Zufallsprogrammen werden 226 von beiden angenommen und ausgeführt. 212 liefern dieselbe Ausgabe einschließlich der Destruktoren, ebenso alle 10 handgeschriebenen Programme. Offen sind 14 Unterschiede (Kapitel 5, Abschnitt 10.5 und `fr-modell/vergleichsbericht.txt`): 13 endlose Destruktor-Kaskaden, bei denen das Modell mit `BorrowError` endet und `rppc` mit `StackOverflowError`, und ein ungeklärter Sperrkonflikt (seed772).

Änderung am Plan (angenommen am 22.11.2026): Abschnitt 3.3 sah eine Zwischenrepräsentation als Kontrollflussgraph vor, die der Interpreter ausführt. Der erste Teil von M4 führt stattdessen den geprüften AST zusammen mit den Angaben der Prüfung aus; Freigabe, Sperren, Unwinding und Prüfungen je Modus stehen ausdrücklich im Interpreter. Die Zwischenrepräsentation entsteht erst mit dem JIT bzw. dem Releasemodus, wenn sie für die Codeerzeugung gebraucht wird; bis dahin ist der AST-Interpreter die Referenz (R-12.3). Begründung: Alle Laufzeitregeln sind so schon prüfbar, und die Zwischenrepräsentation lässt sich später gegen diesen Interpreter testen.

Noch offen nach dem ersten Teil: der Prüfmodus für die Invarianten, `new T()` in generischen Rümpfen und die Iteration über `Iterable`-Klassen (mit M5).

**M4 abgeschlossen (22.11.2026).**

| Teil | Inhalt |
|---|---|
| Prüfmodus | `rppc run --pruefen`; `rppc test` führt alle Laufzeittests damit aus. Nach jeder Anweisung auf oberster Ebene von `main` prüft der Interpreter I-1 (eindeutiger Besitz, Referenzzähler), I-2 und I-4 (keine Verweise auf freigegebene Objekte) und I-5 (gesperrte Objekte leben), am Ende Lecks in Programmen ohne `shared`. Ein Verstoß ist ein interner Fehler mit Exit-Code 3. Gegenprobe: Ein absichtlich eingebauter Fehler (alter Wert bei Zuweisung nicht freigegeben) wird gemeldet. |
| Programmfehler beim Aufräumen | Entsteht beim Aufräumen nach einem Programmfehler ein zweiter, bleibt der erste die gemeldete Ursache, der zweite wird angehängt, und es laufen keine weiteren Destruktoren (R-9.14: das Programm endet sofort). |
| Sperren | Ein geliehenes Objektargument mit shared-Wurzel wird für den Aufruf gesperrt und gebunden, nicht gelesen; primitive Argumente werden gelesen (wie im FR++-Modell). |

Vergleich mit dem FR++-Modell, mit Prüfmodus: alle 10 handgeschriebenen Programme und 466 von 467 ausgeführten Zufallsprogrammen (aus 2000) laufen gleich; keine Invariante verletzt. Der verbleibende Unterschied (seed1070) liegt am Modell: Es räumt nach einem Programmfehler nicht auf, R-9.14 verlangt das (`fr-modell/vergleichsbericht.txt`).

`rppc test ..`: 241 bestanden, 0 fehlgeschlagen, 2 offen für M5 (`StringBuilder`, `Set`).

Weiter offen und für M5 vorgesehen: `new T()` in generischen Rümpfen und `for` über `Iterable`-Klassen.

Als Nächstes folgt M5: die eingebauten Teile der Standardbibliothek (`rpp.text`, `rpp.collections`, `rpp.io`, `rpp.env`, `rpp.fmt`).

**M5, erster Teil (22.11.2026).** Die Klassen der Standardbibliothek stehen weiter als R++-Quelltext in `rppc/std/rpp/…`; Methoden mit dem Rumpf `fail("eingebaut")` führt der Interpreter selbst aus (`rpp-interp/src/stdlib.rs`):

| Paket | eingebaut |
|---|---|
| `rpp.collections` | `Map` (Einträge als `Entry`-Objekte in Einfügereihenfolge, `put` ersetzt und behält die Position, `at` mit fehlendem Schlüssel ist ein Programmfehler), `Set`; Durchlauf über `begin`/`done`/`next`/`at` |
| `rpp.text` | `StringBuilder` |
| `rpp.fmt` | `padLeft`, `padRight`, `hex`, `fixed` (korrekt gerundet) |
| `rpp.io` | `Console`, `File` (`open` wirft `IOException`, `readAllText` wirft `FormatException` bei ungültigem UTF-8), `Dir` (`list` sortiert), `Path` (Trenner `/`) |
| `rpp.env` | `Env::get`, `Env::os` |
| `rpp.core` | `hash`, `equals`, `compareTo`, `less` für primitive Werte und Strings (`hash` von Strings: FNV-1a über UTF-8) |

Dazu `for` über Klassen mit `Iterable<T, C>` (R-13.10). Beim Freigeben eines Objekts werden jetzt auch die eingebauten Inhalte freigegeben (der Prüfmodus hatte ein Leck bei `Map` gefunden).

Neue Laufzeittests: `kap13_bibliothek/laufzeit/l05_map_set.rpp` und `l06_fmt_und_fehlender_schluessel.rpp`. `rppc test ..`: alle 245 Konformitätstests bestehen, die Laufzeittests in beiden Modi und im Prüfmodus. Damit ist das Ziel von M5 erreicht (alle Konformitätstests bestehen).

Noch offen:
- `new T()` in generischen Rümpfen: Der Interpreter führt Typargumente zur Laufzeit noch nicht mit (R-7.26 verlangt das für den Interpreter);
- Tests für Dateien und Verzeichnisse: Sie brauchen ein festgelegtes Testverzeichnis (R-11.19, R-11.20);
- `Map` und `Set` suchen linear; für den späteren Compiler braucht es einen Hash-Index;
- V-15 (Kapitel 5): Mit den heutigen Regeln lässt sich `Map.remove` nicht in R++ schreiben, deshalb sind `Map` und `Set` eingebaut statt Quelltext.

**V-15 umgesetzt (22.11.2026).** Zerlegen eines `own`-Objekts nach R-5.25 in `rppc` (Prüfung und Interpreter) und im FR++-Modell (Parser, Prüfer, Interpreter, Übersetzung nach R++). Neue Tests: Kapitel 5 `a07_zerlegen.rpp`, `l01_zerlegen.rpp`, `n22` bis `n26`; im Modell `a11_zerlegen.fr`, `n25_zerlegen_mit_destruktor.fr`. `rppc test ..`: alle 252 Konformitätstests bestehen. Damit könnten `Map` und `Set` als R++-Quelltext geschrieben werden; sie bleiben vorerst eingebaut.

**M5 abgeschlossen (22.11.2026).**

| Teil | Inhalt |
|---|---|
| Typargumente zur Laufzeit (R-7.26) | Ein Objekt trägt die Typargumente seiner Klasse; Methoden, Konstruktoren, Feldinitialisierer und Lambdas leiten daraus über `this` die Belegung ihrer Typ-Parameter ab, auch über Basisklassen. `new T()` erzeugt damit die richtige Klasse. Noch nicht mitgeführt werden Typargumente generischer Methoden (`new T()` in einer generischen Methode ist ein interner Fehler). |
| Hash-Index | `Map` und `Set` finden Schlüssel über einen Index Hashwert → Positionen; `hash()` eigener Klassen wird aufgerufen. Nach `remove` wird der Index neu aufgebaut. |
| Arbeitsverzeichnis der Laufzeittests | `rppc test` startet jeden Laufzeittest je Modus in einem frischen, leeren Verzeichnis und löscht es danach (V-14.3 in Kapitel 14, angenommen am 22.11.2026). |

Neue Laufzeittests: `kap07_klassen/laufzeit/l04_new_t.rpp`, `kap13_bibliothek/laufzeit/l07_map_eigene_schluessel.rpp` (2000 Einträge), `l08_dateien.rpp` (`File`, `Dir`, `Path`, `IOException`). `rppc test ..`: alle 255 Konformitätstests bestehen, die Laufzeittests in beiden Modi und im Prüfmodus.

Als Nächstes folgt nach dem Plan M6: der Beginn des R++-Compilers in R++.

**M6 begonnen (22.11.2026): der Lexer in R++.** Das Projekt `rpp-compiler/` (Name `rppcc`) enthält einen Lexer in R++ (`src/compiler/lexer/`), gleichwertig zum Rust-Lexer. `rppc run ../rpp-compiler -- tokens DATEI` und `rppc tokens DATEI` geben beide eine Zeile je Token aus; `rpp-compiler/vergleich_lexer.py` vergleicht sie für alle 503 `.rpp`-Dateien des Projekts (einschließlich des Lexers selbst) und 14 Grenzfälle: 517 gleich, 0 verschieden. Gegenprobe: Ein absichtlich entferntes Satzzeichen führt zu 18 Abweichungen.

Der Lexer wurde ohne eine Änderung an Prüfung oder Interpreter angenommen und ausgeführt. Geändert wurde nur, dass `for` über einen String die Zeichen einmal zerlegt (vorher quadratische Laufzeit).

Weiter in M6: Präprozessor (bedingte Kompilierung), dann der Parser mit AST; verglichen wird jeweils mit `rppc` (Annahme, Ablehnung, erste Fehlerzeile, Lesarten).

**M6: Präprozessor und Parser in R++ (22.11.2026).** `rpp-compiler/` enthält jetzt außer dem Lexer den Präprozessor (`src/compiler/pre/`, mit eigener Exception-Klasse für Fehler in Direktiven und einem Enum mit Daten für die Bedingungen) und den Parser (`src/compiler/syntax/`, etwa 1300 Zeilen R++). Der Syntaxbaum ist einheitlich aufgebaut (Art, Text, Position, Kinder); seine Textform erzeugt auf der Rust-Seite `rpp-syntax/src/dump.rs` aus dem AST von `rppc`. Verglichen werden mit `rpp-compiler/vergleich.py`:

| Teil | Befehl | gleich | verschieden |
|---|---|---|---|
| Lexer | `tokens DATEI` | 521 | 0 |
| Präprozessor | `pre SYMBOLE DATEI` | 1056 | 0 |
| Parser mit Prüfung R-8.12 | `ast SYMBOLE DATEI`: Baum oder erster Fehler mit Position | 1034 | 0 |

Eingaben sind alle `.rpp`-Dateien des Projekts, auch die negativen Grammatiktests und die Lesarten, und Grenzfälle; Präprozessor und Parser mit zwei Symbolsätzen. Gegenproben (ein absichtlich falscher Vorrang im Parser) werden erkannt.

Der Parser lief zunächst nur für 964 von 1032 Fällen: Der Interpreter verschob ein `own`-Argument nicht, wenn es über `??`, `?:` oder Klammern kam, und ließ einen Verweis auf einen schon freigegebenen Knoten zurück. Die Prüfung hatte den Ausdruck richtig als Move behandelt. Behoben; dazu der Laufzeittest `kap05_ownership/laufzeit/l02_own_argument_ausdruck.rpp`. `rppc test ..`: 256 bestanden.

Weiter in M6: die Namensauflösung und Typprüfung in R++ (wie `rpp-sema`), verglichen über die Meldungen der Konformitätstests.

**M6: Projekte laden in R++ (22.11.2026).** Erste Etappe der Namensauflösung: `rpp-compiler/src/compiler/project/` liest `rpp.yaml` (eigener Leser für den YAML-Teil aus R-11.13, wie `yaml.rs`) und lädt Projekte, Quellordner, lokale Abhängigkeiten und die Standardbibliothek wie `project.rs`, mit den Meldungen zu R-11.13 und R-11.15 bis R-11.17. Quelldateien liest `compiler/syntax/Source.rpp` (Präprozessor, Lexer, Parser, R-8.12) mit dem ersten Fehler als `SyntaxError`. Verglichen wird die Ausgabe von `projekt [--release] STD PFAD` (Projekte, Dateien mit deklariertem und erwartetem Paket und Typnamen, Meldungen): 592 gleich, 0 verschieden. Eingaben sind alle Projektverzeichnisse und Einzeldateien der Konformitätstests, `rpp-compiler` selbst und 39 Grenzfälle für `rpp.yaml`, jeweils im Debug- und Releasemodus.

Dabei geändert:

- `rppc` meldet falsche Formen in `rpp.yaml` jetzt als Fehler (R-11.13: `sources`, `dependencies` und die Symbole sind Listen, ein `type`, der kein Text ist, ein `path`, der kein Text ist). Bisher wurde etwa ein Quellordner `[x]` als Projektverzeichnis gelesen und `main.rpp` doppelt geladen.
- Pfade, die nicht existieren, normalisiert `rppc` textuell (`.` und `..`); die Standardbibliothek wird mit kanonischem Pfad geladen.
- Neu in der Standardbibliothek (V-13.8, angenommen): `File::exists` und `Path::absolute`; Laufzeittest `kap13_bibliothek/laufzeit/l09_pfade.rpp`. `rppc test ..`: 257 bestanden.

Weiter in M6: Deklarationen sammeln und Namen auflösen (wie `defs.rs`), verglichen über die Meldungen.

**M6: Deklarationsphase in R++ (22.11.2026).** `rpp-compiler/src/compiler/sema/` enthält die Typdarstellung (`Ty.rpp`, wie `ty.rs`; Typen sind unveränderlich und werden als `shared Ty` geteilt), die Deklarationstabellen (`Defs.rpp`) und `Program.rpp` mit allen Phasen von `Program::build` aus `defs.rs`: Typen sammeln (R-11.4), Typnamen auflösen (R-11.7, R-11.8, R-11.10, R-11.12, R-7.23, R-4.37, R-4.45, R-10.13), Basisklassen, Importe und Vererbungszyklen (R-7.12, R-7.20), Felder, Methoden, Konstruktoren und Enum-Varianten (R-4.25, R-4.26, R-7.27), Constraints (R-7.25, R-7.26), Dateien (R-11.2, R-11.14, R-11.18) und Klassen (R-7.1, R-7.6, R-7.10, R-7.11, R-7.13, R-7.14, R-7.16, R-7.18, R-7.20, R-7.21, R-8.15, R-11.11).

Verglichen wird `defs STD PFAD` (`rppc/crates/rppc/src/defsdump.rs`): alle Typen mit Typ-Parametern, Constraints, Basis, Importen, Feldern, Methoden- und Konstruktorsignaturen und Varianten, dazu die Meldungen dieser Phase in der Reihenfolge von `rppc`. Eingaben sind die Projekte und Einzeldateien der Konformitätstests, `rpp-compiler` selbst, die Grammatiktests und 17 Grenzfälle: 501 gleich, 0 verschieden. Gegenprobe: Mit `protected` auf dem Rang von `public` weicht ein Grenzfall ab.

Dabei aufgefallen und behoben: Der Interpreter verschob ein Enum ohne Daten und ein Tupel aus Grundwerten, wenn sie an einen Besitzer gingen (`Access acc = access;`). Die Prüfung behandelt beide als Kopie (`movable`). Die Quelle blieb dann als verschoben zurück, und ein späteres Lesen brach mit einem internen Fehler ab. Neuer Laufzeittest `kap05_ownership/laufzeit/l03_enum_und_tupel_kopieren.rpp`; `rppc test ..`: 258 bestanden.

Weiter in M6: die Prüfung der Rümpfe (`body.rs`, `flow.rs`): Ausdrücke, Anweisungen, Null-Sicherheit, Ownership und Flussanalyse, verglichen über die Meldungen der Konformitätstests.

**M6: Prüfung der Rümpfe in R++ (22.11.2026).** `rpp-compiler/src/compiler/sema/Checker.rpp` (rund 3300 Zeilen) überträgt `body.rs`: Anweisungen und Ausdrücke mit Typen, Überladungsauflösung und Ableitung von Typargumenten, Lambdas, eingebaute Methoden und Funktionen, dazu die Flussanalyse aus `flow.rs` (`Flow.rpp`: Verengung, sichere Initialisierung, Moves, Loans, Fixpunkt an Schleifen, try/catch/finally). Damit prüft `rpp-compiler` R++-Programme vollständig wie `rppc check`. Nicht übertragen sind die Angaben für die Ausführung (`rt.rs`); sie kommen mit dem Codegenerator.

Für den Prüfer besitzt der Prüfer das Programm und leiht die Syntaxbäume; `Program` hält dafür je Datei Projekt, Paket und `using`-Einträge vor. Mengen und Zuordnungen der Flussanalyse sind sortierte Listen; Ganzzahlliterale über `i64::MAX` stehen als Vorzeichen und `ui64`-Betrag da (Literale über `ui64::MAX` gelten anders als in `rppc` als ungültig).

Verglichen wird `pruefe STD PFAD` (alle Meldungen mit Warnungen und Begründungsregeln, sortiert wie `rppc check`). Eingaben sind die Projekte und Einzeldateien der Konformitätstests, die Grammatiktests, die Grenzfälle der Deklarationsphase und die nach R++ übersetzten FR++-Programme (handgeschriebene und 300 Zufallsprogramme): 782 gleich, 0 verschieden. Gegenprobe: Mit R-6.9 statt R-6.8 als Begründung nach einem `if` weicht ein Konformitätstest ab. Auch `rpp-compiler` selbst prüfen beide Seiten gleich (keine Meldung); im Interpreter dauert das gut 20 Minuten, vor allem wegen des Fixpunkts an verschachtelten Schleifen.

Weiter in M6: die Angaben für die Ausführung und ein Codegenerator (Zwischendarstellung oder C), dann die Selbstübersetzung.

**M6: Codegenerator nach C und Selbstübersetzung (22.11.2026).** Entschieden (22.11.2026): Der Codegenerator erzeugt C (C17, übersetzbar mit MSVC und gcc), nicht zuerst eine eigene Zwischendarstellung.

| Teil | Inhalt |
|---|---|
| Angaben für die Ausführung | `Checker.rpp` erhebt sie an denselben Stellen wie `body.rs` (`Rt.rpp`, wie `rt.rs`): Typen, Zieltypen, Moves, Aufrufziele, Basiskonstruktoren, Typen der Deklarationen; dazu die Erfassungen aller Lambdas. Die Knoten sind im Arbeitsbereich nummeriert (`Node.id`); mit `withStd` wird auch die Standardbibliothek geprüft. |
| Codegenerator (`compiler/gen/CGen.rpp`) | setzt die Auswertung des Debug-Interpreters Stelle für Stelle in C um (`eval`, `eval_store`, `transfer`, `temp`, `convert`, `args`, alle Anweisungen); Fehler laufen als Status mit Aufräummarken je Anweisung und Gültigkeitsbereich; virtuelle Aufrufe über Verteiler je Methode; Generics ohne Vervielfältigung, Typargumente am Objekt (R-7.26) einschließlich der Sicht einer Basisklasse |
| Laufzeit (`runtime/rpp_rt.c`) | Werte mit Art und Besitzform wie `value.rs`; Freigabe mit Zählern für `shared`, `weak` und Strings; Zahlen beider Modi mit 128-Bit-Zwischenwerten wie `i128`; Programmfehler, Stacktrace, Exit-Codes (R-12.9); 12 000 Aufrufe auf einem eigenen Stack; eingebaute Teile der Standardbibliothek |
| Befehl | `rppcc c [--release] STD PFAD AUSGABE.c` |

Verglichen wird das Verhalten mit `rppc run` (`rpp-compiler/vergleich_c.py`: erzeugen, mit MSVC übersetzen, im frischen Arbeitsverzeichnis ausführen; Ausgabe, Fehlerausgabe und Exit-Code):

| Eingaben | gleich | verschieden |
|---|---|---|
| Laufzeittests der Konformitätstests, beide Modi | 84 | 0 |
| `rpp-compiler/tests_c` (Lambdas, Enums mit Daten, `finally`, Tupel, Slices, `shared`/`weak` …), beide Modi | 2 | 0 |
| FR++-Programme (handgeschrieben und Zufallsprogramme), beide Modi | 138 (69 Programme) | 0 |

**Selbstübersetzung.** Der interpretierte `rpp-compiler` erzeugt für sich selbst C-Code (217 783 Zeilen, 20 Minuten im Interpreter); MSVC übersetzt ihn in gut einer Minute zu `rppcc.exe`. Dieses Programm
- erzeugt für sich selbst denselben C-Code, Byte für Byte (Fixpunkt; 3 Minuten 50);
- stimmt in allen Teilen von `vergleich.py --exe` mit `rppc` überein: `tokens` 539, `pre` 1092, `ast` 1070, `projekt` 596, `defs` 507, `pruefe` 789 gleich, 0 verschieden; `pruefe` jetzt einschließlich `rpp-compiler` selbst (3 Minuten 45 statt rund 20 Minuten im Interpreter).

Beim Vergleich aufgefallen und behoben: Eine Sicht auf einen `weak`-Verweis wurde beim Speichern nicht gezählt (Laufzeit, Speicherfehler in FR++-Programmen).

**Geschwindigkeit (22.11.2026).** Gemessen mit einem Stichproben-Profiler in der Laufzeit (`RPP_PROFILE=DATEI`, Windows; Adressen über die Map-Datei des Linkers aufgelöst). Drei Ursachen und ihre Behebung:

| Ursache | Behebung | `pruefe rpp-compiler` | Selbstübersetzung |
|---|---|---|---|
| Ausgangsstand | – | 225 s | 230 s |
| kleine Laufzeitfunktionen als echte Aufrufe zwischen zwei Übersetzungseinheiten | schnelle Pfade `static inline` im Header (Sichten, Zwischenwerte, Freigabe von Grundwerten, Ganzzahl-Rechnen und -Vergleich, Index, `length`); gemeinsamer Zahltyp als Tabelle | (mit 2.) 28 s | 101 s |
| Flussanalyse in `Flow.rpp`: sortierte Listen linear durchsucht, Kopien Element für Element | binäre Suche, Schnitt/Vereinigung/Differenz durch Zusammenführen, `copy(…)` für die Kopie; Ergebnisse unverändert | | |
| geschwätziger Code für Grundwerte | Generator lässt `temp`, `promote`, `own_str` und doppelte Zahlumwandlungen bei Grundwerten weg; Sperrprüfung (V-1) nur, wenn die Wurzelvariable `shared` oder ein Typ-Parameter sein kann | 9,2 s | 10,4 s |

Zum Vergleich: `rppc pruefe` (Rust) braucht 0,6 s, der Interpreter rund 20 Minuten. Der Fixpunkt der Selbstübersetzung hält nach jedem Schritt (zwei Runden, weil der neue Generator erst in der zweiten Runde läuft).

**Weitere offene Punkte (22.11.2026).**

| Punkt | Umsetzung |
|---|---|
| `new T()` mit dem Typ-Parameter einer generischen Methode | Die Prüfung hält die abgeleiteten Typargumente am gewählten Aufruf fest (`MethodInfo.targs`, in `body.rs` und `Checker.rpp`). Der Interpreter setzt sie, vom Aufrufer aus gesehen, in den Rahmen der Methode und nimmt sie in Lambdas mit; der Codegenerator gibt sie über `rt_set_mtargs` an die Methode und über `RFun.mt` an Lambdas. Neuer Laufzeittest `kap07_klassen/laufzeit/l05_new_t_methode.rpp` (auch über einen weiteren generischen Aufruf, eine Instanzmethode und ein Lambda); `rppc test ..`: 259 bestanden. Generische Methoden lassen sich nicht überschreiben (R-7.13), virtuelle Aufrufe behalten die Typargumente trotzdem. |
| Unicode für `char` und Strings | Die C-Laufzeit nimmt ihre Tabellen aus der Standardbibliothek von Rust (`runtime/gen_unicode.rs` erzeugt `rpp_unicode.h`: Buchstaben, Groß- und Kleinbuchstaben, Leerraum, Abbildungen auf bis zu drei Zeichen) und Case_Ignorable aus den Unicode-Kategorien (`gen_unicode_ignorable.py`) für das Schluss-Sigma. Zusatztest `rpp-compiler/tests_c/t02_unicode.rpp`. |
| Text von `IOException` | V-13.9 (Kapitel 13, angenommen am 23.11.2026): Der Text nennt den Pfad und beschreibt den Fehler; der genaue Wortlaut, besonders der Teil vom Betriebssystem, ist nicht festgelegt. |

Offen: Der übersetzte Code ist noch rund 15-mal langsamer als `rppc` (Werte mit Art in jedem Ausdruck, Rahmen und Felder je Aufruf, Namensvergleich bei eingebauten Methoden); der Releasemodus über eine eigene Zwischendarstellung oder LLVM bleibt nach dem Plan offen.

**M6 abgeschlossen (23.11.2026).** Kriterium (festgelegt am 23.11.2026): Der R++-Compiler übersetzt sich selbst (Fixpunkt) und stimmt in allen Vergleichen mit `rppc` überein. Stand: Fixpunkt nach zwei Runden; `vergleich.py --exe` 540/1094/1072/598/508/790 gleich (tokens, pre, ast, projekt, defs, pruefe), 0 verschieden; `vergleich_c.py --fr 200` 228 gleich, 0 verschieden; `rppc test ..` 259 bestanden.

Entschieden am 23.11.2026: V-13.9 (Text von `IOException`) ist angenommen. `Map` und `Set` bleiben eingebaut; als R++-Quelltext erst, wenn der erzeugte Code schneller ist. Die Geschwindigkeit des übersetzten Codes wird Meilenstein M7; ob der C-Codegenerator der Releasemodus nach R-12.1 wird (statt einer eigenen Zwischendarstellung bzw. LLVM, Abschnitt 3), ist noch zu besprechen.
