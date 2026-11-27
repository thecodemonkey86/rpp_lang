# R++ Exceptions (Kapitel 9)

[English](R%2B%2B%20Exceptions.md)

Stand: 21.11.2026, Entwurf 0.2. Dieses Dokument füllt Kapitel 9 des „R++ Spezifikationsgerüsts“. Es regelt, was geworfen und gefangen werden kann, wie `try`, `catch` und `finally` ablaufen, was beim Unwinding passiert und wie Programmfehler das Programm beenden. Für Besitz und Freigabe gilt Kapitel 5, für Verengung Kapitel 6 (R-6.12), für Erreichbarkeit Kapitel 8.

Alle Festlegungen sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 11 und 22). Jede normative Regel trägt eine Nummer `R-9.n`. Die Konformitätstests liegen in `conformance/ch09_exceptions/`.

## 0. Ergänzung beim Schreiben der Laufzeittests (angenommen am 21.11.2026)

| Nr. | Vorschlag |
|---|---|
| V-9.1 | Jede Art von Programmfehler hat einen festen Namen, der in der Meldung erscheint (Tabelle in R-9.2); Laufzeittests prüfen ihn. Stacküberlauf und Speichermangel stehen als Art „Ressourcen“ in der Tabelle. |

## 1. Zwei Arten von Fehlern zur Laufzeit

**R-9.1 (Exceptions und Programmfehler).**
- Eine **Exception** wird mit `throw` ausgelöst und kann mit `catch` gefangen werden. Exceptions sind ungeprüft: Methoden geben nicht an, was sie werfen.
- Ein **Programmfehler** wird von der Sprache selbst oder von `fail` ausgelöst. Er kann nicht gefangen werden: Das Programm räumt noch auf (R-9.14) und endet dann.

**R-9.2 (Programmfehler).** Programmfehler sind:

| Art | Auslöser | Modus | Name in der Meldung (V-9.1) |
|---|---|---|---|
| Index | Index oder Bereich außerhalb der Grenzen (Arrays, Vektoren, Slices, Strings) | beide | `IndexError` |
| Überlauf | Ganzzahlüberlauf, Schiebeweite außerhalb des Bereichs | nur Debug (Release: Wraparound bzw. modulo) | `OverflowError` |
| Division | Ganzzahldivision oder `%` durch 0 | beide | `DivisionError` |
| Umwandlung | `cast` außerhalb des Zielbereichs, NaN zu Ganzzahl | nur Debug (Release: Abschneiden bzw. Sättigung) | `CastError` |
| Umwandlung | `cast<char>` ohne gültigen Skalarwert, `cast<E>` ohne passende Variante | beide | `CastError` |
| Zusicherung | `assert(c, text)` mit `c == false` | beide | `AssertError` |
| Sperre | Konflikt beim Zugriff auf ein `shared`-Objekt (Kapitel 5, V-1) | beide | `BorrowError` |
| Destruktor | eine Exception verlässt einen Destruktor (V-11) | beide | `DestructorError` |
| Abbruch | Aufruf von `fail(text)` (R-9.19) | beide | `FailError` |
| Ressourcen | Stacküberlauf, fehlender Speicher (Kapitel 12, R-12.10) | beide | `StackOverflowError`, `OutOfMemoryError` |
| C-Anbindung | ein Argument oder Ergebnis, das sich nicht umwandeln lässt, eine fehlende Bibliothek oder ein fehlendes Symbol beim Start (Kapitel 15) | beide (fehlende Bibliothek im Releasemodus: Fehler beim Bauen, R-12.5) | `NativeError` |

**R-9.3 (Sprechweise in anderen Kapiteln).** Wo die Kapitel 4 bis 8 für diese Prüfungen sagen, dass „eine Exception geworfen wird“, ist ein Programmfehler nach R-9.2 gemeint. Programmfehler sind keine Klassen; ihre Namen (`IndexError`, `OverflowError`, `BorrowError` usw.) erscheinen nur in der Fehlermeldung.

Weil ein Programmfehler nicht gefangen werden kann, verzweigen Debug- und Releasemodus nie unterschiedlich, nur weil eine Prüfung im Debugmodus anschlägt.

## 2. Die Klasse Exception

**R-9.4 (Exception).** Die Standardbibliothek stellt die Klasse `rpp.core.Exception` bereit:

```
export class Exception {
public:
    string message;
    Exception? cause;            // Ursache, besitzt sie
    Exception[] suppressed;      // während des Unwindings unterdrückt (R-9.10)
    constructor(string message);
    constructor(string message, own Exception cause);
    virtual string describe();   // Nachricht mit Ursachen und unterdrückten Exceptions
}
```

Eigene Exceptions erben direkt oder indirekt von `Exception` und können weitere Felder haben.

## 3. throw

**R-9.5 (throw).** `throw e;` verlangt einen Ausdruck vom Typ `own C` mit `C <: Exception`. Der Wert wird verschoben; ein `shared`-Wert, ein Borrow oder ein nullbarer Wert kann nicht geworfen werden. `throw` ist eine Anweisung, die nicht normal endet (Kapitel 6, Abschnitt 3.3).

## 4. try, catch, finally

**R-9.6 (Aufbau).** Ein `try` hat mindestens ein `catch` oder ein `finally`. Der Typ in `catch (C e)` ist eine Klasse `C <: Exception`, ohne `?` und ohne Besitzform.

**R-9.7 (Auswahl).** Entsteht im `try`-Block eine Exception, wird der erste `catch` gewählt, dessen Typ eine Oberklasse der tatsächlichen Klasse der Exception ist. `catch (Exception e)` fängt jede Exception. Ein `catch`, der hinter einem `catch` für eine seiner Oberklassen steht, kann nie gewählt werden; der Compiler meldet ihn als unerreichbaren Code (Warnung, R-8.27). Passt kein `catch`, läuft die Exception weiter.

**R-9.8 (catch-Variable).** Die Variable eines `catch` besitzt die Exception (`own`). `throw e;` wirft sie weiter (Move); sonst wird sie am Ende des `catch`-Blocks freigegeben.

**R-9.9 (finally).** Der `finally`-Block läuft immer, wenn der `try`-Block oder ein `catch`-Block verlassen wird: am normalen Ende, durch `return`, `break` oder `continue` und während eine Exception läuft. Danach geht es so weiter, wie es ohne `finally` weitergegangen wäre.

**R-9.10 (Verlassen von finally).** Ein `finally`-Block darf nicht mit `return` verlassen werden; `break` und `continue` dürfen ihn nicht verlassen (innerhalb einer Schleife im `finally` sind sie erlaubt). Ein `throw` direkt im `finally`-Block ist ein Fehler. Wirft aber ein Aufruf im `finally`-Block, während schon eine Exception läuft, ersetzt die neue Exception die alte; die alte wird an das Feld `suppressed` der neuen angehängt und geht nicht verloren.

## 5. Unwinding

**R-9.11 (Ablauf).** Läuft eine Exception, werden die umgebenden Blöcke von innen nach außen verlassen, bis ein passender `catch` gefunden ist. Beim Verlassen jedes Blocks und jeder Methode
1. werden die dort deklarierten Variablen in umgekehrter Reihenfolge freigegeben (Kapitel 5; verschobene Variablen werden dank Drop-Flag übersprungen),
2. werden die Sperren von `shared`-Objekten aufgehoben, die der Block hält (Kapitel 5, Abschnitt 5.2),
3. läuft ein vorhandener `finally`-Block.

**R-9.12 (Destruktoren).** Destruktoren laufen beim Unwinding wie beim normalen Verlassen eines Blocks. Verlässt eine Exception einen Destruktor, ist das ein Programmfehler (R-9.2).

**R-9.13 (Konstruktoren).** Wirft ein Konstruktor, ist das Objekt nie fertig geworden. Freigegeben werden die bereits gesetzten Felder und, falls der Basiskonstruktor schon fertig war, der Basisanteil; dabei laufen die Destruktoren der Basisklassen. Der Destruktor der Klasse selbst läuft nicht. Für `copy` gilt dasselbe.

## 6. Programmfehler und Programmende

**R-9.14 (Abbruch).** Bei einem Programmfehler wird wie in R-9.11 aufgeräumt: Variablen werden freigegeben, Destruktoren und `finally`-Blöcke laufen, aber kein `catch` wird gewählt. Danach endet das Programm mit einer Meldung auf der Fehlerausgabe und dem Exit-Code **2**. Entsteht beim Aufräumen ein weiterer Programmfehler oder verlässt eine Exception einen Destruktor oder `finally`-Block, endet das Programm sofort.

**R-9.15 (Ungefangene Exception).** Verlässt eine Exception `main`, endet das Programm nach dem Aufräumen mit `describe()` der Exception auf der Fehlerausgabe und dem Exit-Code **1**.

**R-9.16 (Stacktrace).** Im Debugmodus tragen jede Exception und jeder Programmfehler einen Stacktrace, der in der Meldung erscheint. Im Releasemodus ist ein Stacktrace der Implementierung überlassen.

## 7. Zusammenspiel mit anderen Kapiteln

**R-9.17 (Moves und Zuweisungen im try).** Für die Prüfung von `catch`- und `finally`-Blöcken und der Anweisungen danach gilt eine Variable als verschoben, wenn sie an irgendeiner Stelle des `try`-Blocks verschoben werden kann, und als zugewiesen nur, wenn sie es schon vor dem `try` war. Zur Laufzeit entscheiden Drop-Flags (Kapitel 5, V-5). Für Verengung gilt R-6.12.

**R-9.18 (Kein try-with-resources).** Es gibt kein `using` und kein try-with-resources. Ressourcen wie Dateien gibt ihr Destruktor frei; er läuft auch beim Unwinding.

**R-9.19 (fail).** Die eingebaute Funktion `fail(string text)` löst einen Programmfehler mit der Meldung `text` aus und kehrt nie zurück. Ein Aufruf von `fail` endet deshalb nicht normal: Er zählt für Verengung (Kapitel 6, Abschnitt 3.3), für die sichere Rückgabe (R-8.26) und für unerreichbaren Code (R-8.27) wie `throw`. `assert` zählt nicht so, weil es normal endet, wenn die Bedingung erfüllt ist.

## 8. Beispiel

```
class ParseError : Exception {
public:
    i64 line;
    constructor(string message, i64 line) : base(message) {
        this.line = line;
    }
}

class Loader {
public:
    Config load(string path) {
        File f = File::open(path);        // Destruktor schließt die Datei, auch bei Exceptions
        try {
            return parse(f.readAll());
        } catch (ParseError e) {
            throw new Exception("Konfiguration ungültig: " + path, e);   // e wird zur Ursache
        }
    }

    Config parse(string text) {
        if (text.length == 0) {
            throw new ParseError("leer", 1);
        }
        …
    }
}
```

## 9. Konformitätstests

Die Testprogramme liegen in `conformance/ch09_exceptions/`; geprüft werden sie mit `rppc test` (erste Implementierung, Meilenstein M3).

| Regeln | Akzeptiert | Abgelehnt |
|---|---|---|
| R-9.4, R-9.5 Exception, throw | `a01_throw_and_catch.rpp` | `n01_non_exception_type.rpp`, `n02_throw_shared.rpp` |
| R-9.6 – R-9.8 catch | `a01_throw_and_catch.rpp`, `a02_rethrow.rpp` | `n03_catch_non_exception_type.rpp`, `n04_used_after_rethrow.rpp` |
| R-9.7 unerreichbarer catch | `a04_warning.rpp` | – |
| R-9.9, R-9.10 finally | `a01_throw_and_catch.rpp` | `n05_return_in_finally.rpp`, `n06_throw_in_finally.rpp`, `n07_break_from_finally.rpp` |
| R-9.17 Moves im try | `a02_rethrow.rpp` | `n08_move_in_try.rpp` |
| R-9.19 fail, assert | `a03_fail.rpp` | `n09_assert_does_not_exit.rpp` |
