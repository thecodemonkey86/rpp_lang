# R++ Designentscheidungen

Stand: 21.11.2026. Dieses Dokument hält fest, was für die Sprache R++ entschieden ist und was noch offen ist. Es ist die Grundlage für das Dokument „R++ Spezifikationsgerüst“.

Status-Kürzel: **[E]** ausdrücklich entschieden, **[A]** vorgeschlagen und ohne Einwand angenommen, **[O]** offen.

## 1. Ziele

- [E] Debugmodus: Code wird interpretiert oder per JIT in einer VM ausgeführt, für kurze Entwicklungszyklen.
- [E] Releasemodus: Kompilierung zu nativen Executables.
- [E] Bestimmte Fehlerklassen sind formal ausgeschlossen (siehe Abschnitt 4 und 5).
- [E] Objektorientiert mit Klassen; Syntax nah an C/C++/Java/C#, aber einfacher.
- [E] Keine Makros, aber bedingte Kompilierung. Keine Headerdateien.
- [A] Beide Modi teilen sich Frontend und Zwischenrepräsentation. Typ- und Ownership-Prüfung laufen auch im Debugmodus.
- [A] Der Debug-Interpreter dient als Referenzimplementierung, gegen die der native Compiler getestet wird.

## 2. Basistypen

- [E] `void`, `bool`, `char`, `byte`, `string`
- [E] Ganzzahlen: `i8`, `i16`, `i32`, `i64`, `ui8`, `ui16`, `ui32`, `ui64`
- [E] Gleitkomma: `f32`, `f64`
- [E] Primitive Werte werden immer kopiert.
- [E] Strings sind implizit shared (vom Ownership-Mechanismus ausgenommen) und intern UTF-8.
- [A] `char` ist ein 32-Bit-Unicode-Skalarwert.
- [A] Längen und Indizes haben den Typ `i64`; es gibt keinen eigenen Typ wie `usize`.
- [A] Integer-Überlauf: Exception im Debugmodus, definierter Wraparound im Releasemodus.
- [E] Konvertierung zwischen Zahltypen: automatisch nur, wenn jeder Wert des Quelltyps im Zieltyp exakt erhalten bleibt, z. B. `i8` → `i32`, `ui32` → `i64`, `i32` → `f64`, `f32` → `f64`. Nicht automatisch: alle Downcasts sowie `i32` ↔ `ui32`, `i64` → `f64`, `i32` → `f32`.
- [E] Explizite Casts: `cast<i32>(x)`.
- [E] Passt der Wert bei `cast` nicht in den Zieltyp: Exception im Debugmodus, Abschneiden der oberen Bits im Releasemodus (wie beim Integer-Überlauf).
- [E] Zahlliterale bekommen ihren Typ aus dem Kontext; der Compiler prüft, ob der Wert passt (`i8 b = 300;` ist ein Fehler). Ohne Kontext: Ganzzahlen `i32`, bei zu großem Wert `i64`; Gleitkommazahlen `f64`.
- [E] Ziffern dürfen mit `_` getrennt werden (`1_000_000`), aber nicht am Anfang oder Ende. Typ-Suffixe wie `42i64` gibt es nicht.
- [E] `bool` ist strikt: Bedingungen müssen `bool` sein; zwischen Zahlen und `bool` gibt es keine Umwandlung, auch nicht mit `cast`.
- [E] `byte` ist ein eigener Typ für Rohdaten: Vergleiche und Bitoperationen ja, Arithmetik nein. Umwandlung zu und von `ui8` mit `cast`.
- [E] Gemischte Arithmetik: Beide Operanden werden in ihren gemeinsamen Typ umgewandelt (der kleinste Typ, in den beide automatisch umgewandelt werden können; `i8 + i32` → `i32`). Gibt es keinen, ist der Ausdruck ein Fehler. Ganzzahldivision und `%` durch 0 werfen in beiden Modi; Division rundet zur Null; `%` hat das Vorzeichen des Dividenden; Gleitkomma nach IEEE 754.
- [E] `cast` von Gleitkomma zu Ganzzahl schneidet die Nachkommastellen ab. Außerhalb des Bereichs oder bei NaN: Exception im Debugmodus, Sättigung im Releasemodus (NaN wird 0).
- [E] `string`: `s.length` ist die Anzahl der Zeichen, `s[i]` liefert das i-te `char`. Zusätzlich `s.byteLength` und `s.byteAt(i)` für die UTF-8-Bytes.

## 3. Ownership und Borrowing

- [E] Drei Besitzformen: `own` (eindeutiger Besitz, Übergabe per Move), `shared` (Referenzzählung), `weak` (schwache Referenz, gegen Zyklen). Shared wird nie implizit abgeleitet.
- [E] Parameter ohne Markierung sind geliehen (Borrow) und nur lesbar. Objekte werden nie implizit kopiert; Kopieren ist explizit.
- [E] Borrows dürfen nicht in Feldern gespeichert werden. Es gibt deshalb keine Lifetime-Annotationen.
- [A] Folge daraus: Felder und lokale Variablen ohne Markierung besitzen ihr Objekt eindeutig. `own` steht nur an Parametern und Lambdas; `shared` und `weak` können überall stehen.
- [E] Veränderbar geliehen wird mit dem Schlüsselwort `mut` vor dem Parametertyp markiert: `void rename(mut Foo f)` (geändert am 22.11.2026, vorher `Foo& f`).
- [A] An der Aufrufstelle steht ebenfalls `mut`: `rename(mut foo)`.
- [A] Nur geliehene Parameter werden markiert. Lokale Variablen und Felder sind ohne Markierung veränderbar.
- [E] Lokale Variablen, die nur leihen, werden mit `ref` deklariert: `ref Foo f = list[0];`, veränderbar `ref mut Foo g = mut x;` (siehe Abschnitt 17).
- [A] Methoden, die `this` verändern, werden mit `mut` nach der Parameterliste markiert: `void setName(string n) mut { ... }`. Unmarkierte Methoden lesen nur.
- [A] Exklusivität: Solange ein `mut`-Borrow besteht, ist kein anderer Zugriff auf das Objekt erlaubt.

## 4. Null-Sicherheit

- [E] Typen sind standardmäßig nicht nullbar. Nullbare Typen tragen ein Fragezeichen: `Foo?`.
- [E] Der Compiler meldet einen Fehler, wenn ein nullbarer Wert ohne vorherige Prüfung dereferenziert wird. Null-Dereferenzierungen sind damit ausgeschlossen.
- [E] Prüfung per Typverengung: Nach `if (x != null)` gilt `x` im Block als nicht nullbar; nach `if (x == null) { return; }` gilt das für den Rest der Funktion.
- [E] Kurzformen: `x?.methode()` ruft nur auf, wenn `x` nicht null ist; `x ?? ersatz` liefert `ersatz`, wenn `x` null ist.
- [E] Es gibt keinen Operator, der die Prüfung überspringt (kein `x!`).
- [A] Die Verengung gilt nur für lokale Variablen und Parameter. Ein nullbares Feld muss vor dem Zugriff in eine lokale Variable geholt werden.
- [E] Auch primitive Typen und Tupel können nullbar sein (`i32?`, `(i32, string)?`); mit Besitzformen `shared Foo?` und `weak Foo?`. `weak Foo` ohne `?` ist ein Verweis, dessen Ziel freigegeben sein kann; `lock()` liefert immer `shared Foo?`.

## 5. Ausgeschlossene Fehlerklassen

- [A] Zur Compilezeit: Use-after-free, Double-free, Null-Dereferenzierung, ungültige Iteratoren.
- [A] Zur Laufzeit per Exception: Indexfehler bei Arrays.
- [E] Präzisierung (21.11.2026): Solche Laufzeitprüfungen der Sprache lösen keine fangbare Exception aus, sondern einen Programmfehler, der das Programm nach dem Aufräumen beendet (Abschnitt 22).
- [A] Data Races: in Version 1 dadurch ausgeschlossen, dass jedes Objekt zu genau einem Thread gehört (Abschnitt 12).

## 6. Funktionen und Rückgabewerte

- [E] Klassische Definition ohne `fn`: `<Rückgabetyp> name(<Typ> p1, <Typ> p2) { ... }`
- [E] Mehrere Rückgabewerte in Tupel-Schreibweise: `(i32, string) f()` und `(i32 a, string b) = f();`
- [E] Tupel werden in Version 1 nur zerlegt; einen Elementzugriff wie `t.0` gibt es nicht. Ein Tupel besitzt seine Elemente.
- [E] Keine frei definierbaren Funktionen außerhalb von Klassen; höchstens eingebaute Funktionen wie `print`. Einzige Ausnahme ist `main`.
- [E] Statische Methoden mit dem Keyword `static`.
- [E] Statische Member werden mit `::` angesprochen, Instanzmember mit `.`: `Math::max(a, b)`, `Shape::PI`, `List<Foo>::create()`. Dasselbe gilt für Enum-Varianten (`Color::Red`, `case Shape::Circle(r):`) und Array-Größen aus anderen Klassen (`i32[Limits::N]`). Grund ist die Lesbarkeit: Ein statischer Zugriff ist auf den ersten Blick erkennbar, und Paketpfad und Member-Zugriff sind klar getrennt (`my.pkg.Registry::count()`).
- [E] `main` wird gesondert behandelt: Sie darf auch außerhalb einer Klasse definiert werden und gibt den Exit-Code zurück. Die Projektdatei nennt die Sourcedatei, die `main` enthält.
- [E] Signatur von `main`: `i32 main(string[] args)`.
- [E] Statische Felder gibt es in Version 1 nur als Konstanten (`static const`). Veränderbare statische Felder wären globaler Zustand und vertragen sich nicht mit Exklusivität und der Thread-Regel.

## 7. Arrays, Vektoren und Slices

- [E] Drei getrennte Typen: feste Arrays, dynamische Arrays (Vektoren) und Slices.
- [E] Festes Array: `i32[16]` oder `i32[N]` mit einer `static const`-Konstante `N`. Die Größe steht zur Compilezeit fest; die Elemente liegen direkt in der Variable oder im Objekt. Kein `push` oder `pop`.
- [E] Vektor: `T[]` wächst dynamisch mit Methoden wie `push`.
- [E] Array-Literale `[1, 2, 3]` werden zu dem Typ, den der Kontext verlangt; bei festen Arrays muss die Länge passen.
- [E] Slice: Bibliothekstyp `Slice<T>`, eine geliehene Sicht auf einen Ausschnitt eines festen Arrays oder Vektors. Veränderbar als Parameter `mut Slice<T> s`.
- [E] Slices sind Borrows: erlaubt als Parameter und lokale Variablen, nie als Felder. Deshalb braucht es keine Lifetimes.
- [E] Solange ein Slice auf einen Vektor besteht, kann der Vektor nicht wachsen oder schrumpfen (Exklusivitätsregel).
- [E] Bereiche mit `..`, Ende exklusiv: `a[1..3]`, `a[..3]`, `a[1..]`, `a[..]`. Grenzen außerhalb des Arrays werfen eine Exception.
- [E] Feste Arrays und Vektoren werden automatisch als Slice geliehen: `sum(fixed)` und `sum(vec)` rufen dieselbe Methode `sum(Slice<i32> values)` auf.
- [E] Alle drei Typen haben `.length`, geprüften Indexzugriff und funktionieren in `for (x : a)`.
- [A] Einheitlicher Elementtyp; ein leeres `[]` bekommt seinen Typ aus dem Kontext.
- [A] Zugriff außerhalb der Grenzen wirft eine Exception; es gibt keine Lücken.
- [A] Ein Array besitzt seine Elemente. `arr[i]` liefert bei Objekten einen Borrow; Herausnehmen aus einem Vektor nur über `pop()` / `remove(i)` oder per Kopie.
- [A] `push` und andere verändernde Methoden brauchen `mut`-Zugriff.

## 8. Klassen und Vererbung

- [E] Klassen mit `constructor(...)`, Sichtbarkeit über Abschnitte `public:`, `private:`, `protected:`.
- [E] Explizite Kopie über `copy(MyClass other) { ... }`.
- [E] Aufruf der Kopie: `Foo b = copy(a);` liefert ein neues Objekt mit eindeutigem Besitz, auch wenn `a` `shared` oder nur geliehen ist. Hat die Klasse kein `copy(...)`, ist das ein Compilerfehler; so lassen sich Klassen gegen Kopieren sperren.
- [E] Vererbung in C#-Schreibweise: `class A : B`.
- [E] Objekterzeugung mit `new`: `Foo f = new Foo(...);`
- [A] Einfachvererbung plus Mixins; Überschreiben nur mit dem Keyword `override`.
- [E] Aufruf des Basiskonstruktors wie in C#: `constructor(i32 x) : base(x) { ... }`
- [E] Destruktor: `destructor() { ... }`
- [E] Klassen können virtuelle und abstrakte Methoden haben, die in Subklassen überschrieben bzw. implementiert werden.
- [E] Schreibweise wie in C#: Keywords `virtual` und `abstract`. Methoden ohne `virtual` sind nicht überschreibbar. Eine Klasse mit abstrakten Methoden wird als `abstract class` deklariert und kann nicht mit `new` erzeugt werden. `override` ist auch beim Implementieren einer abstrakten Methode Pflicht.
- [E] Properties wie in C# (`int number {get; set;}`): auf eine spätere Version zurückgestellt.
- [E] Downcasts als prüfende Abfrage mit dem neuen Schlüsselwort `as`: `ref Circle? c = s as Circle;` liefert `null`, wenn `s` kein `Circle` ist; danach greift die Null-Prüfung.
- [E] `==` auf Objekten ist nur erlaubt, wenn die Klasse das Mixin `Equatable` importiert; dann ruft `==` die Methode `equals` auf. Vergleich mit `null` ist immer erlaubt. Identität prüft die eingebaute Funktion `same(a, b)`.

## 9. Mixins

- [E] Ein Mixin bringt nur Methoden mit, keinen eigenen Zustand. `import MyMixin;` in der Klasse wirkt so, als wären die Methoden direkt dort definiert.
- [E] Ein Mixin deklariert seine Anforderungen an die Klasse als Methoden ohne Rumpf. Der Compiler prüft das Mixin an der Definition und beim Import nur noch die Anforderungen.
- [E] Ein Mixin ist zugleich ein Typ: Es ersetzt Interfaces und dient als Constraint für Typ-Parameter (`class List<T : Printable>`).
- [A] Namenskonflikte zwischen Mixins oder mit der Klasse sind ein Compilerfehler, der explizit aufgelöst werden muss.
- [E] Aufgelöst wird ein Konflikt, indem die Klasse die Methode selbst mit `override` definiert; darin ruft `Printable::describe()` gezielt die Fassung eines Mixins auf (Abschnitt 20).

## 10. Typ-Parameter

- [E] Typ-Parameter sind möglich, ohne die Probleme von C++-Templates und Java-Type-Erasure.
- [A] Umsetzung: Generics werden an der Definition gegen ihre Constraints geprüft. Im Interpreter bleiben Typargumente zur Laufzeit erhalten, im Release wird pro Typ eigener Code erzeugt.
- [E] Mehrere Constraints werden mit `&` verbunden: `class SortedList<T : Comparable & Printable>`.
- [E] Statischer Zugriff an einem generischen Typ: `List<Foo>::create()`.
- [E] Generische Typen sind invariant: `List<Circle>` ist kein `List<Shape>`.
- [E] Generische Methoden: `static T max<T : Comparable>(T a, T b) { ... }`. Typargumente werden beim Aufruf nur aus den Argumenten abgeleitet, nie ausgeschrieben (`max<i32>(a, b)` wäre ein Vergleich). Lässt sich ein Typ-Parameter nicht ableiten, ist das ein Compilerfehler.

## 11. Kontrollstrukturen, Enums, Exceptions, Lambdas

- [E] `if`, `for`, `while`, `do`-`while`, `switch`-`case` wie in der C-Familie.
- [A] `switch` ohne Fallthrough und mit Prüfung auf Vollständigkeit.
- [A] Enums mit optionalen Daten pro Variante.
- [E] Varianten ohne Daten dürfen einen festen Zahlwert tragen: `enum Color { Red = 1, Green = 2 }`. `cast<i32>(c)` liefert den Wert (ohne Angabe die Position), `cast<Color>(n)` wirft, wenn kein Wert passt. `==` gibt es für Enums ohne Daten.
- [A] Als eigenständige Ausdrucksanweisung sind nur Aufrufe, Zuweisungen und `++`/`--` erlaubt (wie Java/C#).
- [E] Zuweisungen (auch `+=` usw.) sind nur Anweisungen, keine Ausdrücke; `++` und `--` dürfen auch in Ausdrücken stehen (Abschnitt 21).
- [E] Exceptions.
- [A] Exceptions sind ungeprüft (keine `throws`-Listen); `try`/`catch`/`finally`; Destruktoren laufen beim Unwinding.
- [E] Geworfen werden nur `own`-Objekte von Unterklassen von `rpp.core.Exception`; Programmfehler sind nicht fangbar (Abschnitt 22).
- [E] Lambdas in C#-Schreibweise: `(i32 a) => ausdruck` oder `(i32 a) => { ... }`. Keine Capture-Liste.
- [E] Normales Lambda: erfasst Variablen automatisch per Borrow (lesend oder verändernd, je nach Rumpf) und darf nur als Argument übergeben, nicht gespeichert werden.
- [E] `own`-Lambda (`own (i32 x) => x + offset`): erfasst per Wert. Primitive Werte werden kopiert, Strings und `shared`-Objekte geteilt, `own`-Objekte hineinverschoben. Nur `own`-Lambdas dürfen zurückgegeben oder gespeichert werden.
- [E] Funktionstypen: `Func<Param1, ..., Rückgabetyp>`. Parameter tragen dieselben Markierungen wie Methodenparameter: `Func<mut Foo, void>` leiht veränderbar, `Func<own Foo, void>` übernimmt (Abschnitt 23).
- [E] Schleife über Arrays: `for (string c : colors) { ... }`; das Element ist geliehen. Verändernd: `for (mut Foo f : mut list) { ... }`.
- [E] Funktionstypen ohne Rückgabewert: `Func<i32, void>`; ohne Parameter: `Func<void>` bzw. `Func<string>`. `void` ist als Typargument nur an letzter Stelle von `Func` erlaubt.

## 12. Threads und C-Anbindung

- [A] Version 1: Jedes Objekt gehört zu einem Thread; Übergabe an einen anderen Thread nur per `own`.
- [A] C-Anbindung (FFI): zurückgestellt, bis der Sprachkern steht.

## 13. Quelldateien und Projektstruktur

- [E] Nur Sourcedateien. `package my.pkg;` am Dateianfang.
- [E] Paketnamen dürfen keine Schlüsselwörter enthalten (`package my.package;` ist ein Fehler).
- [E] Optionale `using`-Anweisungen, um Klassen ohne vollständig qualifizierten Namen zu verwenden.
- [E] Der Paketname muss dem Verzeichnis relativ zum Quellordner entsprechen: `src/my/pkg/Foo.rpp` enthält `package my.pkg;`.
- [E] Projektdatei `rpp.yaml` im Projektverzeichnis mit den Einträgen `name`, `version`, `type` (`executable` oder `library`), `main` (nur bei Programmen: Sourcedatei mit `main`), `sources` (Quellordner), `dependencies` und `symbols` (eigene Symbole je Modus).
- [E] Abhängigkeiten werden in `dependencies` eingetragen: lokal mit `name` und `path`, aus dem zentralen Verzeichnis mit `name` und `version` (Abschnitt 13b).
- [E] Die Pakete einer Bibliothek beginnen mit ihrem Namen (`mathlib.geometry`). So können zwei Bibliotheken nie dasselbe Paket enthalten.
- [E] Nur direkt eingetragene Abhängigkeiten sind verwendbar. Typen aus den Abhängigkeiten einer Bibliothek kann ein Projekt nur nutzen, wenn es diese selbst einträgt.
- [E] Bibliotheken werden in der Zwischenrepräsentation ausgeliefert, die Debug- und Releasemodus gemeinsam nutzen. Nur so kann der Release-Compiler generische Typen einer Bibliothek für neue Typargumente erzeugen.
- [E] Präzisierung (21.11.2026): Weil `#if` vor dem Parsen aufgelöst wird, hängt die Zwischenrepräsentation von Modus und Betriebssystem ab. Eine Bibliothek wird deshalb als Quelltext veröffentlicht; die Zwischenrepräsentation wird daraus je Kombination gebaut (Abschnitt 13b). Der Compiler hat damit für generische Typen immer die Zwischenrepräsentation zur Verfügung.
- [E] Bedingte Kompilierung über Zeilen, die mit `#` beginnen: `#if debug`, `#else`, `#endif`. Inaktive Zweige werden beim Parsen übersprungen, also weder syntaktisch noch semantisch geprüft.
- [E] Zusätzlich `#elif`; Bedingungen mit `!`, `&&`, `||` und Klammern, z. B. `#if debug && !windows`.
- [E] Vordefinierte Symbole: genau eines von `debug` oder `release`, dazu das Betriebssystem (`windows`, `linux`, `macos`). Eigene Symbole nur in der Projektdatei, nicht im Code; es gibt kein `#define`.

## 13a. Export

- [E] `export` vor `class`, `mixin` oder `enum` macht einen Typ in anderen Projekten nutzbar. Ohne `export` ist ein Typ nur im eigenen Projekt sichtbar.
- [E] Member werden nicht einzeln exportiert. In einem anderen Projekt sichtbar ist ein Member, wenn sein Typ exportiert und der Member `public` ist; für Unterklassen in anderen Projekten zusätzlich `protected`.
- [E] Öffentliche Member exportierter Typen dürfen in ihrer Signatur nur exportierte Typen verwenden; sonst meldet der Compiler einen Fehler.

## 13b. Zentrales Verzeichnis für Bibliotheken

Entschieden am 21.11.2026. Punkte mit [A] stammen aus dem Vorschlag und wurden ohne Einwand übernommen.

- [E] Namen mit Besitzer: Eine Bibliothek im Verzeichnis heißt `besitzer.name` (`acme.json`); der Besitzer ist ein Benutzer- oder Organisationskonto. Die Pakete der Bibliothek beginnen mit diesem Namen (`acme.json.parser`). Besitzer- und Bibliotheksnamen dürfen keine Schlüsselwörter enthalten.
- [E] Versionsauflösung nach Minimal Version Selection (wie Go): Jede Abhängigkeit nennt eine Mindestversion; für jede Bibliothek wird die höchste genannte Mindestversion genommen. Pro Programm gibt es jede Bibliothek nur in einer Version; verlangen Abhängigkeiten verschiedene Hauptversionen, ist das ein Fehler. Zwei Hauptversionen nebeneinander gibt es nur unter verschiedenen Namen (`acme.json2`).
- [E] Veröffentlicht wird Quelltext. Das Verzeichnis baut daraus reproduzierbar die Zwischenrepräsentation für die sechs Standardkombinationen (`debug`/`release` × `windows`/`linux`/`macos`). Der Compiler lädt die passende Variante; bei eigenen Symbolen oder fehlender Variante baut er sie selbst.
- [A] Versionen folgen Semantic Versioning. `rpp publish` vergleicht die exportierte Schnittstelle (exportierte Typen mit ihren `public`- und `protected`-Membern) mit der Vorversion; entfernte oder geänderte Teile verlangen eine neue Hauptversion, neue Teile eine neue Nebenversion. Sonst lehnt das Verzeichnis die Veröffentlichung ab.
- [A] Sperrdatei `rpp.lock` mit exakter Version und Prüfsumme jeder Bibliothek. Ein öffentliches Protokoll aller veröffentlichten Prüfsummen macht nachträgliches Austauschen erkennbar.
- [A] Veröffentlichte Versionen sind unveränderlich; statt zu löschen, werden sie als zurückgezogen markiert. Bestehende Sperrdateien bauen weiter.
- [A] Eintrag in `rpp.yaml`: `dependencies` mit `name` und `version` (Mindestversion) oder `path`; optional `registry` für ein eigenes oder gespiegeltes Verzeichnis.
- [A] Nicht in Version 1: optionale Funktionen einer Bibliothek (wie `features` in Cargo), mehrere Versionen derselben Bibliothek in einem Programm, Skripte, die beim Bauen laufen.
- [E] Die Standardbibliothek steht unter dem reservierten Besitzer `rpp` (`rpp.collections`, `rpp.io`); niemand sonst kann unter `rpp` veröffentlichen. Ihr Umfang folgt in Kapitel 13.

## 14. Variablen und Konstanten

- [E] `const` kennzeichnet Konstanten.
- [E] Typinferenz mit `var`: Der Typ ergibt sich aus dem Initialisierungsausdruck.
- [E] Pro Deklaration genau eine Variable; `i32 a, b;` gibt es nicht. Mehrere Variablen auf einmal werden nur per Tupel-Deklaration eingeführt: `(i32 a, string b) = f();`
- [E] Im `for`-Kopf ist auch eine Tupel-Deklaration erlaubt: `for ((i32 i, i32 j) = (0, n); i < j; i++, j--) { ... }`
- [E] Lokale `const`-Variablen dürfen zur Laufzeit berechnet werden, aber nicht neu zugewiesen werden.
- [E] `const` gilt tief: Auf einem `const`-Objekt dürfen keine `mut`-Methoden aufgerufen werden.
- [E] `const`-Felder werden nur im Konstruktor gesetzt. `static const` verlangt einen Wert, der zur Compilezeit feststeht.
- [E] Parameter tragen kein `const`; ohne `mut` sind sie ohnehin nur lesbar.
- [E] Sichere Initialisierung: Lokale Variablen müssen vor dem ersten Lesen auf jedem Weg zugewiesen sein; es gibt keine Standardwerte. Nullbare Felder ohne Initialisierer sind `null`; alle anderen setzt der Konstruktor.

## 15. Eingebaute Funktionen und Standardbibliothek

- [E] `assert` wird in beiden Modi geprüft und wirft bei `false` (Abschnitt 19).
- [E] Eingebaute Funktionen in Version 1: `print(string)` und `assert(bool, string)`, dazu `replace` und `take` für Felder (Abschnitt 17, V-3) und `same(a, b)` für die Identität von Objekten (Abschnitt 8) und `fail(text)` für einen bewussten Programmfehler (Abschnitt 22).
- [E] Umfang der Standardbibliothek: erster Schnitt, der für einen selbstübersetzenden Compiler reicht (Abschnitt 24).

## 16. Befunde aus dem Grammatiktest

Die Grammatik liegt als „RPP.g4“ vor und ist getestet. Beim Umsetzen wurden folgende Punkte festgelegt und anschließend bestätigt:

- [E] Lambdas stehen auf Ausdrucksebene. Der Rumpf reicht bis zum Ende des umgebenden Ausdrucks; als Operand oder für einen direkten Aufruf muss ein Lambda geklammert werden. Ohne diese Regel ist `(i32 x) => x + 1` mehrdeutig.
- [E] Case-Marken sind Literale, Konstanten- oder Variantennamen und Varianten mit Bindungen (`case Circle(r):`, qualifiziert `case Shape::Circle(r):`), keine beliebigen Ausdrücke. Sonst wäre `Circle(r)` zugleich Aufruf und Muster.
- [E] Schiebeoperatoren bestehen aus zwei Tokens, damit `List<List<i32>>` lesbar ist. Nebenwirkung: `a > > b` mit Leerzeichen wird akzeptiert.
- [E] Links von `=` steht nur ein Name, Feld-, Indexzugriff oder Tupel.

Befunde aus dem Test, inzwischen entschieden:

- [E] Schlüsselwörter in Paketnamen sind verboten (siehe Abschnitt 13).
- [E] Die Kopie wird mit `copy(a)` aufgerufen (siehe Abschnitt 8).
- [E] `List<Foo>::create()` ist erlaubt (siehe Abschnitt 10; ursprünglich mit `.`, seit 20.11.2026 mit `::`).
- [E] Alle Schlüsselwörter bleiben reserviert, auch `shared`, `weak`, `own`, `base` und `cast`. Einzige Ausnahme ist `main`.

## 17. Festlegungen aus dem Ownership-Kernkalkül

Beim Ausarbeiten von Kapitel 5 („R++ Ownership-Kernkalkül“) sind folgende Punkte aufgetaucht und am 18.11.2026 entschieden worden. Begründungen stehen dort in Abschnitt 2.

- [E] V-1: `shared`-Objekte sind veränderbar. Die Exklusivität wird für sie zur Laufzeit geprüft; ein Konflikt wirft `BorrowError`, in beiden Modi.
- [E] V-2: Zugriffe gehen nie direkt durch ein `shared`- oder `weak`-Feld; der Compiler legt dafür eine lokale Hilfsvariable an. Im Quelltext schreibt man weiterhin `this.next.name()`.
- [E] V-3: Aus Feldern wird nicht herausbewegt. Dafür gibt es zwei eingebaute Funktionen: `replace(mut x.f, neu)` setzt den neuen Wert und liefert den alten; `take(mut x.f)` gibt es nur für nullbare Felder, setzt das Feld auf `null` und liefert den alten Wert.
- [E] V-4 (geändert am 21.11.2026): Methoden dürfen Borrows zurückgeben, die von `this` ausgehen (`ref V get(K k)`, veränderbar `ref mut V at(K k) mut` mit `return mut …;`). An der Aufrufstelle gilt der Borrow als Loan auf den Empfänger bis zum Ende der Anweisung oder, mit `ref` gebunden, bis zum Blockende. Borrows auf Parameter zurückzugeben bleibt verboten. Kernkalkül R-5.23, R-5.24; im FR++-Modell geprüft.
- [E] V-5: Ein Move in nur einem `if`-Zweig ist erlaubt; danach gilt die Variable als verschoben, freigegeben wird per Drop-Flag.
- [E] V-6: `weak` entsteht nur aus `shared`; Zugriff über `w.lock()` mit Ergebnis `shared T?`.
- [E] V-7: Freigabereihenfolge: lokale Variablen rückwärts; bei Objekten erst Destruktor, dann Felder rückwärts, Basisklasse zuletzt.
- [E] V-8: Ein lesender Borrow schützt nur, was über `own`-Kanten erreichbar ist; über `shared` erreichte Objekte bleiben veränderbar (mit Laufzeitprüfung nach V-1).
- [E] V-9: `shared`-Werte werden bei Zuweisung und Übergabe implizit kopiert (Zähler + 1). Aus `own` wird `shared` nur bei ausdrücklichem `shared`-Ziel (Variable, Feld oder Parameter); aus `shared` wird nie `own`, dafür gibt es `copy(s)`.
- [E] V-10: Im Konstruktor müssen alle Felder gesetzt sein, bevor `this` anders als zum Setzen von Feldern verwendet wird.
- [E] V-11: Eine Exception, die einen Destruktor verlässt, beendet das Programm.
- [E] Benannte lokale Borrows (20.11.2026): `ref Foo f = list[0];` leiht lesend, `ref mut Foo g = mut x.child;` veränderbar; das `mut` steht wie bei Parametern und Aufrufen vor Typ und Initialisierer. Der Borrow gilt bis zum Ende des umgebenden Blocks, wie bei lokalen Slices. Der Initialisierer muss ein Ort sein (Variable, Feld- oder Indexzugriff). `ref` gibt es nur bei lokalen Variablen, nicht an Feldern, Parametern, im `for`-Kopf, mit Besitzform oder mit `var`. `ref` ist ein reserviertes Wort.

## 18. Festlegungen aus Kapitel 4 (Typen)

Beim Ausformulieren von Kapitel 4 („R++ Typen“) mussten kleinere Punkte zusätzlich festgelegt werden; sie sind am 20.11.2026 entschieden worden. Einzelheiten stehen dort in Abschnitt 0.

- [E] V-4.0: `i32 + ui32` hat den gemeinsamen Typ `i64` und ist kein Fehler (Korrektur des ursprünglichen Beispiels); Fehler nur ohne gemeinsamen Typ (`i64 + ui64`).
- [E] V-4.1: `char` hat Vergleiche, keine Arithmetik; `cast<char>(n)` mit ungültigem Skalarwert wirft in beiden Modi.
- [E] V-4.2: Schiebeweite außerhalb des Bereichs: Exception im Debug, modulo Bitbreite im Release; `>>` arithmetisch bei vorzeichenbehafteten Typen.
- [E] V-4.3: Bitoperatoren nur für Ganzzahlen und `byte`; `bool` hat nur `!`, `&&`, `||`, `==`, `!=`.
- [E] V-4.4: Strings unveränderlich; `+` nur zwischen Strings; `==` nach Inhalt, `<` lexikografisch nach Skalarwerten.
- [E] V-4.5: Gleichheit über das generische Mixin `Equatable<T>` mit `bool equals(T other)`; primitive Typen, `string` und Enums ohne Daten sind von selbst `Equatable`.
- [E] V-4.6: `as` verlangt einen echten Untertyp; Ergebnis ist ein Borrow `C?` oder bei `shared`-Ziel `shared C?`; kein Downcast mit Besitzübergabe.
- [E] V-4.7: Enum-Varianten ohne Wert: vorheriger Wert + 1, die erste 0.
- [E] V-4.8: Tupel, Arrays und Funktionstypen haben kein `==`; Tupel-Literale übernehmen Elementtypen aus dem Kontext.
- [E] V-4.9: Keine automatische Umwandlung zwischen `T[N]` und `T[]`; Arraygröße ≥ 0.
- [E] V-4.10: Umwandlung zu Gleitkomma rundet zum nächsten Wert und wirft nie.
- [E] V-4.11: `null` braucht einen nullbaren Kontext (`var x = null;` ist ein Fehler); kein `T??`, kein `void?`.

## 19. Festlegungen aus Kapitel 6 (Null-Sicherheit)

Entschieden am 20.11.2026; Einzelheiten in „R++ Null-Sicherheit“.

- [E] Volle Flussanalyse für die Verengung: rechte Seite von `&&` und `||`, `!`, `? :`, `else`-Zweige, `while`- und `for`-Bedingungen und jeder Zweig, der nicht normal endet (`return`, `throw`, `break`, `continue`).
- [E] `assert(c, text)` ist in beiden Modi aktiv und verengt danach wie eine erfüllte Bedingung.
- [E] `x == null` und `x != null` mit nicht nullbarem `x` sind Fehler; ebenso `?.` und `??` auf nicht nullbaren Werten.
- [E] Ein generisches `T?` fällt mit `T` zusammen, wenn `T` selbst nullbar ist (`T = Foo?` ergibt `T? = Foo?`).
- [E] Eine Verengung endet mit der nächsten Zuweisung an die Variable; danach gilt der Typ des neuen Werts. Nach einem `if` ohne Ausstieg gilt die Schnittmenge beider Wege.
- [E] Nullbare Felder werden zum Prüfen geholt: `own`-Felder mit `ref` (oder herausgenommen mit `take`), `shared`-Felder per Kopie. Array-Elemente werden nie verengt, sondern ebenfalls mit `ref` geholt.
- [E] `x?.m()` hat den Typ `R?`; bei `void` nur als Anweisung. Bei `null` wird der Rest der Kette übersprungen. `a ?? b` ist `T`, wenn `b` nicht nullbar ist, sonst `T?`; `b` wird nur bei `null` ausgewertet.
- [E] Kein `??=` in Version 1.
- [E] `this`, Ergebnisse von `new`, Literale außer `null` und Parameter ohne `?` sind nie `null`.

## 20. Festlegungen aus Kapitel 7 (Klassen, Mixins, Typ-Parameter)

Entschieden am 21.11.2026; Einzelheiten in „R++ Klassen und Generics“.

- [E] Methoden und Konstruktoren dürfen überladen werden. Varianten dürfen sich nicht nur im Rückgabetyp oder nur in Besitz- und Borrow-Markierungen unterscheiden; eine statische und eine Instanzmethode dürfen nicht gleich heißen.
- [E] Auflösung nach Rangfolge pro Argument: genau gleicher Typ, dann Literal- und Zahlanpassung, dann Upcast, dann `T` → `T?`. Gewählt wird die Variante, die nirgends schlechter und mindestens einmal besser ist; nicht-generische Varianten schlagen generische; sonst Mehrdeutigkeitsfehler.
- [E] Zusatzregel (beim Ausformulieren ergänzt, am 21.11.2026 angenommen): Bei gleichem Rang gewinnt der speziellere Parametertyp (der sich automatisch in den anderen umwandeln lässt, z. B. `i32` vor `i64`, `Circle` vor `Shape`). Ohne sie wäre `f(x)` mit `x : i8` bei `f(i32)` und `f(i64)` mehrdeutig.
- [E] Ein Mixin-Konflikt wird gelöst, indem die Klasse die Methode selbst mit `override` definiert; `M::m()` ruft darin die Fassung des Mixins `M` auf, `base.m()` die der Basisklasse.
- [E] `copy` ist virtuell: `copy(a)` kopiert nach der tatsächlichen Klasse. Hat eine Klasse `copy`, muss jede Unterklasse ein eigenes `copy` haben; der Basisanteil wird mit dem `copy` der Basisklasse kopiert.
- [E] Member vor dem ersten Sichtbarkeits-Label sind `private`.
- [E] Hat eine Klasse keinen Konstruktor, gibt es einen parameterlosen, wenn jedes Feld einen Initialisierer hat oder nullbar ist.
- [E] `override` verlangt exakt gleiche Parametertypen, Besitzformen, `mut`-Modus und Rückgabetyp; die Sichtbarkeit darf nicht enger werden. Überschriebene Methoden bleiben virtuell; kein `sealed` in Version 1.
- [E] Mixins dürfen andere Mixins importieren. Sie haben keine statischen Methoden und keine Konstanten; alle Methoden sind `public`.
- [E] Generische Erzeugung über den Constraint `new()` wie in C#: `class Pool<T : new()>` erlaubt `new T()`; das Typargument braucht einen parameterlosen Konstruktor.
- [E] Virtuelle Aufrufe sind im Konstruktor erst nach vollständiger Initialisierung erlaubt und dann dynamisch; im Destruktor ebenfalls dynamisch.

## 21. Festlegungen aus Kapitel 8 (Anweisungen und Ausdrücke)

Entschieden am 21.11.2026; Einzelheiten in „R++ Anweisungen und Ausdrücke“.

- [E] Zuweisungen und Verbundzuweisungen sind nur Anweisungen (und im `for`-Kopf); `x = y = 0;` und `if ((x = f()) != null)` gibt es nicht. `++` und `--` dürfen in Ausdrücken stehen: `i++` liefert den alten, `++i` den neuen Wert.
- [E] Auswertung von links nach rechts; Empfänger vor Argumenten; bei Zuweisungen erst die Teile des Ziels, dann die rechte Seite.
- [E] `x op= e` bedeutet `x = x op e` mit einmaliger Auswertung von `x`; das Ergebnis muss automatisch in den Typ von `x` passen.
- [E] Methoden mit Rückgabetyp müssen auf jedem Weg mit `return` oder `throw` enden.
- [E] `switch` über Ganzzahlen, `char`, `string` und Enums; doppelte Marken sind Fehler; ohne `default` nur bei vollständigen Enums. Gebundene Daten sind Borrows.
- [E] `break` direkt in einem `case` beendet das `switch` (wie in C/Java); `continue` gilt immer der Schleife.
- [E] Typ von `c ? a : b`: gemeinsamer Zahltyp oder der Typ des Zweigs, in den der andere automatisch übergeht; keine Suche nach gemeinsamer Basisklasse.
- [E] `(a, b) = (b, a);` wertet erst die rechte Seite aus; Tauschen funktioniert.
- [E] `for (x : c)` läuft über Arrays, Vektoren, Slices, Strings und Klassen mit dem Bibliotheks-Mixin `Iterable<T>`.
- [E] Ein unbenutztes Ergebnis eines Aufrufs ist ein Fehler. Absichtliches Verwerfen: `discard list.pop();`. Methoden mit `nodiscard` können auch mit `discard` nicht verworfen werden.
- [E] Unerreichbarer Code erzeugt eine Warnung; damit kennt die Spezifikation neben Fehlern auch Warnungen.
- [E] Keine Sprungmarken (`break outer;`) und keine Bereiche auf Strings in Version 1.
- [E] V-8.1: Lokale Variablen dürfen andere lokale Variablen und Parameter nicht verdecken; Felder schon (beim Ausformulieren ergänzt, am 21.11.2026 angenommen).
- [E] V-8.2: `nodiscard` gehört zur Signatur und muss beim Überschreiben und beim Erfüllen einer Anforderung übernommen werden; an `void`-Methoden ist es ein Fehler (beim Ausformulieren ergänzt, am 21.11.2026 angenommen).

## 22. Festlegungen aus Kapitel 9 (Exceptions)

Entschieden am 21.11.2026; Einzelheiten in „R++ Exceptions“.

- [E] Zwei Arten: Exceptions (mit `throw` ausgelöst, mit `catch` fangbar) und Programmfehler (Index, Überlauf, Division durch 0, ungültiger `cast`, `assert`, `BorrowError`, Exception aus einem Destruktor, `fail`). Programmfehler sind nicht fangbar: Destruktoren und `finally` laufen noch, dann endet das Programm. So verzweigen Debug und Release nie unterschiedlich.
- [E] Geworfen werden nur `own`-Objekte von Unterklassen von `rpp.core.Exception` (Felder `message`, `cause`, `suppressed`).
- [E] Ein `finally`-Block darf nicht mit `return`, `break` oder `continue` verlassen werden; ein direktes `throw` darin ist ein Fehler.
- [E] Wirft ein Aufruf im `finally`, während schon eine Exception läuft, ersetzt die neue die alte; die alte wird in `suppressed` der neuen gespeichert.
- [E] Der erste passende `catch` fängt; ein `catch` hinter einem `catch` seiner Oberklasse erzeugt eine Warnung (unerreichbar).
- [E] Die `catch`-Variable besitzt die Exception; `throw e;` wirft sie weiter (Move).
- [E] Wirft ein Konstruktor, werden gesetzte Felder und ein fertiger Basisanteil freigegeben; der Destruktor der Klasse selbst läuft nicht.
- [E] Exit-Codes: ungefangene Exception 1, Programmfehler 2; Meldung auf der Fehlerausgabe.
- [E] Stacktrace im Debugmodus immer, im Releasemodus der Implementierung überlassen.
- [E] Kein `using`/try-with-resources; Aufräumen über Destruktoren.
- [E] Neue eingebaute Funktion `fail(text)`: löst einen Programmfehler aus und kehrt nie zurück (zählt wie `throw` für Verengung, Rückgabe und Erreichbarkeit).
- [E] Ein `try` braucht mindestens ein `catch` oder ein `finally`.

## 23. Festlegungen aus Kapitel 10 (Lambdas)

Entschieden am 21.11.2026; Einzelheiten in „R++ Lambdas“.

- [E] Erfasste Werte eines `own`-Lambdas sind unveränderlich; ein Aufruf verändert das Lambda nie. Veränderlicher Zustand liegt in einem erfassten `shared`-Objekt, das über den Verweis veränderbar bleibt (wie V-8).
- [E] In `Func` tragen Parameter die Markierungen von Methodenparametern: `Func<Foo, …>` lesend, `Func<mut Foo, …>` veränderbar, `Func<own Foo, …>` übernommen. `mut` ist nur dort erlaubt.
- [E] Ein `own`-Lambda darf `this` nicht verwenden; man holt Werte vorher in lokale Variablen.
- [E] Ein `own`-Lambda ohne erwarteten Typ bekommt seinen `Func`-Typ aus Parametern und Rumpf: `var f = own (i32 x) => x + 1;` ist `Func<i32, i32>`.
- [E] Ein `own`-Lambda kann keine Borrow-Variable eines Objekttyps erfassen; man schreibt vorher `copy(x)`.
- [E] Ein normales Lambda leiht eine Variable veränderbar, wenn der Rumpf sie verändert, sonst lesend; die Borrows gelten für die Dauer des Aufrufs. Außer als Argument darf es nur geklammert direkt aufgerufen werden.
- [E] Ein `own`-Lambda erfasst beim Auswerten des Lambda-Ausdrucks; `own`-Werte werden hineinverschoben.
- [E] Funktionswerte sind `own`-Objekte: Zuweisen verschiebt, `shared` ist möglich, `copy` nicht.
- [E] Rückgabetyp aus dem Ausdrucksrumpf bzw. als gemeinsamer Typ aller `return`-Werte; gegen einen erwarteten `Func`-Typ wird geprüft.
- [E] Aufruf mit `f(args)`, auch über Felder und Parameter; liest nur. Ein `Func`-Parameter ohne `own` darf beliebig oft aufgerufen und weitergegeben, aber nicht gespeichert werden.
- [E] Lambdas liefern keine Borrows.

## 24. Festlegungen aus Kapitel 13 (Standardbibliothek, erster Schnitt)

Entschieden am 21.11.2026; Einzelheiten in „R++ Standardbibliothek“.

- [E] Der erste Schnitt umfasst genau, was ein in R++ geschriebener, selbstübersetzender Compiler braucht. Bis dahin bleiben die Bibliotheken unter `rpp` auf Version 0.x und dürfen sich ändern.
- [E] Module: `rpp.core` (Exceptions, `Equatable`, `Comparable`, `Hashable`, `Iterable`), `rpp.text` (String-Methoden, `StringBuilder`, Zeichenklassen), `rpp.fmt` (Zahlen ↔ Text), `rpp.collections` (Vektor-Methoden, `Map`, `Set`, Sortieren), `rpp.io` (Dateien, Verzeichnisse, Pfade, Konsole), `rpp.env` (Umgebungsvariablen, Plattform).
- [E] Iteration eigener Klassen über ein Cursor-Protokoll: `Iterable<T, C>` mit `begin`, `done`, `next` und `ref T at(C)`. Der Cursor ist ein gewöhnlicher Wert; die Schleife leiht die Sammlung. Iterator-Objekte gibt es nicht, weil sie Borrows in Feldern bräuchten.
- [E] Primitive Typen und `string` haben eingebaute Methoden und statische Member: `n.toString()`, `s.substring(1, 3)`, `i32::parse(s)`, `i64::MAX`.
- [E] `N::parse(s)` wirft eine `FormatException`; `N::tryParse(s)` liefert `N?`.
- [E] V-13.1: Mehrere Bibliotheken unter `rpp`, mit dem Compiler ausgeliefert, ohne Eintrag in `rpp.yaml`; `rpp.core` ist ohne `using` sichtbar.
- [E] V-13.2: Deterministische Iteration: `Map` und `Set` in Einfügereihenfolge, `Dir::list` sortiert.
- [E] V-13.3: Kein `exit`; den Exit-Code liefert `main`.
- [E] V-13.4: `print(text)` schreibt mit Zeilenumbruch; ohne Umbruch `Console::write`.
- [E] V-13.5: `get(k)` liefert `ref V?`; `at(k)` verlangt einen vorhandenen Schlüssel, sonst Programmfehler.
- [E] V-13.6: Eine Borrow-Methode darf auch über `ref`-Variablen, Schleifenelemente und weitere Borrow-Aufrufe zurückgeben, die von `this` ausgehen (im FR++-Modell noch nicht abgebildet).
- [E] V-13.7: `<`, `<=`, `>`, `>=` für Typen mit `Comparable` rufen `compareTo` auf.

## 24a. Ausbau der Standardbibliothek und rpp.random

Entschieden am 23.11.2026; Einzelheiten in „R++ Standardbibliothek“, Abschnitte 9 und 10.

- [E] V-13.9: Neue Pakete unter `rpp` ergänzen die Bibliothek, ohne den ersten Schnitt zu ändern; sie bleiben bis zur Selbstübersetzung auf 0.x. Beim Portieren aus anderen Sprachen: kein `Optional`/`Result` (`T?`, `try…`, Exceptions leisten das), Leihen vor Verbrauchen, Determinismus, gleiches Verhalten in beiden Modi (kein Verlass auf das Überlaufverhalten des Releasemodus). Neue Pakete werden, wo möglich, in R++ selbst geschrieben und dienen als Lasttest für M6.
- [E] Der Ausbau folgt einer Roadmap in drei Phasen (Arbeitsstand): A neben M6 (Vektor-Algorithmen, `Deque`, `rpp.math`, `rpp.random`, Text- und IO-Ausbau), B nach der Selbstübersetzung (`rpp.iter`, `rpp.regex` nach RE2-Art, `rpp.json`, `rpp.time`, `rpp.encoding`, `rpp.hash` mit SHA-256 für `rpp.lock`), C mit den Threads (`rpp.sync` mit Channels per `own`, `rpp.process`, `rpp.net`).
- [E] R-13.23: `rpp.random.Random` ist ein seedbarer Generator mit festgelegtem Algorithmus: Seeding durch SplitMix64, Schritt durch `xoshiro256**`; gleicher Seed liefert auf jeder Plattform und in beiden Modi dieselbe Folge. Die Arithmetik modulo 2⁶⁴ wird ohne Überlauf gerechnet.
- [E] R-13.24: `nextUi64()`, `nextInt(bound)` (gleichverteilt ohne Modulo-Bias durch Verwerfen; `bound ≤ 0` ist ein `FailError`), `nextFloat()` in `[0, 1)` mit 53 Bit, `nextBool()` als oberstes Bit.
- [E] R-13.25: `shuffle<T>(mut T[] v)` mischt mit Fisher-Yates über `remove`/`insert`; ein Tausch zweier Indizes ist nicht möglich (V-3, Überlappung unbekannter Indizes). Feste Arrays lassen sich in Version 1 nicht mischen.
- [E] V-13.10: Vektor-Algorithmen, die in R++ schreibbar sind, kommen als statische generische Methoden der Hilfsklasse `Vectors`, nicht als eingebaute Methoden auf `T[]` (eingebaute Methoden erzeugen bis zum Nachziehen in M6 eine Divergenz). Eingebaute Methoden bleiben, was R++ nicht effizient ausdrücken kann (`sort`). Veränderbare Algorithmen nutzen `remove`/`insert` und sind quadratisch; das genügt für die Referenz.
- [E] R-13.26: `Vectors.binarySearch` (Comparable, aufsteigend sortierter Slice, Position oder `null`) und `Vectors.equals` (Equatable, elementweiser Vergleich; füllt die Lücke, dass Arrays kein `==` haben).
- [E] R-13.27: `Vectors.reverse`, `retain` (Prädikat) und `dedup` (aufeinanderfolgende Duplikate) verändern `mut T[]` über `remove`/`insert`; funktionieren für jeden Elementtyp, auch `own`-Objekte.
- [E] R-7.26 auch für Methodenaufrufe durchgesetzt: Bisher prüfte `rppc` Constraints nur bei Klassen-Instanziierung, nicht bei generischen Methoden; ein Aufruf wie `binarySearch` mit einem Typ ohne `Comparable` wurde angenommen und scheiterte erst zur Laufzeit. Beim Schreiben von `Vectors` aufgefallen und in `rppc` (`body.rs`) und im Compiler in R++ (`Checker.rpp`) behoben; neuer Test `n08_vectors_ohne_comparable`. Die Spezifikation (R-7.26) verlangte die Prüfung bereits.
- [E] V-13.11: `rpp.math` liefert `f64`-Funktionen. Runden und `clamp` sind reiner R++-Quelltext; `sqrt` ist eingebaut (korrekt gerundet nach IEEE 754, Muster `fail("eingebaut")` ohne M6-Divergenz). Transzendente Funktionen folgen später (plattformabhängig, nicht bitgleich).
- [E] R-13.28: `Math.PI`/`Math.E`, `sqrt` (korrekt gerundet, `x < 0` → NaN), `floor`/`ceil`/`trunc`/`round` (exakt ganzzahlig, `round` bei .5 von null weg); NaN/±∞/±0 bleiben erhalten.
- [E] R-13.29: `Math.clamp(x, lo, hi)` begrenzt auf `[lo, hi]`; `lo > hi` ist ein `FailError`, NaN bleibt NaN.
- [E] R-13.30: `rpp.text.Text` (reiner R++-Quelltext nach dem Muster von `Fmt`) ergänzt die String-Methoden um `lines` (CRLF-fähig, kein abschließendes Leerzeilen-Element), `removePrefix`/`removeSuffix`, `repeat` (`n < 0` → `FailError`) und `splitLimit` (höchstens limit Teile, letzter Teil ist der Rest; `limit < 1` oder leerer sep → `FailError`).
- [E] R-13.31: `File` erhält `appendText`, `copyFile`, `moveFile`, `delete`, `size` (eingebaut, Dateisystemzugriff; `IOException` bei Fehlern). `copy` ist ein Schlüsselwort, daher `copyFile`/`moveFile`. `copyFile`/`moveFile` ersetzen ein vorhandenes Ziel. Mtime und gepuffertes Lesen folgen später (Mtime braucht `rpp.time`).
- [E] R-13.32: `Deque<T>` (reiner R++-Quelltext, vektorbasiert, `Iterable<T, i64>`) mit `pushFront`/`pushBack`, `popFront`/`popBack` (leer → `IndexError`), `front`/`back` (`ref T?`), `length`/`isEmpty`/`clear`. Operationen am Anfang sind linear; ein Ringpuffer wäre eine spätere eingebaute Variante.
- [E] R-13.33: `Map.getOrPut(own K key, Func<V> factory)` liefert `ref mut V`; fehlt der Schlüssel, wird `factory()` eingesetzt (nur dann aufgerufen). Eingebaut, damit auch `own`-Schlüssel gehen (eine reine R++-Fassung müsste den verschobenen Schlüssel nach `put` erneut verwenden).

## 25. Festlegungen aus Kapitel 1, 11, 12 und 14

Entschieden am 21.11.2026; Einzelheiten in „R++ Einleitung“, „R++ Pakete und Projekte“, „R++ Ausführungsmodell“ und „R++ Konformitätstests“.

- [E] `using` bindet einzelne Typen ein (`using a.b.C;`) oder unter anderem Namen (`using X = a.b.C;`); keine Platzhalter.
- [E] Eine Datei darf beliebig viele Typen enthalten, ihr Name ist frei; nur das Verzeichnis muss zum Paket passen. Dateien direkt im Quellordner gehören zum Wurzelpaket ohne `package`-Zeile.
- [E] Pakete eines Projekts dürfen sich gegenseitig verwenden; der Abhängigkeitsgraph zwischen Projekten ist zyklenfrei.
- [E] Namensauflösung: lokale Namen → Member → Typ-Parameter → eigenes Paket → `using` → `rpp.core`. Zwei gleichnamige `using` sind ein Fehler bei Verwendung.
- [E] Stacküberlauf und Speichermangel sind Programmfehler (Exit-Code 2).
- [E] Gleitkomma bitgenau nach IEEE 754 in beiden Modi, ohne FMA-Zusammenfassung und erweiterte Genauigkeit.
- [E] Version 1 ist einfach-threaded; die Thread-Regeln bleiben normativ, eine Thread-Bibliothek kommt später.
- [E] Die Testsuite hat neben akzeptierten und abgelehnten Programmen Laufzeittests mit erwarteter Ausgabe und erwartetem Exit-Code je Modus.
- [E] Der Testlauf prüft, welche Regeln in keinem Testkopf genannt werden, und meldet die Lücken.
- [E] Eine Implementierung ist konform, wenn sie alle Tests der Suite besteht.
- [E] V-12.1: `args` enthält die Argumente ohne den Programmnamen (angenommen am 21.11.2026).
- [E] V-12.2: Mindestens 10 000 verschachtelte Methodenaufrufe sind garantiert (angenommen am 21.11.2026).
- [E] Gleitkommaliterale dürfen einen Exponenten haben, aber nur nach Ziffern mit Punkt: `1.0e10`, `2.5E-3`; `1e10` bleibt ein Syntaxfehler, damit jedes Gleitkommaliteral an seinem Punkt erkennbar ist (entschieden am 21.11.2026).

## 26. Regelabdeckung der Konformitätstests

Am 21.11.2026 beim Schließen der Lücken in der Regelabdeckung; Einzelheiten in „R++ Konformitätstests“.

- [E] Tests, die mehrere Dateien oder Projekte brauchen, sind Verzeichnisse mit `rpp.yaml` (Projekttests); in YAML markiert `# FEHLER` die Fehlerzeile.
- [E] Ein abgelehnter Test darf an einem Syntaxfehler oder einer Direktive scheitern, wenn der erste Fehler in der markierten Zeile liegt.
- [E] Regeln, die sich nicht mit einem Testprogramm prüfen lassen, stehen mit Begründung in einer Liste in Kapitel 14 und zählen nicht als Lücke.
- [E] Nach einer Direktive darf ein Zeilenkommentar stehen (`#if debug // …`).
- [E] V-4.12 (angenommen am 21.11.2026): Enum-Werte entstehen ohne `new`: `Color::Red`, `Shape::Circle(2.0)`. Bisher war nicht festgelegt, wie ein Wert mit Daten erzeugt wird.
- [E] V-9.1 (angenommen am 21.11.2026): Jede Art von Programmfehler hat einen festen Namen für die Meldung (`IndexError`, `OverflowError`, `DivisionError`, `CastError`, `AssertError`, `BorrowError`, `DestructorError`, `FailError`, `StackOverflowError`, `OutOfMemoryError`). Laufzeittests prüfen diese Namen; bisher waren nur drei davon beispielhaft genannt.
- [E] V-14.1 (angenommen am 21.11.2026): Laufzeittests können Kommandozeilenargumente und Standardeingabe festlegen (`// Argumente: …`, `// Eingabe: "…"`).

## 27. Erste Implementierung

Entschieden am 21.11.2026; Einzelheiten in „R++ Implementierungsplan“.

- [E] Die erste Implementierung (Frontend, Debug-Interpreter als Referenz, später JIT und Releasemodus) wird in Rust geschrieben. Sie bleibt der Bootstrap-Compiler, bis R++ sich selbst übersetzt.
- [E] Aufbau und Meilensteine nach „R++ Implementierungsplan“ (angenommen am 21.11.2026 mit dem Beginn von M1): Cargo-Workspace `rppc/`, Parser von Hand (rekursiver Abstieg und Vorrangparser), gemeinsame Zwischenrepräsentation, Interpreter mit Prüfmodus für die Invarianten aus Kapitel 5, Meilensteine M1 bis M6.
- [E] Die negativen Grammatiktests und die Lesarten liegen als Dateien in `rpp-grammatik/tests/neg` und `tests/lesarten`; der ANTLR-Testlauf und der Rust-Parser lesen dieselben Dateien.
- [E] Meilenstein M1 ist abgeschlossen: Der Rust-Parser erkennt dieselbe Sprache wie `RPP.g4`, geprüft mit allen Grammatik- und Konformitätstests und einem Differenztest an mutierten Programmen.
- [E] Meilenstein M2 ist abgeschlossen: Projekte, Namensauflösung und Typprüfung; alle Konformitätstests außer denen für M3 bestehen.
- [E] V-4.13 (angenommen am 21.11.2026): Klarstellung zu R-4.12, dass bei zwei Ganzzahlen nur Ganzzahltypen gemeinsamer Typ sein können (sonst hätte `i32`, `ui32` keinen).
- [E] V-8.3 (angenommen am 21.11.2026): Eine Klassenmethode, die eine `nodiscard`-Anforderung erfüllt, muss `nodiscard` nicht wiederholen; ihr Ergebnis bleibt `nodiscard`.
- [E] Meilenstein M3, erster Teil: Flussanalyse für Verengung, sichere Initialisierung, Moves und Borrows sowie Warnungen; alle 242 Konformitätstests bestehen. Offen ist der Vergleich mit dem FR++-Modell.
- [E] V-12 (Kapitel 5, angenommen am 21.11.2026): Ein Zweig, der nicht normal endet, trägt zum Zustand nach `if`, `switch`, Schleife oder `try` nichts bei, wie bei der Verengung (R-6.8).
- [E] V-6.1 (Kapitel 6, angenommen am 21.11.2026): Für R-6.3, R-6.16 und R-6.18 zählt bei Variablen der deklarierte Typ; eine Verengung macht `v?.m()`, `v ?? w` und `v == null` nicht zum Fehler.
- [E] V-14.2 (Kapitel 14, angenommen am 21.11.2026): Eine Meldung darf neben der verletzten Regel begründende Regeln nennen; ein abgelehnter Test ist erfüllt, wenn eine davon die erwartete ist.
- [E] Meilenstein M3 ist abgeschlossen: Flussanalyse in `rppc`, alle Konformitätstests bestehen, und `rppc` beurteilt übersetzte FR++-Programme wie das Modell (5000 Zufallsprogramme ohne Abweichung).
- [E] V-13 (Kapitel 5, angenommen am 21.11.2026): Ein `own`-Wert an einem `weak`-Ziel ist ein Fehler (R-4.41); `weak` entsteht nur aus `shared` oder `weak`.
- [E] Meilenstein M4 ist begonnen: Debug-Interpreter `rpp-interp`, `rppc run`; alle Laufzeittests außer zweien, die eingebaute Teile der Bibliothek brauchen (M5), bestehen in beiden Modi.
- [E] V-14 (Kapitel 5, angenommen am 22.11.2026): Lesart von V-7 bei Vererbung – erst alle Destruktoren (Klasse bis Basis), dann alle Felder in umgekehrter Reihenfolge; ebenso beim Aufräumen nach einem werfenden Konstruktor.
- [E] Implementierungsplan (angenommen am 22.11.2026): Die Zwischenrepräsentation kommt erst mit JIT bzw. Releasemodus; bis dahin führt der Interpreter den geprüften AST aus.
- [E] Meilenstein M4 ist abgeschlossen: Debug-Interpreter mit Prüfmodus für die Invarianten aus Kapitel 5; alle Laufzeittests mit `print` und `rpp.core` bestehen in beiden Modi; Ausführung gleich wie im FR++-Modell bis auf einen erklärten Fall.
- [E] Meilenstein M5, erster Teil: eingebaute Teile der Standardbibliothek im Interpreter; alle 245 Konformitätstests bestehen in beiden Modi.
- [E] V-15 (Kapitel 5, angenommen am 22.11.2026): Ein `own`-Objekt einer Klasse ohne Destruktor lässt sich mit `(K k, V v) = e;` in seine Felder zerlegen (R-5.25); Moves aus einzelnen Feldern bleiben verboten (V-3).
- [E] Meilenstein M5 ist abgeschlossen: Standardbibliothek eingebaut, Typargumente zur Laufzeit, Hash-Index; alle 255 Konformitätstests bestehen in beiden Modi.
- [E] V-14.3 (Kapitel 14, angenommen am 22.11.2026): Laufzeittests laufen in einem frischen, leeren Arbeitsverzeichnis, das sie beschreiben dürfen.
- [E] Meilenstein M6 ist begonnen: Der Compiler in R++ entsteht in `rpp-compiler/`, Schritt für Schritt und jeweils gegen `rppc` verglichen; erster Teil ist der Lexer.
- [E] M6: Der Parser in R++ baut einen einheitlichen Syntaxbaum (Art, Text, Position, Kinder) statt einer Klasse je Knotenart; verglichen wird über eine gemeinsame Textform.
- [E] M6: Das Laden von Projekten ist nach R++ übertragen und gleichwertig zu `rppc`; falsche Formen in `rpp.yaml` (keine Liste, kein Text) sind Fehler nach R-11.13.
- [E] V-13.8 (Kapitel 13, angenommen am 22.11.2026): `File::exists(path)` und `Path::absolute(p)` in `rpp.io`.
- [E] M6: Die Deklarationsphase ist nach R++ übertragen; Typen sind dort unveränderliche, geteilte Werte (`shared Ty`).
- [E] M6: Die Prüfung der Rümpfe ist nach R++ übertragen; der Prüfer besitzt das Programm und leiht die Syntaxbäume. Die Angaben für die Ausführung folgen mit dem Codegenerator.
- [E] Schlüsselwort `mut` (angenommen am 22.11.2026): Veränderbarkeit wird wie in Rust mit `mut` markiert statt mit `&`, das zugleich bitweises Und und Verbindung von Constraints war. `mut` steht vor dem Typ (`mut Foo f`, `Func<mut Foo, void>`, `ref mut Foo g`, `ref mut V at(K k)`), vor dem Ausdruck (`rename(mut foo)`, `= mut x`, `return mut …;`, `for (mut Foo f : mut list)`) und nach der Parameterliste verändernder Methoden (`void clear() mut`). `mut` ist ein reserviertes Wort. `&` bedeutet nur noch bitweises Und und verbindet Constraints (`T : A & B`).
