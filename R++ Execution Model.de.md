# R++ Ausführungsmodell (Kapitel 12)

[English](R%2B%2B%20Execution%20Model.md)

Stand: 21.11.2026, Entwurf 0.2. Dieses Dokument füllt Kapitel 12 des „R++ Spezifikationsgerüsts“. Es regelt, wie Debug- und Releasemodus zusammenhängen, welches Verhalten eines Programms beobachtbar ist, worin sich die Modi unterscheiden dürfen, wie ein Programm startet und endet und was bei erschöpften Ressourcen geschieht.

Alle Festlegungen sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 1 und 25). Jede normative Regel trägt eine Nummer `R-12.n`. Die Konformitätstests liegen in `conformance/ch12_execution/`.

## 0. Ergänzungen beim Ausformulieren (angenommen am 21.11.2026)

| Nr. | Vorschlag |
|---|---|
| V-12.1 | `args` in `main` enthält nur die Argumente, nicht den Programmnamen (wie Java und C#). |
| V-12.2 | Jede Implementierung garantiert mindestens 10 000 verschachtelte Methodenaufrufe, bevor ein Stacküberlauf eintreten darf. |

## 1. Zwei Modi, eine Sprache

**R-12.1 (Modi).** Ein Programm wird im **Debugmodus** ausgeführt (interpretiert oder per JIT in einer virtuellen Maschine, für kurze Entwicklungszyklen) oder im **Releasemodus** zu einem nativen Programm übersetzt.

**R-12.2 (Gemeinsamer Weg).** Beide Modi durchlaufen dasselbe Frontend: bedingte Kompilierung, Parsen, Namensauflösung, Typprüfung, Ownership- und Null-Prüfung. Ein Programm, das in einem Modus abgelehnt wird, wird auch im anderen abgelehnt, außer der Unterschied liegt in einem Zweig, der nur in einem Modus aktiv ist (`#if debug`). Beide Modi arbeiten mit derselben Zwischenrepräsentation.

**R-12.3 (Referenz).** Der Debug-Interpreter ist die Referenzimplementierung. Der Release-Compiler wird gegen ihn getestet; bei abweichendem beobachtbaren Verhalten (R-12.4) außerhalb der zulässigen Unterschiede (R-12.5) ist der Release-Compiler fehlerhaft.

## 2. Beobachtbares Verhalten

**R-12.4 (Was beobachtbar ist).** Beobachtbar sind
- die Ausgabe auf Standard- und Fehlerausgabe in ihrer Reihenfolge,
- die Inhalte geschriebener Dateien und Verzeichnisse,
- die gelesene Eingabe in ihrer Reihenfolge,
- der Exit-Code und ob das Programm endet.

Nicht beobachtbar sind Laufzeit, Speicherverbrauch, Adressen von Objekten und die Reihenfolge von Vorgängen, die keine der obigen Wirkungen haben.

**R-12.5 (Zulässige Unterschiede).** Debug- und Releasemodus müssen dasselbe beobachtbare Verhalten zeigen. Unterscheiden dürfen sie sich nur in diesen Punkten:

| Punkt | Debug | Release | Regel |
|---|---|---|---|
| Ganzzahlüberlauf | Programmfehler | Rechnung modulo 2ⁿ | R-4.14 |
| Schiebeweite außerhalb des Bereichs | Programmfehler | modulo Bitbreite | R-4.16 |
| `cast` zwischen Ganzzahlen außerhalb des Zielbereichs | Programmfehler | untere Bits | R-4.9 |
| `cast` von Gleitkomma außerhalb des Bereichs, NaN | Programmfehler | Sättigung, NaN → 0 | R-4.10 |
| Stacktrace in Meldungen | immer | der Implementierung überlassen | R-9.16 |
| Stacküberlauf, Speichermangel | Programmfehler | Programmfehler, aber an anderer Stelle möglich | R-12.10 |
| Code in `#if debug` bzw. `#if release` | aktiv | inaktiv bzw. umgekehrt | R-11.24 |
| Fehlende C-Bibliothek oder fehlendes Symbol | Programmfehler vor `main` (`NativeError`) | Fehler beim Bauen | R-15.12 |

Weil Programmfehler nicht gefangen werden können (Kapitel 9), führt keiner dieser Unterschiede dazu, dass ein Programm in beiden Modi verschiedene `catch`-Zweige nimmt.

**R-12.6 (Kein undefiniertes Verhalten).** Für jedes übersetzte Programm legt die Spezifikation das beobachtbare Verhalten fest, bis auf die Punkte in R-12.5. Es gibt kein undefiniertes Verhalten: keine ungültigen Speicherzugriffe (Kapitel 5), keine Null-Dereferenzierung (Kapitel 6), keine uninitialisierten Werte (R-4.46), eine feste Auswertungsreihenfolge (R-8.2) und festgelegtes Verhalten bei Überlauf. Für Programme mit C-Code gilt das nur unter den Bedingungen aus R-15.16.

**R-12.7 (Gleitkomma).** Gleitkommaoperationen werden in beiden Modi bitgenau nach IEEE 754 ausgeführt: jede Operation einzeln mit Rundung zum nächsten Wert, ohne Zusammenfassen zu einer FMA-Operation und ohne erweiterte Zwischengenauigkeit. Debug und Release liefern dieselben Bits. Mathematische Funktionen der Standardbibliothek (soweit vorhanden) liefern in beiden Modi dieselben Ergebnisse.

## 3. Programmstart und -ende

**R-12.8 (Start).** Ein Programm beginnt mit dem Aufruf von `main(args)`. `args` enthält die Kommandozeilenargumente ohne den Programmnamen (V-12.1), als `string`-Werte in UTF-8. Es gibt keine globalen Variablen und keine Initialisierung vor `main`: `static const`-Werte stehen zur Compilezeit fest.

**R-12.9 (Ende).** Das Programm endet
- mit dem Rückgabewert von `main` als Exit-Code, nachdem die lokalen Variablen von `main` freigegeben sind;
- mit Exit-Code 1, wenn eine Exception `main` verlässt (R-9.15);
- mit Exit-Code 2 bei einem Programmfehler (R-9.14).

Eine Funktion zum vorzeitigen Beenden gibt es nicht (V-13.3). Ausgaben werden vor dem Ende vollständig geschrieben.

**R-12.10 (Ressourcen).** Ein Stacküberlauf und fehlender Speicher sind Programmfehler: Das Programm endet mit einer Meldung und Exit-Code 2. Aufgeräumt wird, soweit es noch möglich ist; nach Speichermangel verlangt die Spezifikation kein Unwinding. Jede Implementierung garantiert mindestens 10 000 verschachtelte Methodenaufrufe (V-12.2).

## 4. Speicher und Threads

**R-12.11 (Freigabe).** Objekte werden deterministisch freigegeben, sobald ihr Besitzer sie aufgibt (Kapitel 5); es gibt keine Garbage Collection. Destruktoren laufen in der festgelegten Reihenfolge (V-7). Objekte in Zyklen aus `shared`-Verweisen werden nie freigegeben.

**R-12.12 (Threads).** Version 1 hat keine Bibliothek für Threads; ein Programm läuft in einem Thread. Für spätere Versionen gelten schon jetzt: Jedes Objekt gehört zu genau einem Thread; an einen anderen Thread geht nur ein `own`-Wert, dessen Besitzbaum keine `shared`- oder `weak`-Verweise enthält (Kapitel 5, Abschnitt 7). Strings sind davon ausgenommen und brauchen dann einen atomaren Zähler.

## 5. Konformitätstests

Die Laufzeittests liegen in `conformance/ch12_execution/runtime/` und legen das erwartete Verhalten je Modus fest (Format: Kapitel 14). Solange es keine Implementierung gibt, ist an ihnen nur die Syntax prüfbar.

| Regeln | Laufzeittest |
|---|---|
| R-12.5 Überlauf | `l01_overflow.rpp` |
| R-12.5 cast | `l02_cast.rpp` |
| R-12.7 Gleitkomma | `l03_floating_point.rpp` |
| R-12.9 Exit-Codes | `l04_exit_code.rpp`, `l05_exception_from_main.rpp`, `l06_program_error.rpp` |
| R-12.11 Freigabereihenfolge | `l07_destructors.rpp` |
