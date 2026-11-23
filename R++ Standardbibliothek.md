# R++ Standardbibliothek (Kapitel 13)

Stand: 23.11.2026, Entwurf 0.8. Dieses Dokument füllt Kapitel 13 des „R++ Spezifikationsgerüsts“ mit einem **ersten Schnitt**: genau dem, was ein Compiler für R++ braucht, der in R++ geschrieben ist und sich selbst übersetzt (Abschnitte 2 bis 7). Threads, Netz, Zeit, reguläre Ausdrücke und Datenformate wie JSON gehören nicht dazu. Abschnitt 9 beschreibt den Ausbau darüber hinaus; die Abschnitte 10 bis 15 legen die ersten Ausbau-Pakete `rpp.random`, die Vektor-Algorithmen, `rpp.math`, die Text-Hilfen, die Datei-Operationen sowie `Deque` und `getOrPut` fest.

Normativ sind in diesem Entwurf nur die Grundsätze (Abschnitt 1), die Teile, auf die sich andere Kapitel stützen (`Exception`, `Equatable`, `Comparable`, `Hashable`, `Iterable`), und die Ausbau-Pakete `rpp.random` (Abschnitt 10), `Vectors` (Abschnitt 11), `rpp.math` (Abschnitt 12), `Text` (Abschnitt 13), der `File`-Ausbau (Abschnitt 14) sowie `Deque` und `getOrPut` (Abschnitt 15). Die übrigen Schnittstellen sind ein Arbeitsstand: Die Bibliotheken unter `rpp` bleiben bis zum Abschluss der Selbstübersetzung auf Version 0.x und dürfen sich ändern.

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
| V-13.8 | `File::exists(path)` sagt, ob unter `path` eine gewöhnliche Datei liegt; `Path::absolute(p)` liefert den absoluten Pfad mit `/` als Trenner, `.` und `..` rein textuell aufgelöst (Verweise im Dateisystem werden nicht verfolgt). Beides braucht ein Compiler, der Projekte lädt. |
| V-13.9 | (angenommen am 23.11.2026) Ausbau-Grundsätze: Neue Pakete unter `rpp` ergänzen die Bibliothek, ohne den ersten Schnitt (Abschnitte 2 bis 7) zu ändern; auch sie bleiben bis zur Selbstübersetzung auf 0.x (R-13.2). Beim Portieren aus anderen Sprachen gelten die Übersetzungsregeln in Abschnitt 9: kein `Optional`/`Result` (das leisten `T?`, `try…` und Exceptions), Leihen vor Verbrauchen, Determinismus, und gleiches Verhalten in beiden Modi – die Bibliothek verlässt sich nicht auf das Überlaufverhalten des Releasemodus (R-4.14, R-12.5). Neue Pakete werden, wo möglich, in R++ selbst geschrieben und dienen damit als Lasttest für den Compiler in R++ (M6). |
| V-13.10 | (angenommen am 23.11.2026) Vektor-Algorithmen, die in R++ schreibbar sind, kommen als statische generische Methoden einer Hilfsklasse (`Vectors`, Abschnitt 11), nicht als eingebaute Methoden auf `T[]`. Eingebaute Methoden müssten im Compiler in R++ (M6) nachgezogen werden und erzeugen bis dahin eine Divergenz; die Hilfsklasse behandeln beide Compiler gleich. Eingebaute Methoden auf `T[]` bleiben, was R++ nicht effizient ausdrücken kann (wie `sort`). Veränderbare Algorithmen nutzen `remove`/`insert` (V-3; `swap` auf zwei Indizes gilt immer als überlappend, Kapitel 5, Abschnitt 7) und sind dadurch quadratisch; das genügt für die Referenz. |
| V-13.11 | (angenommen am 23.11.2026) `rpp.math` (Abschnitt 12) liefert `f64`-Funktionen. Runden (`floor`, `ceil`, `trunc`, `round`) und `clamp` sind reiner R++-Quelltext; `sqrt` ist eingebaut, weil IEEE 754 korrekt gerundetes Wurzelziehen verlangt und das in R++ nicht effizient ausdrückbar ist (Muster `fail("eingebaut")` wie bei `Fmt`, ohne M6-Divergenz). Transzendente Funktionen (`pow`, `exp`, `log`, `sin`, `cos`) folgen später; sie sind plattformabhängig und nicht bitgleich (Abschnitt 9). |
| V-13.12 | Eine `IOException` trägt den betroffenen Pfad in `path`; ihr Text (`message`) nennt den Pfad und beschreibt den Fehler. Der genaue Wortlaut der Beschreibung, insbesondere der Teil, der vom Betriebssystem kommt, ist nicht festgelegt und kann sich zwischen Implementierungen und Systemen unterscheiden. Programme und Tests werten ihn nicht aus. (angenommen am 23.11.2026) |

## 1. Grundsätze

**R-13.1 (Aufbau).** Die Standardbibliothek besteht aus den Bibliotheken in Abschnitt 2 bis 7 und den Ausbau-Bibliotheken in Abschnitt 10 bis 15. Sie stehen unter dem reservierten Besitzer `rpp` (Designentscheidungen, Abschnitt 13b); niemand sonst kann unter `rpp` veröffentlichen.

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
    ref mut T atMut(C cursor) mut;
}
```

Das Mixin implementiert die folgenden Methoden in Cursor-Reihenfolge. Der Callback leiht das jeweilige Element lesend.

| Methode | Verhalten | Leere Sammlung |
|---|---|---|
| `any(Func<T, bool> predicate)` | Beendet die Suche beim ersten Treffer. | `false` |
| `all(Func<T, bool> predicate)` | Beendet die Prüfung beim ersten Gegenbeispiel. | `true` |
| `fold<A>(own A acc, Func<own A, T, A> step)` | Übergibt den Akkumulator und das Element an `step`. Der Rückgabewert wird zum nächsten Akkumulator. Nach dem letzten Element gibt `fold` den Akkumulator zurück. | Startwert |
| `U[] mapNotNull<U>(Func<T, U?> transform)` | Ruft `transform` einmal je Element auf und sammelt alle Ergebnisse außer `null` in einem neuen Vektor. Reihenfolge und Duplikate bleiben erhalten. | Leerer Vektor |

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

`for (mut T x : mut c) S` verlangt `MutIterable<T, C>`, leiht `c` veränderbar und bindet `ref mut T x = mut k.atMut(cur);`. `break`, `continue` und `return` in `S` funktionieren wie in jeder Schleife. Während der Schleife kann `c` nicht verändert werden (Kapitel 5, Satz 5.2).

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

`new StringView(text)` erzeugt eine unveränderliche Sicht auf einen String. Die Sicht hält den gemeinsamen Quelltext am Leben, auch wenn die ursprüngliche Variable neu belegt wird oder ihren Gültigkeitsbereich verlässt. `length()`, `charAt(i)`, `indexOf(part)` und `slice(start, end)` zählen wie `string` Unicode-Skalarwerte. Das Ende eines Bereichs ist exklusiv. Ungültige Bereiche und Indizes führen zu einem `IndexError`.

`slice`, `trim()`, `trimEnd()` und `split(sep)` liefern Sichten auf denselben Speicher. Beim Aufteilen bleiben leere Teile erhalten. `startsWith` und `endsWith` prüfen direkt den Ausschnitt. `toString()` liefert seinen Inhalt als eigenständigen String.

`splitOnce(sep)` liefert ein nullbares Paar `(StringView, StringView)?`. Die Sichten umfassen den Text vor und nach dem ersten Treffer. Weitere Trennzeichen bleiben im rechten Teil. Bei fehlendem Treffer ist das Ergebnis `null`; ein Treffer am Ende liefert eine leere rechte Sicht. Der leere Trenner trifft am Anfang.

**R-13.14 (StringBuilder).** Baut einen String schrittweise auf:

```
export class StringBuilder {
public:
    constructor();
    nodiscard i64 length();                       // Anzahl Zeichen bisher
    void append(string s) mut;
    void appendChar(char c) mut;
    void appendLine(string s) mut;                // mit Zeilenumbruch
    void clear() mut;
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
| `push(own T x) mut`, `insert(i64 i, own T x) mut` | anfügen, einfügen |
| `pop() mut`, `remove(i64 i) mut` | `own T`, entfernt das letzte bzw. das `i`-te Element |
| `clear() mut` | alle Elemente freigeben |
| `ref T? first()`, `ref T? last()` | Borrow auf das erste bzw. letzte Element, `null` bei leerem Vektor |
| `contains(T x)`, `indexOf(T x)` | verlangen `T : Equatable<T>`; `indexOf` liefert `i64?` |
| `sort() mut` | aufsteigend, stabil; verlangt `T : Comparable<T>` |
| `sortBy(Func<T, T, i32> cmp) mut` | stabil nach `cmp` |

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
    ref mut V at(K key) mut;                  // Programmfehler, wenn key fehlt
    void put(own K key, own V value) mut;     // ersetzt einen vorhandenen Wert (der alte wird freigegeben)
    V? remove(K key) mut;                     // entfernt und liefert den Wert
    void clear() mut;
}

export class Entry<K, V> {
public:
    K key;
    V value;
}
```

Einträge werden in Einfügereihenfolge durchlaufen (V-13.2); `put` mit einem vorhandenen Schlüssel behält die Position.

**R-13.18 (Set).** `Set<T : Hashable & Equatable<T>>` mit `add(own T x) mut`, `contains(T x)`, `remove(T x) mut`, `clear() mut`, `length()`; iterierbar in Einfügereihenfolge.

`Set<T>` implementiert `Equatable<Set<T>>`. Zwei Mengen sind genau dann gleich, wenn sie dieselben Elemente enthalten, unabhängig von der Einfügereihenfolge.

| Operation | Elemente und Iterationsreihenfolge der neuen Menge |
|---|---|
| `a.intersect(b)` | Alle Elemente aus `a`, die auch in `b` enthalten sind, in der Reihenfolge von `a`. |
| `a.unite(b)` | Alle Elemente aus `a`, gefolgt von den zusätzlichen Elementen aus `b` in deren Reihenfolge. |
| `a.subtract(b)` | Alle Elemente aus `a`, die in `b` fehlen, in der Reihenfolge von `a`. |

Beide Operanden bleiben erhalten. Die Mitgliedschaft lässt sich in jeder Menge unabhängig ändern. Übernommene Objekte bleiben bis zum Entfernen ihres letzten Eintrags am Leben. Bei gleichen Elementen aus beiden Operanden behält das Ergebnis den Eintrag aus `a`.

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
    static bool exists(string path);           // gewöhnliche Datei vorhanden? (V-13.8)
    destructor();                              // schließt die Datei
}
```

`SaveFile::open(string path)` eröffnet einen Schreibvorgang in einer eigenen temporären Datei im Zielverzeichnis. `writeText(string text)` hängt UTF-8-Text an, `writeBytes(Slice<byte> data)` hängt Bytes an. Erst `commit()` macht den vollständigen Inhalt durch atomares Umbenennen am Zielpfad sichtbar. Auch eine zuvor fehlende Zieldatei entsteht erst dabei.

`cancel()` und der Destruktor verwerfen einen offenen Schreibvorgang. Das gilt auch beim Verlassen eines Blocks durch eine Exception. Ein Schreib- oder Commitfehler beendet den Vorgang mit einer `IOException`. Weitere Schreib- oder Commitversuche auf einem beendeten Vorgang werfen ebenfalls eine `IOException`. `cancel()` kann mehrfach aufgerufen werden.

**R-13.20 (Verzeichnisse und Pfade).** `Dir::list(string path)` liefert die Namen der Einträge sortiert (V-13.2); `Dir::exists`, `Dir::create`. `Path::join(a, b)`, `Path::fileName(p)`, `Path::extension(p)`, `Path::parent(p)`, `Path::absolute(p)` (V-13.8). Pfade verwenden `/` als Trenner; die Bibliothek wandelt auf Windows um.

`Dir::entries(string path)` liefert die unmittelbaren Einträge als `DirEntry[]` in derselben Namensreihenfolge wie `Dir::list`. Jeder Eintrag enthält `name`, den Pfad `path` mit dem übergebenen Verzeichnis als Basis und den Dateityp `kind` (`FileType::File`, `Directory`, `Symlink` oder `Other`). Symbolische Links haben unabhängig vom Ziel den Typ `Symlink`. Ein leeres Verzeichnis ergibt einen leeren Vektor. Ein Lesefehler löst eine `IOException` aus.

`new Path(text)` erzeugt einen unveränderlichen Pfad. `toString()` liefert seine normalisierte Schreibweise mit `/`. Die Normalisierung akzeptiert auch Rückstriche, entfernt überzählige Trenner und `.` und löst `..` rein textuell auf. Führende `..` bleiben bei relativen Pfaden erhalten. Ein leerer Pfad und `new Path()` stehen für `.`. Wurzeln folgen den Regeln der Plattform, unter Windows einschließlich Laufwerken und UNC-Freigaben.

`Path? relativeTo(Path baseDir)` liefert für einen Pfad unterhalb von `baseDir` die verbleibenden Pfadkomponenten und für denselben Pfad `.`. Liegt der Pfad außerhalb der Basis oder sind die Wurzeln verschieden, ist das Ergebnis `null`. Der Vergleich braucht keinen Dateizugriff. So ergibt `new Path("app/src/main.rpp").relativeTo(new Path("app"))` den Pfad `src/main.rpp`.

**R-13.21 (Konsole).** `print(text)` schreibt `text` mit Zeilenumbruch auf die Standardausgabe (V-13.4). `Console::write(text)` ohne Zeilenumbruch, `Console::error(text)` auf die Fehlerausgabe, `Console::readLine()` liefert `string?` (`null` am Ende der Eingabe).

**R-13.22 (Umgebung).** `Env::get(string name)` liefert `string?`. `Env::os()` liefert `Os::Windows`, `Os::Linux` oder `Os::MacOS`. Es gibt kein `exit` (V-13.3).

**Kommandozeilen in rpp.env.** `new CommandLineParser(befehl, beschreibung)` deklariert einen Befehl. `addPositional(name)` ergänzt ein Pflichtargument, `addFlag("--name", hilfetext)` einen Schalter ohne Wert. `helpText()` erzeugt die Hilfe aus diesen Angaben.

`parse(Slice<string> args)` erwartet die Argumente nach dem Befehlsnamen und liefert ein neues `CommandLineResult`. Bei Erfolg enthält `positional` die Werte in Aufrufreihenfolge, `isSet("--name")` prüft einen Schalter. `-h` und `--help` setzen `helpRequested`. Nach `--` gelten alle weiteren Wörter als Positionsargumente. Bei einem Argumentfehler enthält `error` die Meldung. Ausgabe und Exit-Code bestimmt das aufrufende Programm.

`new ElapsedTimer()` aus `rpp.env` startet eine monotone Stoppuhr. `elapsed()` liefert die verstrichene Zeit in ganzen Millisekunden als `i64`. `start()` setzt den Startpunkt neu. `restart()` liefert die Dauer des abgeschlossenen Intervalls und verwendet denselben Messpunkt als Beginn des nächsten. Änderungen an Datum, Uhrzeit oder Zeitzone beeinflussen die Messung nicht. Intervalle unter einer Millisekunde ergeben `0`.

## 8. Was ein selbstübersetzender Compiler damit abdeckt

| Aufgabe im Compiler | Bibliothek |
|---|---|
| Quelldateien finden und lesen | `Dir::list`, `File::open`, `readAllText` |
| Lexer | `string`-Zugriffe, `char.isLetter()` usw., `i64::parse`, `f64::parse` |
| Symbol- und Typtabellen | `Map`, `Set`, `Hashable` |
| Fehlermeldungen | `StringBuilder`, `toString()`, `Console::error` |
| Ausgabe der Zwischenrepräsentation | `File::writeBytes`, `Fmt` |
| `rpp.yaml` lesen | ein kleiner eigener Parser für die benötigte YAML-Teilmenge im Compiler, nicht in der Standardbibliothek |

## 9. Ausbau über den ersten Schnitt hinaus (Arbeitsstand)

Der erste Schnitt bleibt unverändert, bis der Compiler sich selbst übersetzt. Darüber hinaus wächst die Bibliothek in Paketen unter `rpp` (V-13.9). Dieser Abschnitt ist ein Arbeitsstand, keine normative Festlegung; nur Abschnitt 10 ist normativ.

**Übersetzungsregeln beim Portieren aus anderen Sprachen.** Nicht die API wird kopiert, sondern das Konzept an R++ angepasst:

| Konzept der Vorbilder | R++-Entsprechung |
|---|---|
| `Option`/`Maybe`/`Optional` (Rust, Swift, Java) | nicht portieren: `T?` mit Flussanalyse (Kapitel 6) |
| `Result<T, E>` (Rust) | nicht portieren: Exceptions für erwartbare Fehler, `try…`-Varianten liefern `null` (R-13.4) |
| `into_iter` vs. `iter` (Rust) | Methoden leihen (`ref`); verbrauchende Varianten nehmen `own` – die Ownership-Prüfung erzwingt die Korrektheit |
| Lazy Iteratoren / LINQ / Kotlin Sequences | Adapter über dem Cursor-Protokoll `Iterable<T, C>`; sie halten einen Borrow auf die Quelle, Lebensdauer = Block (R-5.17) |
| RAII / `use` / `defer` (C++, Kotlin, Go) | Destruktoren; Muster ist `SaveFile` (R-13.19) |
| GC-abhängige APIs (Finalizer-Queues, zyklische `WeakRef`s) | nicht portieren: kein Garbage Collector |
| Determinismus | durchziehen (V-13.2): Einfügereihenfolge, stabile Sortierung, seedbarer Zufall |

**Roadmap.** Phase A läuft neben M6 und ist in R++ schreibbar; Phase B folgt nach der Selbstübersetzung; Phase C erst mit der Thread-Bibliothek (Version 1 ist einfach-threaded, Designentscheidungen Abschnitt 12).

| Phase | Paket | Inhalt | Vorbilder |
|---|---|---|---|
| A | `rpp.collections` (Ausbau) | `Vectors` (Abschnitt 11), `Deque` und `Map.getOrPut` (Abschnitt 15), **festgelegt**; offen: Sichten (`chunked`, `windowed`, `zip`) | Rust `Vec`, Go `slices`, C++ `<algorithm>`, Kotlin, Python |
| A | `rpp.math` | `sqrt`, `floor`/`ceil`/`trunc`/`round`, `clamp`, `PI`/`E` (Abschnitt 12, **festgelegt**); offen: `pow`, `exp`, `log`, `sin`, `cos` (plattformabhängig) | Rust `f64`, Java `Math` |
| A | `rpp.random` | seedbarer Zufall, `shuffle` (Abschnitt 10, **festgelegt**) | Rust `rand`, Go `math/rand` |
| A | `rpp.text` (Ausbau) | `Text`: `lines`, `removePrefix`/`removeSuffix`, `repeat`, `splitLimit` (Abschnitt 13, **festgelegt**) | Kotlin, Python |
| A | `rpp.io` (Ausbau) | `File`: `appendText`, `copyFile`, `moveFile`, `delete`, `size` (Abschnitt 14, **festgelegt**); offen: gepuffertes Lesen, Mtime (braucht `rpp.time`) | Java `BufferedReader`, Go `os` |
| B | `rpp.iter` | lazy `map`/`filter`/`take`/`flatMap`/`chain` über dem Cursor-Protokoll | Rust Iterator, Kotlin Sequence, C++20 ranges |
| B | `rpp.regex` | lineare Zeit, kein katastrophales Backtracking | RE2 (Go) |
| B | `rpp.json` | Pull-Parser und Baum; Parsefehler mit Zeile/Spalte | Go `encoding/json`, Rust `serde_json` |
| B | `rpp.time` | `Duration`, monotone `Instant` (Ausbau von `ElapsedTimer`) | Rust `std::time`, Java `java.time` |
| B | `rpp.encoding` | Base64, Hex, UTF-16 | Go, Rust |
| B | `rpp.hash` | SHA-256 (wird für die Prüfsummen in `rpp.lock` gebraucht, Designentscheidungen 13b) | Rust `sha2` |
| C | `rpp.sync` | Channels; Werte wandern per `own` durch den Channel | Go, Rust |
| C | `rpp.process`, `rpp.net` | Subprozesse; HTTP-Client (für das zentrale Verzeichnis) | Go `os/exec`, Rust `reqwest` |

Bewusst nicht portiert werden `Optional`/`Result`-Typen, Reflection, Expression Trees und `exit` (V-13.3). Transzendente Funktionen in `rpp.math` sind plattformübergreifend nicht bitgleich; das Determinismusversprechen (V-13.2) gilt für sie nur innerhalb einer Plattform.

## 10. rpp.random

Seedbarer Pseudozufall, deterministisch und in beiden Modi gleich (V-13.9). Eingebunden mit `using rpp.random.Random;`. Der Algorithmus ist festgelegt, damit jede Implementierung dieselbe Folge liefert; damit lässt sich auch das FR++-Fuzzing reproduzierbar nach R++ portieren.

**R-13.23 (Random).**

```
export class Random {
public:
    constructor(ui64 seed);
    nodiscard ui64 nextUi64() mut;
    nodiscard i64 nextInt(i64 bound) mut;
    nodiscard f64 nextFloat() mut;
    nodiscard bool nextBool() mut;
    void shuffle<T>(mut T[] v) mut;
}
```

Der Zustand sind vier `ui64`-Wörter `s0` bis `s3`. Alle Arithmetik in diesem Paket rechnet modulo 2⁶⁴; die Bibliothek vermeidet Überlauf (R-4.14), damit Debug- und Releasemodus dasselbe Verhalten zeigen (R-12.5). Schieben ist logisch; herausgeschobene Bits entfallen (R-4.16). `rotl(x, k)` ist `(x << k) | (x >> (64 - k))`.

**Seeding (SplitMix64).** Aus `seed` entstehen die vier Zustandswörter: `z = seed`; dann je Wort:

```
z = (z + 0x9E3779B97F4A7C15) mod 2⁶⁴
z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) mod 2⁶⁴
z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) mod 2⁶⁴
sᵢ = z ^ (z >> 31)
```

`seed = 0` ist erlaubt. Der Zustand ist nie ganz null: Die Konstante ist ungerade, und ein Wort kann nur für einen einzigen `z`-Wert null werden.

**Schritt (`xoshiro256**`).** `nextUi64()` liefert

```
result = (rotl((s1 * 5) mod 2⁶⁴, 7) * 9) mod 2⁶⁴
t = (s1 << 17) mod 2⁶⁴
s2 ^= s0;  s3 ^= s1;  s1 ^= s2;  s0 ^= s3;  s2 ^= t
s3 = rotl(s3, 45)
```

und gibt `result` zurück. Gleicher Seed heißt gleiche Folge, auf jeder Plattform und in beiden Modi.

**R-13.24 (Zahlen ziehen).**

| Methode | Verhalten |
|---|---|
| `nextUi64()` | der nächste 64-Bit-Wert nach R-13.23 |
| `nextInt(bound)` | gleichverteilt in `0` bis `bound − 1`, ohne Modulo-Bias. `bound ≤ 0` ist ein Programmfehler (`FailError`). Mit `b = cast<ui64>(bound)` und `threshold = ((ui64::MAX % b) + 1) % b` wird `x = nextUi64()` gezogen, bis `x ≥ threshold`; das Ergebnis ist `cast<i64>(x % b)`. Erwartet höchstens zwei Züge. |
| `nextFloat()` | `cast<f64>(nextUi64() >> 11) / 2⁵³`, gleichverteilt in `[0, 1)` mit 53 Bit Auflösung |
| `nextBool()` | `(nextUi64() >> 63) == 1`, das oberste Bit der gezogenen Zahl |

**R-13.25 (Mischen).** `shuffle<T>(mut T[] v)` mischt einen Vektor mit Fisher-Yates von hinten nach vorn: für `i` von `v.length − 1` bis `1` wird `j = nextInt(i + 1)` gezogen und die Plätze `i` und `j` werden getauscht. Ein leerer Vektor bleibt unverändert. Das Ergebnis ist eine Permutation; bei gleichem Seed dieselbe Reihenfolge.

Der Tausch geschieht über Herausnehmen und Einsetzen (`remove`/`insert`, R-13.16), nicht über einen Tausch zweier Indizes: Aus Elementen wird nicht herausbewegt (V-3), und `swap(mut v[i], mut v[j])` gilt immer als überlappend (Kapitel 5, Abschnitt 7). Die Elemente werden verschoben; `shuffle` funktioniert damit für jeden Elementtyp, auch für `own`-Objekte. Feste Arrays lassen sich in Version 1 nicht mischen, weil ein Slice die Länge nicht ändern darf (Satz 5.2).

## 11. rpp.collections: Vektor-Algorithmen (Ausbau)

Statische generische Methoden der Klasse `Vectors`, eingebunden mit `using rpp.collections.Vectors;`. Reiner R++-Quelltext (V-13.9, V-13.10); die Methoden ergänzen die eingebauten Vektor-Methoden (R-13.16). Sie stehen als Hilfsklasse statt als eingebaute Methoden auf `T[]`, damit beide Compiler sie gleich behandeln (V-13.10).

**R-13.26 (Vectors, Suchen und Vergleichen).**

```
export class Vectors {
public:
    static i64? binarySearch<T : Comparable<T>>(Slice<T> s, T x);
    static bool equals<T : Equatable<T>>(Slice<T> a, Slice<T> b);
    static void reverse<T>(mut T[] v);
    static void retain<T>(mut T[] v, Func<T, bool> keep);
    static void dedup<T : Equatable<T>>(mut T[] v);
}
```

| Methode | Verhalten |
|---|---|
| `binarySearch(s, x)` | binäre Suche im **aufsteigend sortierten** Slice `s` (Comparable); liefert die Position eines Treffers oder `null`. Leerer Slice: `null`. Bei mehreren gleichen Treffern wird eine Position geliefert. |
| `equals(a, b)` | elementweiser Vergleich zweier Slices (Equatable); `true` bei gleicher Länge und gleichen Elementen in gleicher Reihenfolge. Füllt die Lücke, dass Arrays kein `==` haben (V-4.8). |

**R-13.27 (Veränderbare Algorithmen).** `reverse`, `retain` und `dedup` verändern einen Vektor an Ort und Stelle:

| Methode | Verhalten |
|---|---|
| `reverse(v)` | kehrt die Reihenfolge um; ein leerer Vektor und einer mit einem Element bleiben unverändert |
| `retain(v, keep)` | behält nur die Elemente, für die `keep` wahr ist; die anderen werden freigegeben. Die Reihenfolge der behaltenen bleibt erhalten. |
| `dedup(v)` | entfernt **aufeinanderfolgende** Duplikate (Equatable); nicht aufeinanderfolgende gleiche Elemente bleiben erhalten |

Alle drei arbeiten über `remove`/`insert` (R-13.16): Aus Elementen wird nicht herausbewegt (V-3), und `swap` auf zwei Indizes gilt immer als überlappend (Kapitel 5, Abschnitt 7). Die Elemente werden verschoben; die Methoden funktionieren für jeden Elementtyp, auch für `own`-Objekte. Da jede Verschiebung linear ist, brauchen sie Zeit proportional zum Quadrat der Länge; für den Referenz-Interpreter genügt das. Ein Slice darf seine Länge nicht ändern (Satz 5.2), deshalb nehmen die Methoden `mut T[]` und nicht `mut Slice<T>`; feste Arrays lassen sich so nicht verändern.

Die Constraints werden am Aufruf geprüft (R-7.26): `binarySearch` ohne `Comparable`, `equals` und `dedup` ohne `Equatable` sind Fehler.

## 12. rpp.math

Gleitkomma-Hilfen für `f64`, eingebunden mit `using rpp.math.Math;`. Die Rundungs- und Begrenzungsfunktionen sind reiner R++-Quelltext, `sqrt` ist eingebaut (V-13.11). Alle Funktionen verhalten sich in beiden Modi gleich (V-13.9). `abs`, `min` und `max` gibt es bereits als Methoden der Zahltypen (R-13.11) und werden hier nicht wiederholt.

**R-13.28 (Math, Konstanten und Runden).**

```
export class Math {
public:
    static const f64 PI;
    static const f64 E;
    static f64 sqrt(f64 x);
    static f64 floor(f64 x);
    static f64 ceil(f64 x);
    static f64 trunc(f64 x);
    static f64 round(f64 x);
    static f64 clamp(f64 x, f64 lo, f64 hi);
}
```

`PI` und `E` sind die `f64`-Werte, die π und e am nächsten kommen.

| Funktion | Verhalten |
|---|---|
| `sqrt(x)` | korrekt gerundete Quadratwurzel (IEEE 754). `x < 0`: NaN. `sqrt(±0) = ±0`, `sqrt(+∞) = +∞`, `sqrt(NaN) = NaN`. |
| `floor(x)` | größte ganze Zahl ≤ x |
| `ceil(x)` | kleinste ganze Zahl ≥ x |
| `trunc(x)` | ganzzahliger Anteil Richtung null |
| `round(x)` | nächste ganze Zahl; bei genau .5 von null weg (2.5 → 3, −2.5 → −3) |

Die Rundungsfunktionen liefern exakte ganzzahlige Werte. Für alle gilt: NaN bleibt NaN, ±∞ bleibt ±∞, und ±0 bleibt ±0 (das Vorzeichen von null bleibt erhalten).

**R-13.29 (clamp).** `clamp(x, lo, hi)` begrenzt x auf das Intervall `[lo, hi]`: kleiner als lo → lo, größer als hi → hi, sonst x. `lo <= hi` wird verlangt; `lo > hi` ist ein Programmfehler (`FailError`). Ist x NaN, bleibt das Ergebnis NaN.

Transzendente Funktionen (`pow`, `exp`, `log`, `sin`, `cos`, `tan`) sind noch nicht festgelegt; sie folgen mit einer späteren Version (Abschnitt 9). Ihr Ergebnis ist plattformabhängig und nicht bitgleich.

## 13. rpp.text: Text-Hilfen (Ausbau)

Statische Methoden der Klasse `Text` auf `string`, eingebunden mit `using rpp.text.Text;`. Reiner R++-Quelltext (V-13.9, V-13.10) nach dem Muster von `Fmt` (R-13.15); sie ergänzen die eingebauten String-Methoden (R-13.13), ohne den ersten Schnitt zu ändern.

**R-13.30 (Text).**

```
export class Text {
public:
    static string[] lines(string s);
    static string removePrefix(string s, string prefix);
    static string removeSuffix(string s, string suffix);
    static string repeat(string s, i64 n);
    static string[] splitLimit(string s, string sep, i64 limit);
}
```

| Methode | Verhalten |
|---|---|
| `lines(s)` | zerlegt s in Zeilen. Trenner ist `\n`; ein `\r` unmittelbar davor entfällt (CRLF). Leere Zeilen bleiben erhalten; ein abschließendes `\n` erzeugt keine zusätzliche leere Zeile. Für `""` ist das Ergebnis ein leerer Vektor. |
| `removePrefix(s, prefix)` | entfernt prefix vom Anfang, falls s damit beginnt; sonst bleibt s unverändert. Ein leerer prefix lässt s unverändert. |
| `removeSuffix(s, suffix)` | entfernt suffix vom Ende, falls s damit endet; sonst bleibt s unverändert. Ein leerer suffix lässt s unverändert. |
| `repeat(s, n)` | hängt s n-mal aneinander. `n >= 0` wird verlangt; `n < 0` ist ein Programmfehler (`FailError`). `n = 0` oder ein leerer s ergibt `""`. |
| `splitLimit(s, sep, limit)` | teilt s an sep in höchstens limit Teile; der letzte Teil enthält den ungeteilten Rest. `limit >= 1` und ein nicht leerer sep werden verlangt, sonst Programmfehler (`FailError`). `limit = 1` ergibt `[s]`; ist die Teilzahl kleiner als limit, werden alle Teile geliefert. Leere Teile bleiben erhalten. |

`lines` und `splitLimit` liefern neue Vektoren; die übrigen Methoden geben unveränderliche Strings zurück (Strings sind implizit geteilt, R-4.18).

## 14. rpp.io: Datei-Operationen (Ausbau)

Die Klasse `File` (R-13.19) erhält weitere statische Methoden für Datei-Operationen auf Pfaden. Sie sind eingebaut (`fail("eingebaut")`, Dateisystemzugriff wie bei `File`/`Dir`); als R++-Klasse behandeln beide Compiler die Signaturen gleich, es entsteht keine M6-Divergenz (V-13.10). Fehler werfen `IOException` (R-13.4).

**R-13.31 (Datei-Operationen).** Zusätzlich zu R-13.19:

```
static void appendText(string path, string text);
static i64 copyFile(string from, string to);
static void moveFile(string from, string to);
static void delete(string path);
static i64 size(string path);
```

| Methode | Verhalten |
|---|---|
| `appendText(path, text)` | hängt text als UTF-8 an die Datei an; eine fehlende Datei wird angelegt. `IOException` bei Schreibfehler. |
| `copyFile(from, to)` | kopiert die Datei from nach to (ein vorhandenes to wird ersetzt) und liefert die Zahl der kopierten Bytes. `IOException`, wenn from fehlt. |
| `moveFile(from, to)` | benennt from nach to um (ein vorhandenes to wird ersetzt). `IOException`, wenn from fehlt. |
| `delete(path)` | entfernt die Datei. `IOException`, wenn sie fehlt. |
| `size(path)` | Größe der Datei in Bytes. `IOException`, wenn sie fehlt. |

`copy` ist ein Schlüsselwort (Kapitel 7), deshalb heißen die Methoden `copyFile` und `moveFile`. Pfade verwenden `/` als Trenner; die Bibliothek wandelt auf Windows um (R-13.20).

Noch offen (später): gepuffertes Lesen und die Änderungszeit (Mtime); letztere braucht eine Zeitdarstellung aus `rpp.time` (Abschnitt 9, Phase B).

## 15. rpp.collections: Deque und getOrPut (Ausbau)

**R-13.32 (Deque).** `Deque<T>` ist eine doppelseitige Warteschlange, eingebunden mit `using rpp.collections.Deque;`. Reiner R++-Quelltext (V-13.9), vektorbasiert und iterierbar.

```
export class Deque<T> {
    import Iterable<T, i64>;
public:
    constructor();
    nodiscard i64 length();
    nodiscard bool isEmpty();
    void pushBack(own T x) mut;
    void pushFront(own T x) mut;
    own T popBack() mut;
    own T popFront() mut;
    ref T? front();
    ref T? back();
    void clear() mut;
}
```

| Methode | Verhalten |
|---|---|
| `pushBack(x)` / `pushFront(x)` | hängt x hinten bzw. vorne an (übernimmt `own T`) |
| `popBack()` / `popFront()` | entfernt und liefert das hintere bzw. vordere Element (`own T`). Auf einer leeren Deque ein Programmfehler (`IndexError`). |
| `front()` / `back()` | Borrow auf das vordere bzw. hintere Element, `null` bei leerer Deque |
| `length()` / `isEmpty()` / `clear()` | Anzahl, Leeretest, alle Elemente freigeben |

Die Iteration (`Iterable<T, i64>`) läuft von vorne nach hinten in Einfügereihenfolge. Die Elemente werden verschoben; `Deque` funktioniert für jeden Elementtyp, auch für `own`-Objekte. Die Referenz ist vektorbasiert: Operationen am Anfang (`pushFront`, `popFront`) brauchen lineare Zeit (V-13.10); ein Ringpuffer mit konstanter Zeit wäre eine spätere eingebaute Variante.

**R-13.33 (getOrPut).** `Map<K, V>` (R-13.17) erhält:

```
ref mut V getOrPut(own K key, Func<V> factory) mut;
```

Liefert einen Borrow auf den Wert zum Schlüssel. Fehlt der Schlüssel, wird `factory()` aufgerufen, der Wert eingesetzt und sein Borrow zurückgegeben; die Factory wird **nur** im fehlenden Fall aufgerufen. Der Rückgabewert ist ein veränderlicher Borrow (`ref mut V`). `getOrPut` ist eingebaut (`fail("eingebaut")`), damit es auch für `own`-Schlüssel funktioniert – eine reine R++-Fassung müsste den Schlüssel nach `put` erneut verwenden, was bei verschobenen `own`-Schlüsseln nicht geht. Als R++-Methode behandeln beide Compiler die Signatur gleich (V-13.10).






