# R++ Standardbibliothek (Kapitel 13)

Stand: 21.11.2026, Entwurf 0.2. Dieses Dokument füllt Kapitel 13 des „R++ Spezifikationsgerüsts“ mit einem **ersten Schnitt**: genau dem, was ein Compiler für R++ braucht, der in R++ geschrieben ist und sich selbst übersetzt. Threads, Netz, Zeit, Zufall, reguläre Ausdrücke und Datenformate wie JSON gehören nicht dazu.

Normativ sind in diesem Entwurf nur die Grundsätze (Abschnitt 1) und die Teile, auf die sich andere Kapitel stützen (`Exception`, `Equatable`, `Comparable`, `Hashable`, `Iterable`). Die übrigen Schnittstellen sind ein Arbeitsstand: Die Bibliotheken unter `rpp` bleiben bis zum Abschluss der Selbstübersetzung auf Version 0.x und dürfen sich ändern.

Grundsätze, die beim Schreiben zusätzlich festgelegt werden mussten, stehen in Abschnitt 0; sie sind am 21.11.2026 alle angenommen worden.

## 0. Ergänzende Festlegungen (angenommen am 21.11.2026)

| Nr. | Festlegung |
|---|---|
| V-13.1 | Die Standardbibliothek besteht aus mehreren Bibliotheken unter dem Besitzer `rpp`: `rpp.core`, `rpp.text`, `rpp.fmt`, `rpp.collections`, `rpp.io`, `rpp.env`. Sie werden mit dem Compiler ausgeliefert und brauchen keinen Eintrag in `rpp.yaml`. Die Typen von `rpp.core` sind überall ohne `using` sichtbar; die anderen werden mit `using` eingebunden. |
| V-13.2 | Iteration ist deterministisch: `Map` und `Set` liefern ihre Einträge in Einfügereihenfolge, `Dir::list` liefert sortierte Namen. Damit baut ein Compiler bei gleicher Eingabe immer gleiche Ausgabe. |
| V-13.3 | Es gibt keine Funktion zum sofortigen Beenden (`exit`). Den Exit-Code liefert `main`; so laufen alle Destruktoren. |
| V-13.4 | `print(text)` schreibt `text` und einen Zeilenumbruch auf die Standardausgabe. |
| V-13.5 | Lesender Zugriff auf einen fehlenden Schlüssel liefert `null` (`ref V? get(K key)`); veränderbarer Zugriff mit `at` verlangt einen vorhandenen Schlüssel und löst sonst einen Programmfehler aus (wie ein Index außerhalb der Grenzen). |
| V-13.6 | Eine Borrow-Methode darf auch einen Borrow zurückgeben, der über eine `ref`-Variable, ein Schleifenelement oder einen weiteren Borrow-Aufruf von `this` ausgeht (z. B. aus einer Schleife über `this.entries`). Das erweitert R-5.23 in der Oberflächensprache; die Sicherheitsbegründung ist dieselbe. |
| V-13.7 | Für Typen mit `Comparable` sind `<`, `<=`, `>`, `>=` erlaubt und rufen `compareTo` auf (Erweiterung von Kapitel 4, R-4.17, analog zu `==` und `Equatable`). |

## 1. Grundsätze

**R-13.1 (Aufbau).** Die Standardbibliothek besteht aus den Bibliotheken in Abschnitt 2 bis 7. Sie stehen unter dem reservierten Besitzer `rpp` (Designentscheidungen, Abschnitt 13b); niemand sonst kann unter `rpp` veröffentlichen.

**R-13.2 (Version).** Bis ein in R++ geschriebener Compiler sich selbst übersetzt, tragen die Bibliotheken unter `rpp` Versionen 0.x. Ihre Schnittstellen dürfen sich in dieser Zeit ohne neue Hauptversion ändern.

**R-13.3 (Methoden an eingebauten Typen).** Primitive Typen und `string` haben eingebaute Methoden und statische Member; sie sind in den Abschnitten 2 bis 4 aufgeführt und werden am Wert bzw. mit `::` am Typ aufgerufen: `n.toString()`, `i32::parse(s)`, `i64::MAX`.

**R-13.4 (Fehler).** Ungültige Eingaben, die ein Programm erwarten muss (Text, der keine Zahl ist; Datei fehlt), melden die Bibliotheken mit Exceptions. Wo es eine bequeme Alternative gibt, liefert eine `try…`-Variante stattdessen `null`. Verstöße gegen Vorbedingungen (Index, fehlender Schlüssel bei `at`) sind Programmfehler (Kapitel 9).

## 2. rpp.core

Ohne `using` überall sichtbar (V-13.1).

### 2.1 Exceptions

**R-13.5.** `Exception` ist in Kapitel 9 (R-9.4) beschrieben. Dazu kommen:

```
export class FormatException : Exception { … }     // ungültiger Text beim Parsen
export class IOException : Exception {             // Fehler bei Dateien und Verzeichnissen
public:
    string path;
}
export class ArgumentException : Exception { … }   // ungültiges Argument, das kein Programmfehler ist
```

### 2.2 Mixins der Sprache

**R-13.6 (Equatable).**

```
export mixin Equatable<T> {
    nodiscard bool equals(T other);
}
```

`a == b` und `a != b` rufen `equals` auf (Kapitel 4, R-4.24). Primitive Typen, `string` und Enums ohne Daten erfüllen `Equatable` von selbst.

**R-13.7 (Comparable).**

```
export mixin Comparable<T> {
    import Equatable<T>;
    nodiscard i32 compareTo(T other);          // < 0, 0, > 0
    bool less(T other) { return compareTo(other) < 0; }
}
```

Für Typen mit `Comparable` sind `<`, `<=`, `>`, `>=` erlaubt und rufen `compareTo` auf (V-13.7). Zahltypen, `char`, `string` und `byte` erfüllen `Comparable` von selbst.

**R-13.8 (Hashable).**

```
export mixin Hashable {
    nodiscard i64 hash();
}
```

Gleiche Objekte (`equals`) müssen gleiche Hashwerte haben. Primitive Typen, `string` und Enums ohne Daten erfüllen `Hashable` von selbst. Hashwerte sind innerhalb eines Programmlaufs stabil; ob sie zwischen Läufen gleich sind, bleibt offen.

### 2.3 Iteration

**R-13.9 (Iterable).** Klassen werden mit einem **Cursor-Protokoll** iterierbar. Der Cursor ist ein gewöhnlicher Wert (z. B. ein `i64`-Index oder ein `shared`-Knoten), kein Borrow; einen Borrow auf ein Element liefert erst `at`.

```
export mixin Iterable<T, C> {
    nodiscard C begin();
    nodiscard bool done(C cursor);
    nodiscard C next(C cursor);
    ref T at(C cursor);
}

export mixin MutIterable<T, C> {
    import Iterable<T, C>;
    ref T& atMut(C cursor) &;
}
```

**R-13.10 (for-each über Iterable).** `for (T x : c) S` mit `c` vom Typ `Iterable<T, C>` bedeutet

```
{
    ref Coll k = c;                                   // c bleibt für die ganze Schleife geliehen
    for (C cur = k.begin(); !k.done(cur); cur = k.next(cur)) {
        ref T x = k.at(cur);
        S
    }
}
```

`for (T& x : &c) S` verlangt `MutIterable<T, C>`, leiht `c` veränderbar und bindet `ref T& x = &k.atMut(cur);`. `break`, `continue` und `return` in `S` funktionieren wie in jeder Schleife. Während der Schleife kann `c` nicht verändert werden (Kapitel 5, Satz 5.2).

## 3. Eingebaute Methoden der primitiven Typen und von string

**R-13.11 (Zahlen).** Für jeden Ganzzahl- und Gleitkommatyp `N`:

| Member | Bedeutung |
|---|---|
| `N::MIN`, `N::MAX` | kleinster und größter Wert (bei Gleitkomma: größter endlicher) |
| `n.toString()` | Dezimaldarstellung; bei Gleitkomma die kürzeste, die beim Einlesen genau `n` ergibt |
| `N::parse(string s)` | liest eine Zahl in der Schreibweise der Literale (Kapitel 2), ohne Trennzeichen `_`; wirft `FormatException`, wenn `s` keine gültige Zahl ist oder nicht in `N` passt |
| `N::tryParse(string s)` | wie `parse`, liefert aber `N?` (`null` statt Exception) |
| `n.abs()`, `a.min(b)`, `a.max(b)` | Betrag, Minimum, Maximum; `abs` von `N::MIN` ist ein Überlauf (Kapitel 4, R-4.14) |

Gleitkomma-Text wird exakt gerundet eingelesen (IEEE 754, zur nächsten darstellbaren Zahl).

**R-13.12 (char).** `c.isLetter()`, `c.isDigit()`, `c.isWhitespace()`, `c.isUpper()`, `c.isLower()` nach den Unicode-Eigenschaften; `c.toUpper()`, `c.toLower()`; `c.toString()`.

**R-13.13 (string).** Neben `length`, `s[i]`, `byteLength` und `byteAt` (Kapitel 4, R-4.19):

| Methode | Ergebnis |
|---|---|
| `substring(i64 start, i64 end)` | Zeichen von `start` bis vor `end` |
| `indexOf(string part)` | `i64?`, Position des ersten Vorkommens oder `null` |
| `startsWith`, `endsWith`, `contains` | `bool` |
| `split(string sep)` | `string[]` |
| `trim()`, `toUpper()`, `toLower()` | `string` |
| `replace(string old, string new)` | `string`, alle Vorkommen ersetzt |
| `bytes()` | `byte[]`, UTF-8-Bytes als Kopie |
| `string::fromUtf8(Slice<byte> b)` | `string`; wirft `FormatException` bei ungültigem UTF-8 |
| `string::join(string sep, Slice<string> parts)` | `string` |

Strings sind `Equatable`, `Comparable` und `Hashable`.

## 4. rpp.text

**R-13.14 (StringBuilder).** Baut einen String schrittweise auf:

```
export class StringBuilder {
public:
    constructor();
    nodiscard i64 length();                       // Anzahl Zeichen bisher
    void append(string s) &;
    void appendChar(char c) &;
    void appendLine(string s) &;                  // mit Zeilenumbruch
    void clear() &;
    nodiscard string toString();
}
```

## 5. rpp.fmt

**R-13.15.** Hilfen für Textausgabe, soweit sie über `toString()` hinausgehen:

| Funktion | Ergebnis |
|---|---|
| `Fmt::padLeft(string s, i64 width)`, `Fmt::padRight(…)` | mit Leerzeichen auf `width` Zeichen aufgefüllt |
| `Fmt::hex(i64 n)`, `Fmt::hex(ui64 n)` | hexadezimal, ohne Präfix |
| `Fmt::fixed(f64 x, i32 digits)` | Gleitkomma mit fester Anzahl Nachkommastellen, korrekt gerundet |

## 6. rpp.collections

Vektoren (`T[]`) sind eingebaut. Ihre Methoden und die Sammlungen `Map` und `Set` stehen hier.

**R-13.16 (Vektoren).** Für `T[] v`:

| Methode | Bedeutung |
|---|---|
| `push(own T x) &`, `insert(i64 i, own T x) &` | anfügen, einfügen |
| `pop() &`, `remove(i64 i) &` | `own T`, entfernt das letzte bzw. das `i`-te Element |
| `clear() &` | alle Elemente freigeben |
| `ref T? first()`, `ref T? last()` | Borrow auf das erste bzw. letzte Element, `null` bei leerem Vektor |
| `contains(T x)`, `indexOf(T x)` | verlangen `T : Equatable<T>`; `indexOf` liefert `i64?` |
| `sort() &` | aufsteigend, stabil; verlangt `T : Comparable<T>` |
| `sortBy(Func<T, T, i32> cmp) &` | stabil nach `cmp` |

`pop` und `remove` auf einem leeren Vektor bzw. mit ungültigem Index sind Programmfehler.

**R-13.17 (Map).**

```
export class Map<K : Hashable & Equatable<K>, V> {
    import Iterable<Entry<K, V>, i64>;
public:
    constructor();
    nodiscard i64 length();
    nodiscard bool contains(K key);
    ref V? get(K key);                        // null, wenn key fehlt (V-13.5)
    ref V& at(K key) &;                       // Programmfehler, wenn key fehlt
    void put(own K key, own V value) &;       // ersetzt einen vorhandenen Wert (der alte wird freigegeben)
    V? remove(K key) &;                       // entfernt und liefert den Wert
    void clear() &;
}

export class Entry<K, V> {
public:
    K key;
    V value;
}
```

Einträge werden in Einfügereihenfolge durchlaufen (V-13.2); `put` mit einem vorhandenen Schlüssel behält die Position.

**R-13.18 (Set).** `Set<T : Hashable & Equatable<T>>` mit `add(own T x) &`, `contains(T x)`, `remove(T x) &`, `clear() &`, `length()`; iterierbar in Einfügereihenfolge.

## 7. rpp.io und rpp.env

**R-13.19 (Dateien).**

```
export class File {
public:
    static own File open(string path);         // wirft IOException
    nodiscard string readAllText();            // UTF-8; wirft FormatException bei ungültigem UTF-8
    nodiscard byte[] readAllBytes();
    static void writeText(string path, string text);
    static void writeBytes(string path, Slice<byte> data);
    destructor();                              // schließt die Datei
}
```

**R-13.20 (Verzeichnisse und Pfade).** `Dir::list(string path)` liefert die Namen der Einträge sortiert (V-13.2); `Dir::exists`, `Dir::create`. `Path::join(a, b)`, `Path::fileName(p)`, `Path::extension(p)`, `Path::parent(p)`. Pfade verwenden `/` als Trenner; die Bibliothek wandelt auf Windows um.

**R-13.21 (Konsole).** `print(text)` schreibt `text` mit Zeilenumbruch auf die Standardausgabe (V-13.4). `Console::write(text)` ohne Zeilenumbruch, `Console::error(text)` auf die Fehlerausgabe, `Console::readLine()` liefert `string?` (`null` am Ende der Eingabe).

**R-13.22 (Umgebung).** `Env::get(string name)` liefert `string?`. `Env::os()` liefert `Os::Windows`, `Os::Linux` oder `Os::MacOS`. Es gibt kein `exit` (V-13.3).

## 8. Was ein selbstübersetzender Compiler damit abdeckt

| Aufgabe im Compiler | Bibliothek |
|---|---|
| Quelldateien finden und lesen | `Dir::list`, `File::open`, `readAllText` |
| Lexer | `string`-Zugriffe, `char.isLetter()` usw., `i64::parse`, `f64::parse` |
| Symbol- und Typtabellen | `Map`, `Set`, `Hashable` |
| Fehlermeldungen | `StringBuilder`, `toString()`, `Console::error` |
| Ausgabe der Zwischenrepräsentation | `File::writeBytes`, `Fmt` |
| `rpp.yaml` lesen | ein kleiner eigener Parser für die benötigte YAML-Teilmenge im Compiler, nicht in der Standardbibliothek |
