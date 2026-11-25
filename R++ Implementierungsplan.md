# R++ Implementierungsplan

Stand: 23.11.2026, Entwurf 0.12. Dieses Dokument beschreibt die erste Implementierung von R++: Wirtssprache, Aufbau, Prüfstrategie und Meilensteine. Es ist kein Teil der Spezifikation; maßgeblich bleiben die Kapitel 1 bis 14 und die Grammatik `RPP.g4`.

Wirtssprache, Aufbau und Meilensteine sind entschieden (Designentscheidungen, Abschnitt 27). Die Meilensteine M1 bis M6 sind abgeschlossen (Abschnitt 6); M7, die Geschwindigkeit des übersetzten Codes, läuft über den Ausbau des C-Codegenerators.

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
| M7 | Geschwindigkeit des übersetzten Codes: Ausbau des C-Codegenerators (entschieden am 24.11.2026) | – (Zielwert noch festzulegen, Vorschlag: übersetzter `rppcc` mindestens so schnell wie `rppc` bei `diagnose rpp-compiler`) |

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

Erster Stand des Vergleichs mit dem FR++-Modell (`fr-modell/vergleich.py --run`): Von 1000 Zufallsprogrammen werden 226 von beiden angenommen und ausgeführt. 212 liefern dieselbe Ausgabe einschließlich der Destruktoren, ebenso alle 10 handgeschriebenen Programme. Offen sind 14 Unterschiede (Kapitel 5, Abschnitt 10.5 und `fr-modell/vergleichsbericht.txt`): 13 endlose Destruktor-Kaskaden, bei denen das Modell mit `BorrowError` endet und `rppc` mit `StackOverflowError`, und ein ungeklärter Sperrkonflikt (seed772).

Änderung am Plan (angenommen am 22.11.2026): Abschnitt 3.3 sah eine Zwischenrepräsentation als Kontrollflussgraph vor, die der Interpreter ausführt. Der erste Teil von M4 führt stattdessen den geprüften AST zusammen mit den Angaben der Prüfung aus; Freigabe, Sperren, Unwinding und Prüfungen je Modus stehen ausdrücklich im Interpreter. Die Zwischenrepräsentation entsteht erst mit dem JIT bzw. dem Releasemodus, wenn sie für die Codeerzeugung gebraucht wird; bis dahin ist der AST-Interpreter die Referenz (R-12.3). Begründung: Alle Laufzeitregeln sind so schon prüfbar, und die Zwischenrepräsentation lässt sich später gegen diesen Interpreter testen.

Noch offen nach dem ersten Teil: der Prüfmodus für die Invarianten, `new T()` in generischen Rümpfen und die Iteration über `Iterable`-Klassen (mit M5).

**M4 abgeschlossen (22.11.2026).**

| Teil | Inhalt |
|---|---|
| Prüfmodus | `rppc run --check-invariants`; `rppc test` führt alle Laufzeittests damit aus. Nach jeder Anweisung auf oberster Ebene von `main` prüft der Interpreter I-1 (eindeutiger Besitz, Referenzzähler), I-2 und I-4 (keine Verweise auf freigegebene Objekte) und I-5 (gesperrte Objekte leben), am Ende Lecks in Programmen ohne `shared`. Ein Verstoß ist ein interner Fehler mit Exit-Code 3. Gegenprobe: Ein absichtlich eingebauter Fehler (alter Wert bei Zuweisung nicht freigegeben) wird gemeldet. |
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
- stimmt in allen Teilen von `vergleich.py --exe` mit `rppc` überein: `tokens` 539, `pre` 1092, `ast` 1070, `project` 596, `defs` 507, `diagnose` 789 gleich, 0 verschieden; `diagnose` jetzt einschließlich `rpp-compiler` selbst (3 Minuten 45 statt rund 20 Minuten im Interpreter).

Beim Vergleich aufgefallen und behoben: Eine Sicht auf einen `weak`-Verweis wurde beim Speichern nicht gezählt (Laufzeit, Speicherfehler in FR++-Programmen).

**Geschwindigkeit (22.11.2026).** Gemessen mit einem Stichproben-Profiler in der Laufzeit (`RPP_PROFILE=DATEI`, Windows; Adressen über die Map-Datei des Linkers aufgelöst). Drei Ursachen und ihre Behebung:

| Ursache | Behebung | `diagnose rpp-compiler` | Selbstübersetzung |
|---|---|---|---|
| Ausgangsstand | – | 225 s | 230 s |
| kleine Laufzeitfunktionen als echte Aufrufe zwischen zwei Übersetzungseinheiten | schnelle Pfade `static inline` im Header (Sichten, Zwischenwerte, Freigabe von Grundwerten, Ganzzahl-Rechnen und -Vergleich, Index, `length`); gemeinsamer Zahltyp als Tabelle | (mit 2.) 28 s | 101 s |
| Flussanalyse in `Flow.rpp`: sortierte Listen linear durchsucht, Kopien Element für Element | binäre Suche, Schnitt/Vereinigung/Differenz durch Zusammenführen, `copy(…)` für die Kopie; Ergebnisse unverändert | | |
| geschwätziger Code für Grundwerte | Generator lässt `temp`, `promote`, `own_str` und doppelte Zahlumwandlungen bei Grundwerten weg; Sperrprüfung (V-1) nur, wenn die Wurzelvariable `shared` oder ein Typ-Parameter sein kann | 9,2 s | 10,4 s |

Zum Vergleich: `rppc diagnose` (Rust) braucht 0,6 s, der Interpreter rund 20 Minuten. Der Fixpunkt der Selbstübersetzung hält nach jedem Schritt (zwei Runden, weil der neue Generator erst in der zweiten Runde läuft).

**Weitere offene Punkte (22.11.2026).**

| Punkt | Umsetzung |
|---|---|
| `new T()` mit dem Typ-Parameter einer generischen Methode | Die Prüfung hält die abgeleiteten Typargumente am gewählten Aufruf fest (`MethodInfo.targs`, in `body.rs` und `Checker.rpp`). Der Interpreter setzt sie, vom Aufrufer aus gesehen, in den Rahmen der Methode und nimmt sie in Lambdas mit; der Codegenerator gibt sie über `rt_set_mtargs` an die Methode und über `RFun.mt` an Lambdas. Neuer Laufzeittest `kap07_klassen/laufzeit/l05_new_t_methode.rpp` (auch über einen weiteren generischen Aufruf, eine Instanzmethode und ein Lambda); `rppc test ..`: 259 bestanden. Generische Methoden lassen sich nicht überschreiben (R-7.13), virtuelle Aufrufe behalten die Typargumente trotzdem. |
| Unicode für `char` und Strings | Die C-Laufzeit nimmt ihre Tabellen aus der Standardbibliothek von Rust (`runtime/gen_unicode.rs` erzeugt `rpp_unicode.h`: Buchstaben, Groß- und Kleinbuchstaben, Leerraum, Abbildungen auf bis zu drei Zeichen) und Case_Ignorable aus den Unicode-Kategorien (`gen_unicode_ignorable.py`) für das Schluss-Sigma. Zusatztest `rpp-compiler/tests_c/t02_unicode.rpp`. |
| Text von `IOException` | V-13.12 (Kapitel 13, angenommen am 23.11.2026): Der Text nennt den Pfad und beschreibt den Fehler; der genaue Wortlaut, besonders der Teil vom Betriebssystem, ist nicht festgelegt. |

Offen: Der übersetzte Code ist noch rund 15-mal langsamer als `rppc` (Werte mit Art in jedem Ausdruck, Rahmen und Felder je Aufruf, Namensvergleich bei eingebauten Methoden); der Releasemodus über eine eigene Zwischendarstellung oder LLVM bleibt nach dem Plan offen.

**M6 abgeschlossen (23.11.2026).** Kriterium (festgelegt am 23.11.2026): Der R++-Compiler übersetzt sich selbst (Fixpunkt) und stimmt in allen Vergleichen mit `rppc` überein. Stand: Fixpunkt nach zwei Runden; `vergleich.py --exe` 540/1094/1072/598/508/790 gleich (tokens, pre, ast, project, defs, diagnose), 0 verschieden; `vergleich_c.py --fr 200` 228 gleich, 0 verschieden; `rppc test ..` 259 bestanden.

Entschieden am 23.11.2026: V-13.12 (Text von `IOException`) ist angenommen. `Map` und `Set` bleiben eingebaut; als R++-Quelltext erst, wenn der erzeugte Code schneller ist. Die Geschwindigkeit des übersetzten Codes wird Meilenstein M7; ob der C-Codegenerator der Releasemodus nach R-12.1 wird (statt einer eigenen Zwischendarstellung bzw. LLVM, Abschnitt 3), ist noch zu besprechen.

**Ausbau der Standardbibliothek: rpp.random (23.11.2026).** Erstes Paket des Ausbaus (V-13.9; Standardbibliothek, Abschnitte 9 und 10, R-13.23 bis R-13.25). `rppc/std/rpp/random/Random.rpp` ist reiner R++-Quelltext ohne eingebaute Teile: xoshiro256** mit SplitMix64-Seeding, dazu `nextUi64`, `nextInt`, `nextFloat`, `nextBool` und das generische `shuffle<T>(mut T[] v)`.

| Teil | Inhalt |
|---|---|
| Arithmetik modulo 2⁶⁴ | Addition und Multiplikation in 32-Bit-Hälften, ohne Überlauf (R-4.14); beide Modi laufen gleich. Schieben verwirft herausgeschobene Bits (R-4.16). |
| `nextInt` | gleichverteilt ohne Modulo-Bias durch Verwerfen unterhalb einer Schwelle; `bound ≤ 0` ist ein `FailError` |
| `shuffle` | Fisher-Yates über `remove`/`insert`: Aus Elementen wird nicht herausbewegt (V-3), und `swap` auf zwei Indizes gilt immer als überlappend (Kapitel 5, Abschnitt 7). Die Elemente werden verschoben; das funktioniert für jeden Elementtyp, auch für `own`-Objekte. |

Die erwarteten Ausgaben der Laufzeittests sind mit einer unabhängigen Referenzimplementierung (Python) berechnet und in den Testköpfen festgeschrieben: `a06_random`, `l20_random_folge` (feste Folgen für die Seeds 42, 0 und 7, Reproduzierbarkeit, `nextInt`, `nextFloat`, `nextBool`), `l21_random_shuffle` (Grundwerte, `own`-Objekte, Strings, leerer Vektor; Seed 2026), `l22_random_bound` (`FailError` bei `nextInt(0)`).

`rppc test ..`: alle 272 Konformitätstests bestehen, die Laufzeittests in beiden Modi und im Prüfmodus. Gegenprobe mit dem Compiler in R++: `diagnose` meldet für `Random.rpp` und die neuen Tests auf beiden Seiten dieselben (keine) Meldungen. Der veränderbare Vektor-Parameter `mut T[]` mit `remove`/`insert` durch die Prüfung war damit erstmals in einem Bibliothekspaket im Einsatz.

**Ausbau: Vektor-Algorithmen und ein Prüfungs-Fix (23.11.2026).** `rppc/std/rpp/collections/Vectors.rpp` (reiner R++-Quelltext, V-13.10) ergänzt `rpp.collections` um statische generische Methoden: `binarySearch` (Comparable), `equals` (Equatable, elementweiser Vergleich – Arrays haben kein `==`, V-4.8), `reverse`, `retain` (Prädikat) und `dedup` (aufeinanderfolgende Duplikate). Die veränderbaren Methoden arbeiten über `remove`/`insert` (V-3; `swap` auf zwei Indizes gilt immer als überlappend) und verschieben die Elemente; sie funktionieren auch für `own`-Objekte.

| Teil | Inhalt |
|---|---|
| Hilfsklasse statt eingebaut | Als R++-Klasse geschrieben, nicht als eingebaute Methoden auf `T[]`: beide Compiler behandeln sie gleich, keine Divergenz in M6 (V-13.10). Eingebaute Methoden bleiben, was R++ nicht effizient ausdrücken kann (`sort`). |
| Prüfungs-Fix R-7.26 | Beim Schreiben aufgefallen: Constraints generischer **Methoden** wurden am Aufruf nicht geprüft (nur Klassen-Constraints über `check_pending`). `binarySearch` mit einem Typ ohne `Comparable` wurde angenommen und scheiterte erst zur Laufzeit (`InternalError: Methode compareTo ohne Rumpf`). In `body.rs` (nach der Methodenwahl, die abgeleiteten Typargumente gegen die Constraints geprüft) und in `Checker.rpp` behoben. R-7.26 verlangte die Prüfung bereits; der Fix macht die Implementierung erst spec-konform. |

Neue Tests: `a07_vectors`, `l23_vectors_suche`, `l24_vectors_mutieren`, `n08_vectors_ohne_comparable` (R-7.26). `rppc test ..`: alle 276 Konformitätstests bestehen, die Laufzeittests in beiden Modi und im Prüfmodus; `cargo test` ohne Fehler. Gegenprobe mit dem Compiler in R++: `diagnose` meldet für `Vectors.rpp` und die vier neuen Tests auf beiden Seiten dasselbe (der Negativtest wird hüben wie drüben mit R-7.26 abgelehnt).

**Ausbau: rpp.math (23.11.2026).** `rppc/std/rpp/math/Math.rpp` ergänzt die Bibliothek um `f64`-Funktionen (V-13.11; Standardbibliothek, Abschnitt 12, R-13.28, R-13.29): die Konstanten `PI` und `E`, `sqrt`, `floor`, `ceil`, `trunc`, `round` und `clamp`.

| Teil | Inhalt |
|---|---|
| Runden und clamp | reiner R++-Quelltext. `floor`/`ceil`/`trunc`/`round` runden exakt ganzzahlig über `cast<i64>` (Richtung null, R-4.10) mit Anpassung; `round` bei .5 von null weg. NaN (`x != x`), ±0 und \|x\| ≥ 2⁵² (ganzzahlig, auch ±∞) bleiben unverändert – dadurch laufen beide Modi gleich und das Vorzeichen von null bleibt erhalten. `clamp` verlangt `lo <= hi` (sonst `FailError`). |
| sqrt | eingebaut (`fail("eingebaut")`, Dispatch in `stdlib.rs` über `rpp.math.Math`), korrekt gerundet nach IEEE 754 wie `Fmt::fixed`; `x < 0` ergibt NaN. Als R++-Klasse mit eingebautem Rumpf behandeln beide Compiler die Signatur gleich – keine M6-Divergenz (V-13.11). |

Die Soll-Ausgaben sind mit einer unabhängigen Python-Referenz berechnet (u. a. `sqrt(2)` auf 10 Stellen, `PI`/`E` auf 5 Stellen). Neue Tests: `a08_math`, `l25_math_runden` (Runden, clamp, NaN/∞), `l26_math_sqrt` (Quadratzahlen, korrekt gerundetes `sqrt(2)`, NaN/∞, Konstanten), `l27_math_clamp_fehler` (`FailError` bei `lo > hi`). `rppc test ..`: alle 280 Konformitätstests bestehen, die Laufzeittests in beiden Modi und im Prüfmodus; `cargo test` ohne Fehler. Gegenprobe mit dem Compiler in R++: `diagnose` meldet für `Math.rpp` und die vier neuen Tests auf beiden Seiten dasselbe.

**Ausbau: Text-Hilfen (23.11.2026).** `rppc/std/rpp/text/Text.rpp` ergänzt `rpp.text` um statische Methoden auf `string` (Standardbibliothek, Abschnitt 13, R-13.30): `lines`, `removePrefix`, `removeSuffix`, `repeat` und `splitLimit`. Das Paket ist **reiner R++-Quelltext ohne eingebaute Teile** und ohne jede Änderung am Rust-Code – es setzt nur auf vorhandenen String-Methoden (R-13.13), `StringBuilder` und `string::join` auf (V-13.9, V-13.10, Muster `Fmt`).

| Teil | Inhalt |
|---|---|
| `lines` | über `split("\n")`, `removeSuffix(…, "\r")` für CRLF; ein abschließendes `\n` erzeugt keine zusätzliche leere Zeile, `""` ergibt einen leeren Vektor |
| `removePrefix`/`removeSuffix` | `startsWith`/`endsWith` plus `substring`; sonst unverändert |
| `repeat` | `StringBuilder`; `n < 0` ist ein `FailError` |
| `splitLimit` | `split` und `string::join` für den ungeteilten Rest im letzten Teil; `limit < 1` oder leerer Trenner ist ein `FailError` |

Neue Tests: `a09_text`, `l28_text_lines`, `l29_text_hilfen`, `l30_text_split_limit`, `l31_text_repeat_fehler`, `l32_text_split_limit_fehler`. String-Ergebnisse sind in den Tests in `[…]` gekapselt, damit leere Strings eindeutig prüfbar sind. `rppc test ..`: alle 286 Konformitätstests bestehen, die Laufzeittests in beiden Modi und im Prüfmodus. Gegenprobe mit dem Compiler in R++: `diagnose` meldet für `Text.rpp` und die sechs neuen Tests auf beiden Seiten dasselbe.

**Ausbau: Datei-Operationen (23.11.2026).** `File` (in `rppc/std/rpp/io/Io.rpp`) erhält fünf weitere statische Methoden (Standardbibliothek, Abschnitt 14, R-13.31): `appendText`, `copyFile`, `moveFile`, `delete`, `size`. Sie sind eingebaut (`fail("eingebaut")`, Dispatch in `stdlib.rs`), weil sie Dateisystemzugriff brauchen – wie schon `File`/`Dir`. Als R++-Klasse behandeln beide Compiler die Signaturen gleich (keine M6-Divergenz, V-13.10).

| Teil | Inhalt |
|---|---|
| `appendText` | hängt UTF-8 an, legt eine fehlende Datei an (`OpenOptions` append+create) |
| `copyFile` | `std::fs::copy`, liefert die Bytezahl; ersetzt ein vorhandenes Ziel |
| `moveFile` | `std::fs::rename`; schlägt das fehl, weil das Ziel existiert (Windows), wird es entfernt und erneut umbenannt – so ersetzen beide Plattformen ein vorhandenes Ziel gleich |
| `delete` / `size` | `remove_file` bzw. `metadata().len()`; `IOException`, wenn die Datei fehlt |

`copy` ist ein Schlüsselwort (Kapitel 7), deshalb `copyFile`/`moveFile`. Neue Tests: `a10_io`, `l33_io_datei_ops` (alle Operationen im frischen Arbeitsverzeichnis, V-14.3), `l34_io_fehler` (`IOException` bei fehlender Datei). `rppc test ..`: alle 289 Konformitätstests bestehen, die Laufzeittests in beiden Modi und im Prüfmodus; `cargo test` ohne Fehler. Gegenprobe mit dem Compiler in R++: `diagnose` meldet für `Io.rpp` und die drei neuen Tests auf beiden Seiten dasselbe. Offen bleiben gepuffertes Lesen und Mtime (letzteres braucht `rpp.time`, Phase B).

**Merge des Bibliotheksausbaus und C-Laufzeit nachgezogen (23.11.2026).** Der Ausbau der Standardbibliothek (oben, Zweig `temp_merge`) ist mit M6 zusammengeführt; die neuen Teile verwenden `mut` statt `&`. V-13.9 war doppelt vergeben, die Regel zum Text von `IOException` heißt jetzt V-13.12. Die C-Laufzeit (`rpp_rt.c`) kennt jetzt alle neuen eingebauten Teile wie `stdlib.rs`:

| Teil | Umsetzung in C |
|---|---|
| Datei-Operationen, `Dir.entries`, `Path.normalizedText`/`relativeText` | wie `stdlib.rs`; Pfade rein lexikalisch wie `lexical_path`; Art des Eintrags über `lstat` bzw. die Attribute unter Windows |
| `SaveFile` | temporäre Datei neben dem Ziel (exklusiv angelegt), `commit` benennt um und ersetzt ein vorhandenes Ziel, `cancel` und der Destruktor löschen sie |
| `StringView`, `Math.sqrt`, `ElapsedTimer.monotonicNanos` | Sichten als Objekte mit `source`, `byteStart`, `byteEnd`; Zeichen nach UTF-8 |
| `Map.getOrPut`, `Set.appendEntry`, `partitionPoint` | `appendEntry` teilt `own`-Objekte zwischen beiden Mengen wie die Halter im Interpreter |
| Unicode-Pfade unter Windows | Dateizugriffe und Kommandozeilenargumente über die `wchar_t`-Fassungen (UTF-8 ↔ UTF-16) |

Im Codegenerator: `for` über `Iterable` nimmt bei einer Methode ohne Rumpf (`this` in einem Mixin, etwa `Iterable.any`) den Verteiler, wie der Interpreter die Klasse des Werts; der Verteiler unterscheidet Überladungen nach `mut` (`Map.at(i64)` neben `Map.at(K) mut`).

Beim Erzeugen von C (Prüfung mit den Rümpfen der Standardbibliothek) fielen zwei Fehler im neuen Bibliothekscode auf, die `rppc` nicht meldet, weil es die Rümpfe der Standardbibliothek nicht prüft: `Random.mix` wies seinem Parameter zu (R-8.19), `Deque.front`/`back` gaben `null` bzw. einen Borrow zurück, der nicht von `this` ausgeht (R-5.23). Beide sind behoben; die Ausgaben bleiben gleich.

Stand: `rppc test ..` 291 bestanden; `vergleich_c.py --fr 200` 278 gleich, 0 verschieden; Fixpunkt der Selbstübersetzung (Runde 1 bis 3 Byte für Byte gleich, gut 10 s je Runde); `vergleich.py --exe` 581/1176/1154/670/540/822 gleich (tokens, pre, ast, project, defs, diagnose), 0 verschieden.

**M7 begonnen: Ausbau des C-Codegenerators (24.11.2026).** Entschieden (24.11.2026): Der Releasemodus nach R-12.1 entsteht durch Ausbau des C-Codegenerators aus M6, nicht über eine eigene Zwischendarstellung oder LLVM. Der Generator bleibt gegen den Interpreter geprüft (`vergleich_c.py`), der Fixpunkt der Selbstübersetzung muss nach jedem Schritt halten. Ein Zielwert für M7 ist noch festzulegen.

Ausgangsstand (gemessen wie in M6, MSVC `/O2`): `diagnose rpp-compiler` 9,6 s, Selbstübersetzung 10,2 s; `rppc diagnose` (Rust, Release) 0,54 s. Das Profil zeigte 32 % in einer einzigen Binärsuche (`IdSet.lowerBound` in `Flow.rpp`) und 11 % in `rt_deep_copy`.

| Schritt | Umsetzung | `diagnose rpp-compiler` | Selbstübersetzung |
|---|---|---|---|
| Ausgangsstand | – | 9,6 s | 10,2 s |
| native Grundwerte | Reine Ausdrücke über `i8`–`i64`, `char` und `bool` (Literale, lokale Variablen, Felder und Feldketten ohne Sperre, Index, `length`, Operatoren; keine Aufrufe) wertet der Generator direkt in C als `int64_t` aus (`CGen.rpp`, Abschnitt „Native Grundwerte“). Schritte, die scheitern können (Überlauf, Division, Index), stehen als eigene Anweisungen in der Reihenfolge des Interpreters; ihr schneller Fall steht inline in `rpp_rt.h` (`rn_…`), alles andere geht über die bisherigen rv-Pfade und verhält sich damit gleich (Meldungen, Releasemodus). Bedingungen, Zuweisungen, Verbundzuweisungen, `++`/`--`, `return` und Deklarationen mit nativem Wert brauchen keine Marke für Zwischenwerte; Blöcke und Verzweigungen ebenso nicht. | 5,4 s | 6,0 s |
| Kopie von Vektoren | `rt_deep_copy` kopiert Grundwerte, Enums ohne Daten und Strings ohne Aufruf je Element (838 000 Kopien mit 89 Mio. Elementen je `diagnose`) | 4,5 s | |
| Ziele und Variablen | `Checker.rpp` hält den Typ lokaler Grundwert-Ziele fest (native Zuweisung); Grundwert-Variablen werden am Bereichsende nicht freigegeben; `return;`, `break`, `continue` ohne Marke; langsame Fälle der `rn_…` als eigene Funktionen | 3,9 s | 4,4 s |
| Grundwert-Variablen | Lokale Variablen und Parameter vom Typ `i8`–`i64`, `char`, `bool` werden C-Variablen (`int64_t x…`) statt Werte mit Art im Feld `V`. Ausgenommen sind Namen, die ein Lambda erfasst, die `take`/`replace` als Ort brauchen oder die Ziel einer Tupelzuweisung sind (`scanNoNat`); andere Schreibwege (Zuweisung eines nicht nativen Werts, Verbundzuweisung, `++`/`--` als Ausdruck) schreiben die C-Variable direkt. Gelesen als Wert entsteht `rn_box`. | 3,8 s | 4,4 s |
| Speicherverwalter | `xmalloc`/`xrealloc`/`xfree` in `rpp_rt.c`: Blöcke bis 1024 Byte aus Freilisten je Größenklasse (16 Byte), gespeist aus Stücken von 1 MiB; größere über malloc. Der Heap der C-Bibliothek fiel im Profil von 13 % auf unter 1 %. | 3,6 s | 4,1 s |
| schlankere Aufrufe | Virtuelle Aufrufe gehen direkt an die Methode statt über den Verteiler, wenn der Empfänger statisch ein Objekt einer Klasse ist und keine Klasse die Methode überschreibt (dieselbe Prüfung wie beim Erzeugen des Verteilers; im Compiler fallen 3255 von 3279 Verteileraufrufen weg). `rt_enter`/`rt_leave` (Stapel für den Stacktrace) und `rt_is_prim` stehen inline im Header. | 3,3 s | 3,8 s |
| Zwischenwerte einzeln | Zwischenwerte sind einzelne C-Variablen `t0, t1, …` statt eines Felds `T`, sodass der C-Compiler die nicht weitergereichten in Registern halten kann; Argumente, Elemente und Zerlegungen bekommen eigene Felder `A0, A1, …`, die wie die Zwischenwerte je Anweisung wiederverwendet werden (der Stapelrahmen wächst nicht). Gegenprobe: Auch die Variablen (`V`) als einzelne C-Variablen waren rund 1 % langsamer und wurden verworfen. | 3,1 s | 3,6 s |
| Übergabe nach dem statischen Wissen | `rt_temp` entfällt, wenn die vorige Zeile dem Zwischenwert einen Wert ohne Besitz zugewiesen hat (Sicht `rv_view`, Literal, Enum ohne Daten, `null`, Grundwert); das betrifft rund 15 000 von 19 000 Stellen im Compiler. Der Gewinn ist weit größer als der Anteil von `rt_temp` im Profil (rund 1 %), weil die Funktionen kleiner werden und MSVC `rt_temp` in großen Funktionen nicht einbettet. Gegenprobe: Verschieben einer `string`-Variablen bei ihrer letzten Verwendung (Lebendigkeit rückwärts über die Anweisungen) brachte nichts, weil Parameter geliehen ankommen und der Zähler beim Speichern ohnehin steigt; der Aufwand liegt im Speicherzugriff auf den String, nicht in der Fallunterscheidung. Verworfen. | 2,55 s | 2,95 s |
| promote und own_str nach statischem Wissen | `rt_own_str` entfällt nach `rt_promote` und `rt_transfer_var` (die geliehene Strings und `weakb` schon umwandeln); `rt_promote` und `rt_own_str` entfallen direkt nach dem Aufruf einer erzeugten, nicht nativen Methode und nach einem Konstruktor, weil diese immer besitzende Werte liefern (jede Rückgabe läuft über `store`, ein zweites promote wäre wirkungslos). Nach `rt_index` und eingebauten Methoden bleiben sie, dort können Sichten zurückkommen. Im Compiler fallen 1700 von 3300 `rt_promote` und 3200 von 4600 `rt_own_str` weg. | 2,30 s | 2,66 s |
| eingebaute Methoden nach Nummer | Der Generator setzt für eingebaute Methoden (`push`, `contains`, `substring`, …) eine Nummer ein (`rt_bm` mit `BM_…`, Liste `BM_LIST` in `rpp_rt.h` und `builtinMethodId` in `CGen.rpp`) statt des Namens als Zeichenkette; `rt_builtin_method` mit Namen bleibt für älteren erzeugten Code. In `rt_native` stehen Map/Set, `StringBuilder` und `StringView` vorn (eigene Funktionen `native_collection`, `native_string_builder`); vorher lief jeder Aufruf von `Map.get` oder `StringBuilder.append` an rund 45 `strcmp` mit vollen Namen vorbei. Neu gemessen am selben Rechner gegen den vorigen Stand (2,26 s / 2,65 s). | 2,24 s | 2,54 s |
| `Flow.join` linear | Programmarbeit des Compilers, nicht des Generators: `join` baut `lost` und `moved` in je einem Durchlauf über die sortierten Listen (`IdMap::unite`, `IdMap::uniform`) statt als Kopie plus `put` je Eintrag (Binärsuche und `insert` je Schlüssel). Das Ergebnis ist dasselbe wie in `flow.rs`; Moves, die nur auf einem Weg geschahen, bekommen ihre Regel schon beim Zusammenführen. Gemessen gegen den vorigen Stand im selben Lauf (2,25 s / 2,57 s). | 2,01 s | 2,33 s |
| weniger Kopien von Flussständen | Programmarbeit des Compilers: `noteTryState` kopierte nach jeder Anweisung den ganzen Flussstand, auch ohne offenes `try`; `join` liest ihn nur, die Kopie entfällt. `endProbe` übernimmt Stand und Loans aus dem gesicherten `ProbeState` (`replace`), statt sie ein zweites Mal zu kopieren. Laut Profil gingen vorher 9,5 % in `Flow.dup` und ein großer Teil der Freigabe in diese Kopien. Gemessen gegen den vorigen Stand im selben Lauf (2,02 s / 2,33 s). | 1,86 s | 2,17 s |
| `push` ohne Verteiler | Der Generator erzeugt für `push` mit einem Argument `rt_bm_push` (inline in `rpp_rt.h`): Ist der Empfänger ein Vektor, wird das Element direkt angehängt (ohne `rt_bm`, ohne `vec_method`), sonst geht der Aufruf an `rt_bm` wie bisher. Im Compiler 397 Stellen. Gemessen gegen den vorigen Stand im selben Lauf (1,85 s / 2,18 s). | 1,80 s | 2,12 s |
| Probe lokaler Namen | Programmarbeit des Compilers: `argShape` prüft jedes Argument vorab als Probe (wie `arg_shape` in `body.rs`: Flussstand und Loans sichern, prüfen, zurücksetzen). Für einen lokalen Namen, den kein Lambda erfasst, ergibt die Probe `localType` und hinterlässt nichts, was sie nicht selbst zurücksetzt; `argShape` nimmt den Typ dort direkt. Die Probe ganz wegzulassen, wenn es nur eine Fassung ohne Typparameter gibt, wäre nicht gleichwertig: Passt ein Argument nicht, nimmt `invoke` einen anderen Pfad (ohne R-7.4, R-7.27, Zugriff, R-5.10), die Meldungen wichen von `rppc` ab. Verworfen. Gemessen gegen den vorigen Stand im selben Lauf (1,77 s / 2,08 s). Danach bleiben 8,5 % in `probeExpr`, davon 3,4 % allein Sichern und Zurücksetzen (`beginProbe`, `endProbe`). | 1,73 s | 2,04 s |
| Strings inline | Nur die Laufzeit (`rpp_rt.h`), der erzeugte Code bleibt Byte für Byte gleich: `rt_equal` vergleicht zwei Strings inline (gleicher Zeiger oder Länge und `memcmp`) statt über `rt_equal_slow`; `rt_lit` liefert ein schon angelegtes Literal inline (`rt_lit_new` legt es beim ersten Gebrauch an); `rt_promote` zählt einen geliehenen String inline hoch; `rt_drop` zählt einen String inline herunter und ruft `rt_drop_slow` nur noch zum Freigeben. Der Gewinn ist größer als die Anteile im Profil (`rt_equal_slow` gut 2 %, `rt_promote_slow` 5 %), weil die Aufrufe in großen Funktionen sonst nicht eingebettet werden. Gemessen gegen den vorigen Stand im selben Lauf (1,73 s / 2,04 s). | 1,53 s | 1,82 s |

Prüfung des Generators: Mit `RPP_NATCHECK` (C-Makro) prüft jeder native Lesezugriff, dass der Wert die erwartete Art hat; damit laufen der übersetzte Compiler selbst und `vergleich_c.py --cflags -DRPP_NATCHECK`. `vergleich_c.py` kann mit `--exe` den übersetzten Compiler statt des Interpreters zum Erzeugen nehmen.

Stand (nach Grundwert-Variablen und Speicherverwalter): Fixpunkt der Selbstübersetzung (Runde 2 und 3 Byte für Byte gleich, auch mit `RPP_NATCHECK`); `vergleich_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 290 gleich, 0 verschieden; `vergleich.py --exe` 591/1196/1174/690/550/832 gleich (tokens, pre, ast, project, defs, diagnose), 0 verschieden; die Laufzeit übersetzt auch mit gcc.

Verbleibendes Profil (nach dem Speicherverwalter): rund 66 % im erzeugten Code, verteilt auf viele kleine Methoden (Rahmen je Aufruf, Argumente als Feld von Werten, Zwischenwerte mit Art), gut 30 % Laufzeit, davon etwa 15 % Freigabe und Übergabe (`rt_promote_slow`, `rt_drop_slow`, `destruct`, `rt_temp`, `rt_transfer_var`, `share`). Nach der Übergabe nach dem statischen Wissen (Profil mit Aufrufern, `RPP_PROFILE`): Die teuersten Laufzeitanteile sind Programmarbeit des Compilers, etwa `rt_deep_copy` aus `IdMap.dup` (Kopie von `string[]`), Zähler der Strings in `IdMap.put` und `destruct` beim Freigeben der Flussstände. Stand (nach den eingebauten Methoden nach Nummer, 25.11.2026): Fixpunkt der Selbstübersetzung (Runde 2 und 3 Byte für Byte gleich); `vergleich_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 296 gleich, 0 verschieden; `vergleich.py --exe` 596/1206/1184/696/553/835 gleich (tokens, pre, ast, project, defs, diagnose), 0 verschieden. Die Vergleiche brauchen ein frisch gebautes `rppc` (die Meldungen sind seit dem 25.11.2026 englisch; ein älteres `rppc.exe` ergibt Hunderte scheinbarer Abweichungen).

Die direkten Aufrufe eingebauter Methoden brachten weniger als erwartet (rund 1 % bei `diagnose`, 4 % bei der Selbstübersetzung); der Namensvergleich war kein großer Posten. Nächster Kandidat: `rt_temp` nach Rechnungen, deren Ergebnis statisch kein String ist; vorher ein neues Profil (`RPP_PROFILE`).

Profil (25.11.2026, `diagnose rpp-compiler`, 1272 Stichproben je 1 ms, mit Aufrufern): `Flow.join` 23 % einschließlich, darin `IdMap.put` 15 % (Binärsuche, `insert` und Übergabe der Strings: `rt_transfer_var`/`share`/`rt_promote_slow` aus `IdMap.put` allein 4 %) und `IdMap.dup` 9 % (`rt_deep_copy` von `string[]`, 7 % eigene Zeit). Freigabe (`destruct`, `rt_drop_slow`) 14 %, verteilt auf viele Stellen (`probeExpr`, `noteTryState`, `endProbe`, …). `rt_native` und `rt_bm` zusammen gut 1 %. Die Selbstübersetzung zeigt dasselbe Bild (`Flow.join` 21 %). `join` baut die Vereinigung von `lost` und `moved` als Kopie plus `put` je Eintrag, also mit Binärsuche und Einfügen je Schlüssel statt eines Durchlaufs über zwei sortierte Listen (behoben, siehe `Flow.join` linear in der Tabelle).

Stand (nach `Flow.join` linear, 25.11.2026): Fixpunkt der Selbstübersetzung; `diagnose rpp-compiler` gibt dieselbe Ausgabe wie vorher; `vergleich_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 296 gleich, 0 verschieden; `vergleich.py --exe` 596/1206/1184/696/553/835 gleich, 0 verschieden.
