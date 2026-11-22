# R++ Typen (Kapitel 4)

Stand: 21.11.2026, Entwurf 0.3. Dieses Dokument füllt Kapitel 4 des „R++ Spezifikationsgerüsts“. Es legt fest, welche Typen es gibt, welche Werte sie haben, wie sie ineinander umgewandelt werden und welche Operatoren für sie gelten. Wie Typen mit Besitz und Borrows zusammenspielen, regelt Kapitel 5 („R++ Ownership-Kernkalkül“); wie nullbare Werte geprüft werden, Kapitel 6.

Jede normative Regel trägt eine Nummer `R-4.n`. Die Konformitätstests liegen in `konformitaet/kap04_typen/`.

## 0. Neue Festlegungen in diesem Kapitel

Die meisten Regeln folgen aus den „R++ Designentscheidungen“ (Abschnitte 2, 4, 6–11 und 18). Beim Ausformulieren mussten einige kleinere Punkte zusätzlich festgelegt werden. Sie sind am 20.11.2026 alle so entschieden worden (siehe „R++ Designentscheidungen“, Abschnitt 18):

| Nr. | Festlegung |
|---|---|
| V-4.0 | Korrektur: `i32 + ui32` hat nach der Regel „gemeinsamer Typ“ (R-4.12) den Typ `i64` und ist kein Fehler. Ein Fehler ist es nur, wenn kein gemeinsamer Typ existiert, z. B. `i64 + ui64` oder `i64 + f64`. |
| V-4.1 | `char` hat Vergleiche, aber keine Arithmetik. `cast<char>(n)` mit einem Wert, der kein Unicode-Skalarwert ist, wirft in **beiden** Modi eine Exception, weil ein ungültiges `char` UTF-8-Strings verderben würde. |
| V-4.2 | Schiebeoperatoren: Eine Schiebeweite außerhalb von `0` bis Bitbreite − 1 wirft im Debugmodus eine Exception; im Releasemodus wird sie modulo Bitbreite genommen. `>>` schiebt bei vorzeichenbehafteten Typen arithmetisch, sonst logisch. |
| V-4.3 | Bitoperatoren (`&`, `\|`, `^`, `~`, `<<`, `>>`) gibt es nur für Ganzzahlen und `byte`. Für `bool` gelten nur `!`, `&&`, `\|\|`, `==` und `!=`. |
| V-4.4 | Strings sind unveränderlich. `+` verbindet zwei Strings; Zahlen werden nicht automatisch zu Strings. `==` und `!=` vergleichen den Inhalt, `<`, `<=`, `>`, `>=` lexikografisch nach Unicode-Skalarwerten. |
| V-4.5 | Gleichheit von Objekten über das generische Bibliotheks-Mixin `Equatable<T>` mit der Anforderung `bool equals(T other)`. Primitive Typen, `string` und Enums ohne Daten gelten als `Equatable`. Identität prüft die eingebaute Funktion `same(a, b)`. |
| V-4.6 | `e as C`: `C` muss echter Untertyp des statischen Typs von `e` sein, sonst Compilerfehler. Das Ergebnis ist ein Borrow `C?`; ist `e` ein `shared`-Wert und das Ziel ausdrücklich `shared`, ist es `shared C?` (Zähler + 1). Einen Downcast mit Besitzübergabe gibt es in Version 1 nicht. |
| V-4.7 | Enum-Varianten ohne Wert bekommen den Wert der vorigen Variante + 1, die erste den Wert 0. |
| V-4.8 | Tupel, Arrays und Funktionstypen haben kein `==`. Tupel-Literale übernehmen die Elementtypen aus dem Kontext; sonst müssen Tupeltypen genau übereinstimmen. |
| V-4.9 | Zwischen festen Arrays und Vektoren gibt es keine automatische Umwandlung; beide werden nur automatisch als `Slice<T>` geliehen. Die Größe eines festen Arrays ist ≥ 0. |
| V-4.10 | `cast` von Ganzzahl zu Gleitkomma und von `f64` zu `f32` rundet zum nächsten darstellbaren Wert (bei Gleichstand zur geraden Mantisse) und wirft nie; zu große Werte werden ±∞. |
| V-4.11 | `null` braucht einen nullbaren Kontext: `var x = null;` ist ein Fehler. `T??` und `void?` gibt es nicht. |
| V-4.13 | (angenommen am 21.11.2026) Klarstellung zu R-4.12: Sind beide Operanden Ganzzahlen, kommen als gemeinsamer Typ nur Ganzzahltypen in Frage. Ohne diesen Satz hätte `i32`, `ui32` keinen gemeinsamen Typ, weil `i64` und `f64` beide passen und keiner in den anderen umgewandelt werden kann; die Tabelle in R-4.12 nennt aber `i64`. |
| V-4.12 | (angenommen am 21.11.2026) Ein Enum-Wert entsteht ohne `new`: `Color::Red` für eine Variante ohne Daten, `Shape::Circle(2.0)` für eine mit Daten (R-4.25). |

## 1. Übersicht

| Typart | Beispiele | Abschnitt |
|---|---|---|
| Wahrheitswert | `bool` | 2 |
| Ganzzahlen | `i8` … `i64`, `ui8` … `ui64` | 2 |
| Gleitkomma | `f32`, `f64` | 2 |
| Zeichen, Rohdaten | `char`, `byte` | 2 |
| Zeichenkette | `string` | 5 |
| Klassen und Mixins | `Foo`, `Printable`, `List<Foo>` | 6 |
| Enums | `Color`, `Shape` | 7 |
| Feste Arrays, Vektoren, Slices | `i32[16]`, `Foo[]`, `Slice<i32>` | 8 |
| Tupel | `(i32, string)` | 9 |
| Funktionstypen | `Func<i32, bool>` | 10 |
| Nullbare Typen | `Foo?`, `i32?` | 11 |
| Typ-Parameter | `T`, `T : Comparable` | 12 |
| `void` | nur als Rückgabetyp und letztes Argument von `Func` | 13 |

Vor Klassen-, Array-, Tupel- und Funktionstypen kann eine Besitzform stehen (`own`, `shared`, `weak`) oder an Parametern `mut`; deren Bedeutung regelt Kapitel 5. Für die Typregeln dieses Kapitels zählt nur der Typ ohne Besitzform, außer wo es ausdrücklich gesagt ist.

## 2. Primitive Typen

**R-4.1 (Wertebereiche).**

| Typ | Werte |
|---|---|
| `bool` | `true`, `false` |
| `i8`, `i16`, `i32`, `i64` | Zweierkomplement mit 8, 16, 32, 64 Bit |
| `ui8`, `ui16`, `ui32`, `ui64` | 0 bis 2ⁿ − 1 mit n = 8, 16, 32, 64 |
| `f32`, `f64` | IEEE 754 binary32 bzw. binary64, einschließlich ±0, ±∞ und NaN |
| `char` | Unicode-Skalarwerte: 0 bis 0x10FFFF ohne 0xD800 bis 0xDFFF |
| `byte` | 8 Bit Rohdaten |

**R-4.2 (Kopie).** Werte primitiver Typen werden bei Zuweisung, Übergabe und Rückgabe kopiert. Sie haben keine Besitzform.

**R-4.3 (bool).** Bedingungen von `if`, `while`, `do`-`while`, `for` und `? :` sowie die Operanden von `!`, `&&` und `||` müssen den Typ `bool` haben. Zwischen `bool` und anderen Typen gibt es keine Umwandlung, auch nicht mit `cast`.

**R-4.4 (char, V-4.1).** Für `char` sind `==`, `!=`, `<`, `<=`, `>` und `>=` definiert (nach Skalarwert), aber keine Arithmetik. `cast<T>(c)` mit einem Ganzzahltyp `T` liefert den Skalarwert (mit Bereichsprüfung nach R-4.9). `cast<char>(n)` wirft in beiden Modi eine Exception, wenn `n` kein Skalarwert ist.

**R-4.5 (byte).** `byte` ist ein eigener Typ für Rohdaten. Erlaubt sind `==`, `!=`, `<`, `<=`, `>`, `>=` und die Bitoperatoren (V-4.3); Arithmetik ist ein Fehler. `cast<ui8>(b)` und `cast<byte>(u)` mit `u : ui8` wandeln verlustfrei um und werfen nie. Andere Umwandlungen von und zu `byte` gibt es nicht.

## 3. Umwandlungen zwischen Zahltypen

**R-4.6 (automatische Umwandlung).** Ein Zahlwert wird automatisch in einen anderen Zahltyp umgewandelt, wenn jeder Wert des Quelltyps im Zieltyp exakt darstellbar ist. Das ergibt genau diese Umwandlungen:

| Von | Automatisch nach |
|---|---|
| `i8` | `i16`, `i32`, `i64`, `f32`, `f64` |
| `i16` | `i32`, `i64`, `f32`, `f64` |
| `i32` | `i64`, `f64` |
| `i64` | – |
| `ui8` | `ui16`, `ui32`, `ui64`, `i16`, `i32`, `i64`, `f32`, `f64` |
| `ui16` | `ui32`, `ui64`, `i32`, `i64`, `f32`, `f64` |
| `ui32` | `ui64`, `i64`, `f64` |
| `ui64` | – |
| `f32` | `f64` |
| `f64` | – |

`bool`, `char` und `byte` werden nie automatisch umgewandelt. Die automatische Umwandlung gilt bei Zuweisung, Initialisierung, Argumentübergabe, Rückgabe und für Operanden (R-4.12).

**R-4.7 (Literale).** Ein Zahlliteral hat den Typ, den der Kontext verlangt. Der Compiler prüft, ob der Wert passt (`i8 b = 300;` ist ein Fehler; `f32 x = 0.1;` ist erlaubt und rundet). Ohne Kontext ist ein Ganzzahlliteral `i32`, bei zu großem Wert `i64`, und ein Gleitkommaliteral `f64`. Gleitkommaliterale dürfen einen Exponenten haben (`6.022e23`, `2.5E-3`, siehe Kapitel 2); ist der Wert im Zieltyp nicht endlich darstellbar (`f32 x = 1.0e39;`), ist das ein Fehler. Steht ein Literal als Operand neben einem Wert eines anderen Zahltyps, ist dieser Typ der Kontext (`x + 1` mit `x : i8` rechnet in `i8`).

**R-4.8 (explizite Umwandlung).** `cast<T>(e)` wandelt zwischen Zahltypen in jede Richtung um, außerdem zwischen `char` und Ganzzahlen (R-4.4), `byte` und `ui8` (R-4.5) sowie Enums mit Zahlwerten und Ganzzahlen (R-4.27).

**R-4.9 (cast zwischen Ganzzahlen).** Passt der Wert in den Zieltyp, bleibt er erhalten. Sonst: Exception im Debugmodus, im Releasemodus werden die unteren Bits behalten (Zweierkomplement).

**R-4.10 (cast von Gleitkomma zu Ganzzahl).** Die Nachkommastellen werden abgeschnitten (Rundung zur Null). Liegt das Ergebnis außerhalb des Zieltyps oder ist der Wert NaN: Exception im Debugmodus; im Releasemodus Sättigung auf den größten bzw. kleinsten Wert des Zieltyps, NaN wird 0.

**R-4.11 (cast zu Gleitkomma, V-4.10).** Ganzzahl → Gleitkomma und `f64` → `f32` runden zum nächsten darstellbaren Wert, bei Gleichstand zur geraden Mantisse. Zu große Werte werden ±∞. Diese Umwandlungen werfen nie.

## 4. Operatoren auf primitiven Typen

**R-4.12 (gemeinsamer Typ, V-4.0).** Für zweistellige arithmetische Operatoren und Vergleiche werden beide Operanden in ihren **gemeinsamen Typ** umgewandelt: den Typ `T`, in den beide automatisch umgewandelt werden können (R-4.6) und der selbst in jeden anderen solchen Typ umgewandelt werden kann; sind beide Operanden Ganzzahlen, kommen nur Ganzzahltypen in Frage (V-4.13). Gibt es keinen, ist der Ausdruck ein Fehler und verlangt einen `cast`. Beispiele:

| Operanden | Gemeinsamer Typ |
|---|---|
| `i8`, `i32` | `i32` |
| `i8`, `ui8` | `i16` |
| `i32`, `ui32` | `i64` |
| `i32`, `f32` | `f64` |
| `i64`, `ui64` | keiner – Fehler |
| `i64`, `f64` | keiner – Fehler |
| `ui64`, `f32` | keiner – Fehler |

Dadurch werden auch gemischte Vergleiche mathematisch richtig ausgewertet: `cast<i32>(-1) < cast<ui32>(3_000_000_000)` ist `true`.

**R-4.13 (Arithmetik).** `+`, `-`, `*`, `/`, `%` und die einstelligen `-`, `++`, `--` gibt es für Ganzzahl- und Gleitkommatypen. Das Ergebnis hat den gemeinsamen Typ. Einstelliges `-` auf einem vorzeichenlosen Typ ist ein Fehler.

**R-4.14 (Überlauf).** Liegt das mathematische Ergebnis einer Ganzzahloperation außerhalb des Ergebnistyps, wirft der Debugmodus eine Exception; der Releasemodus rechnet modulo 2ⁿ weiter. Das gilt auch für `i32::MIN / -1`.

**R-4.15 (Division).** Ganzzahldivision rundet zur Null; `a % b` hat das Vorzeichen von `a`, und es gilt `(a / b) * b + a % b == a`. Ganzzahldivision und `%` durch 0 werfen in beiden Modi eine Exception. Gleitkommaoperationen folgen IEEE 754 (Division durch 0 ergibt ±∞ oder NaN).

**R-4.16 (Bitoperatoren, V-4.2, V-4.3).** `&`, `|`, `^`, `~` gibt es für Ganzzahltypen (gemeinsamer Typ) und für `byte`. `<<` und `>>` haben den Typ des linken Operanden; der rechte ist eine Ganzzahl. Liegt die Schiebeweite außerhalb von 0 bis Bitbreite − 1: Exception im Debugmodus, im Releasemodus wird sie modulo Bitbreite genommen. `>>` ist bei vorzeichenbehafteten Typen arithmetisch, sonst logisch.

**R-4.17 (Vergleiche).** `==`, `!=`, `<`, `<=`, `>`, `>=` gibt es für Zahltypen (gemeinsamer Typ), `char` und `byte`; `==` und `!=` zusätzlich für `bool`. Das Ergebnis ist `bool`. Vergleiche mit NaN sind außer `!=` immer `false`.

## 5. string

**R-4.18 (Werte).** Ein `string` ist eine unveränderliche Folge von Unicode-Skalarwerten, intern UTF-8. Strings sind implizit geteilt und nehmen nicht an Besitz und Borrows teil (Kapitel 5); Zuweisung und Übergabe kopieren nur den Verweis.

**R-4.19 (Zugriff).**

| Ausdruck | Typ | Bedeutung |
|---|---|---|
| `s.length` | `i64` | Anzahl der Zeichen (Skalarwerte) |
| `s[i]` | `char` | i-tes Zeichen, ab 0 |
| `s.byteLength` | `i64` | Anzahl der Bytes der UTF-8-Darstellung |
| `s.byteAt(i)` | `byte` | i-tes Byte, ab 0 |
| `for (char c : s)` | | Iteration über die Zeichen |

Indizes außerhalb des Bereichs werfen eine Exception. `s[i] = c` ist ein Fehler, weil Strings unveränderlich sind. `s.length` und `s[i]` können bei UTF-8 Zeit proportional zur Länge brauchen; eine Implementierung darf dafür Hilfsdaten zwischenspeichern.

**R-4.20 (Operatoren, V-4.4).** `a + b` verbindet zwei Strings. Andere Operanden werden nicht automatisch umgewandelt (`"n=" + 3` ist ein Fehler). `==` und `!=` vergleichen den Inhalt; `<`, `<=`, `>`, `>=` vergleichen lexikografisch nach Skalarwerten.

## 6. Klassen und Mixins

**R-4.21 (Nominale Typen).** Jede Klasse und jedes Mixin ist ein eigener Typ. Zwei Klassentypen sind nur gleich, wenn sie dieselbe Deklaration und dieselben Typargumente haben.

**R-4.22 (Untertypen).** `C <: D` gilt, wenn
- `C = D`, oder
- `C` direkt von `D` erbt (`class C : D`), oder
- `C` das Mixin `D` importiert (`import D;`), oder
- es ein `E` gibt mit `C <: E` und `E <: D`.

Ein Wert vom Typ `C` darf überall stehen, wo `D` verlangt wird; das gilt mit derselben Besitzform (Kapitel 5, Abschnitt 4.4). Wird dabei ein `own`-Objekt übergeben, wird es verschoben.

**R-4.23 (Downcast, V-4.6).** `e as C` prüft zur Laufzeit, ob das Objekt, das `e` bezeichnet, den Typ `C` hat (oder einen Untertyp davon).
- `C` ist ein Klassen- oder Mixin-Typ ohne `?` und echter Untertyp des statischen Typs von `e`; sonst ist der Ausdruck ein Fehler.
- Ist `e` ein Ort, ist das Ergebnis ein Borrow vom Typ `C?`: `ref Circle? c = s as Circle;`. Er gilt als Borrow auf `e` im Sinne von Kapitel 5.
- Ist `e` vom Typ `shared D` und das Ziel ausdrücklich `shared`, ist das Ergebnis `shared C?` und erhöht den Zähler: `shared Circle? c = s as Circle;`.
- Passt der Typ nicht, ist das Ergebnis `null`. `as` wirft nie.
- Einen Downcast, der Besitz übergibt (`own Base` → `own Derived`), gibt es in Version 1 nicht.

**R-4.24 (Gleichheit von Objekten, V-4.5).** `a == b` und `a != b` sind für Objekte nur erlaubt, wenn die Klasse von `a` das Bibliotheks-Mixin `Equatable<T>` importiert und `b` in `T` umgewandelt werden kann. `a == b` bedeutet dann `a.equals(b)`, `a != b` bedeutet `!a.equals(b)`. Vergleiche eines nullbaren Werts mit `null` sind immer erlaubt (Kapitel 6). Die eingebaute Funktion `same(a, b)` liefert `true`, wenn `a` und `b` dasselbe Objekt bezeichnen; sie nimmt zwei Borrows auf Objekte.

## 7. Enums

**R-4.25 (Varianten).** Ein Enum ist ein nominaler Typ mit einer festen Menge von Varianten. Eine Variante trägt entweder Daten (`Circle(f64 radius)`) oder einen festen Zahlwert (`Red = 1`) oder nichts. Ein Enum mit Zahlwerten hat keine Varianten mit Daten. Ein Wert entsteht ohne `new` als `E::V` oder, mit Daten, als `E::V(a₁, …, aₙ)`; die Argumente werden wie bei einem Konstruktor übergeben (V-4.12).

**R-4.26 (Zahlwerte, V-4.7).** Zahlwerte sind Ganzzahlliterale, auch negativ, im Bereich von `i32` und innerhalb eines Enums eindeutig. Eine Variante ohne Wert in einem Enum ohne Daten hat den Wert der vorigen Variante + 1, die erste den Wert 0.

**R-4.27 (Umwandlung).** Enums werden nie automatisch umgewandelt. Für ein Enum ohne Daten liefert `cast<T>(e)` mit einem Ganzzahltyp `T` den Zahlwert der Variante (Bereichsprüfung nach R-4.9). `cast<E>(n)` liefert die Variante mit dem Wert `n` und wirft in beiden Modi eine Exception, wenn es keine gibt. Für Enums mit Daten gibt es keine Umwandlung.

**R-4.28 (Gleichheit).** `==` und `!=` sind für Enums ohne Daten erlaubt und vergleichen die Variante. Enums mit Daten vergleicht man mit `switch`.

**R-4.29 (Besitz).** Ein Enum-Wert besitzt seine Daten. Enums ohne Daten werden wie primitive Werte kopiert; Enums mit Daten verhalten sich wie Objekte (Kapitel 5).

## 8. Feste Arrays, Vektoren und Slices

**R-4.30 (Typen).** `T[N]` ist ein festes Array mit `N` Elementen; `N` ist ein Ganzzahlliteral oder eine `static const`-Ganzzahlkonstante ≥ 0. `T[]` ist ein Vektor. `Slice<T>` ist eine geliehene Sicht auf einen Ausschnitt eines festen Arrays oder Vektors. Alle Elemente haben den Typ `T`. `T[N]` und `T[M]` sind nur bei `N = M` derselbe Typ.

**R-4.31 (Umwandlungen, V-4.9).** `T[N]` und `T[]` werden automatisch als `Slice<T>` geliehen (lesend; mit `mut` veränderbar). Zwischen `T[N]` und `T[]` gibt es keine automatische Umwandlung. Arrays sind invariant: `Circle[]` ist kein `Shape[]`.

**R-4.32 (Literale).** Ein Array-Literal `[a, b, c]` hat den Typ, den der Kontext verlangt; jedes Element wird in den Elementtyp umgewandelt. Bei festen Arrays muss die Anzahl stimmen. `[]` braucht einen Kontext.

**R-4.33 (Länge und Index).** `.length` hat den Typ `i64`. Indizes sind Ganzzahlen, die automatisch in `i64` umgewandelt werden können; ein Index außerhalb von `0` bis `length − 1` wirft eine Exception. Arrays haben kein `==` (V-4.8).

## 9. Tupel

**R-4.34 (Typen).** `(T₁, …, Tₙ)` mit n ≥ 2 ist ein Tupeltyp. Zwei Tupeltypen sind gleich, wenn sie gleich viele Elemente mit jeweils gleichen Typen haben. Es gibt keine Untertypen zwischen Tupeln.

**R-4.35 (Verwendung).** Tupel entstehen als Rückgabewert (`(i32, string) f()`) oder als Literal (`(1, "a")`). Ein Tupel-Literal übernimmt die Elementtypen aus dem Kontext (V-4.8). Ein Tupel wird nur zerlegt: `(i32 a, string b) = t;` oder `(a, b) = t;`. Einen Elementzugriff wie `t.0` gibt es in Version 1 nicht, ebenso kein `==`.

**R-4.36 (Besitz).** Ein Tupel besitzt seine Elemente. Beim Zerlegen gehen sie an die neuen Variablen über. Mit derselben Schreibweise lässt sich ein `own`-Objekt einer Klasse ohne Destruktor in seine Felder zerlegen (Kapitel 5, R-5.25).

## 10. Funktionstypen

**R-4.37 (Func).** `Func<P₁, …, Pₙ, R>` ist der Typ einer Funktion mit Parametern `P₁ … Pₙ` und Rückgabetyp `R`; `Func<R>` hat keine Parameter. `void` darf nur an letzter Stelle stehen. Parameter können wie Methodenparameter markiert sein (`Func<mut Foo, void>`, `Func<own Foo, void>`; Kapitel 10, R-10.13). Funktionstypen sind strukturell und invariant: `Func<A, B>` und `Func<C, D>` sind nur bei `A = C` und `B = D` gleich.

**R-4.38 (Werte).** Werte eines Funktionstyps entstehen nur aus Lambdas. Ein Parameter vom Typ `Func<…>` nimmt normale und `own`-Lambdas; Variablen, Felder und Rückgabewerte nehmen nur `own`-Lambdas (Kapitel 5, Abschnitt 7). Funktionstypen haben kein `==`.

## 11. Nullbare Typen

**R-4.39 (Bildung, V-4.11).** Zu jedem Typ `T` außer `void` gibt es den nullbaren Typ `T?`; er enthält alle Werte von `T` und `null`. `T??` gibt es nicht. Das gilt auch für primitive Typen (`i32?`), Tupel (`(i32, string)?`), Arrays (`Foo[]?`) und mit Besitzformen (`shared Foo?`, `weak Foo?`). `Foo?[]` ist ein Vektor nullbarer Elemente, `Foo[]?` ein nullbarer Vektor.

**R-4.40 (Umwandlung).** `T` wird automatisch in `T?` umgewandelt; die Umwandlung zurück geht nur über die Prüfungen aus Kapitel 6. Das Literal `null` braucht einen Kontext mit nullbarem Typ; `var x = null;` ist ein Fehler.

**R-4.41 (weak).** `weak Foo` ist ein Verweis, der nie `null` ist, dessen Ziel aber freigegeben sein kann. `w.lock()` liefert deshalb immer `shared Foo?`. Ein `weak`-Wert entsteht nur aus `shared` oder `weak` (Kapitel 5, V-6, V-13); `weak Foo w = new Foo();` ist ein Fehler.

## 12. Typ-Parameter

**R-4.42 (Constraints).** Ein Typ-Parameter `T` steht für einen beliebigen Typ außer `void`, auch für primitive Typen. Mit Constraints `T : M₁ & … & Mₙ` muss das Typargument Untertyp jedes `Mᵢ` sein. Im Rumpf sind auf `T` nur die Operationen erlaubt, die die Constraints zusagen; `==` verlangt `T : Equatable<T>`. Primitive Typen, `string` und Enums ohne Daten erfüllen `Equatable` von selbst.

**R-4.43 (Invarianz).** Generische Typen sind invariant: `List<Circle>` ist kein Untertyp von `List<Shape>`, auch wenn `Circle <: Shape`.

**R-4.44 (Ableitung).** Bei generischen Methoden werden die Typargumente nur aus den Argumenten abgeleitet. Lässt sich ein Typ-Parameter nicht eindeutig ableiten, ist das ein Fehler.

## 13. void

**R-4.45.** `void` ist nur als Rückgabetyp einer Methode und als letztes Typargument von `Func` erlaubt. Es gibt keine Werte und keine Variablen vom Typ `void`.

## 14. Initialisierung und Typinferenz

**R-4.46 (Sichere Initialisierung).** Eine lokale Variable muss auf jedem Weg zugewiesen sein, bevor sie gelesen wird. Es gibt keine Standardwerte; `i32 x; print(x);` ist ein Fehler.

**R-4.47 (Felder).** Ein nullbares Feld ohne Initialisierer ist `null`. Jedes andere Feld ohne Initialisierer muss im Konstruktor gesetzt werden, bevor `this` anders verwendet wird (V-10 aus Kapitel 5).

**R-4.48 (var).** `var x = e;` gibt `x` den Typ von `e`, einschließlich Besitzform: `var f = new Foo();` ist `own Foo`, `var s = t;` mit `t : shared Foo` ist `shared Foo`. Ein Literal ohne weiteren Kontext bekommt den Standardtyp aus R-4.7. `var` verlangt einen Initialisierer.

## 15. Konformitätstests

Die Testprogramme liegen in `konformitaet/kap04_typen/`. Die Syntax prüft `rpp-grammatik/tools/run_tests.py`, die Semantik `rppc test` (erste Implementierung).

| Regeln | Akzeptiert | Abgelehnt |
|---|---|---|
| R-4.3 bool | `a01_primitive.rpp` | `n01_int_als_bedingung.rpp`, `n02_cast_zu_bool.rpp` |
| R-4.4, R-4.5 char, byte | `a01_primitive.rpp` | `n03_arithmetik_mit_byte.rpp`, `n04_arithmetik_mit_char.rpp` |
| R-4.6, R-4.7 Umwandlung, Literale | `a02_umwandlungen.rpp` | `n05_i64_nach_f64.rpp`, `n06_literal_passt_nicht.rpp` |
| R-4.12 gemeinsamer Typ | `a02_umwandlungen.rpp` | `n07_kein_gemeinsamer_typ.rpp` |
| R-4.13 Arithmetik | – | `n08_minus_auf_vorzeichenlos.rpp` |
| R-4.19, R-4.20 string | `a03_string.rpp` | `n09_string_zuweisung_an_index.rpp`, `n10_string_plus_zahl.rpp` |
| R-4.23, R-4.24 as, Gleichheit | `a04_klassen.rpp` | `n11_as_kein_untertyp.rpp`, `n12_gleichheit_ohne_equatable.rpp` |
| R-4.26 – R-4.28 Enums | `a05_enums.rpp` | `n13_enum_wert_doppelt.rpp`, `n14_enum_mit_daten_vergleichen.rpp` |
| R-4.31 Arrays | `a06_arrays_tupel.rpp` | `n15_array_kovarianz.rpp` |
| R-4.35 Tupel | `a06_arrays_tupel.rpp` | `n16_tupel_vergleich.rpp` |
| R-4.40 null | `a07_nullbar.rpp` | `n17_var_null.rpp` |
| R-4.43 Invarianz | – | `n18_generics_kovarianz.rpp` |
| R-4.46 Initialisierung | `a07_nullbar.rpp` | `n19_lesen_vor_zuweisung.rpp` |
| R-4.45 void | – | `n20_void_nicht_am_ende.rpp` |
