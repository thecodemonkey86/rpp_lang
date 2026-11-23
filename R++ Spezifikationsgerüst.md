# R++ Spezifikationsgerüst

Stand: 22.11.2026, Entwurf 0.47. Grundlage ist das Dokument „R++ Designentscheidungen“. Dieses Gerüst legt die Kapitelstruktur der Sprachspezifikation fest. Die Grammatik liegt als ausführbare ANTLR4-Datei im Projektdokument „RPP.g4“ vor und ist mit einem Testlauf geprüft (siehe Abschnitt 3.4).

## Konventionen

- Jede normative Regel bekommt eine Nummer der Form `R-<Kapitel>.<laufende Nummer>`, z. B. `R-5.3`.
- Zu jeder Regel gehören Testprogramme in der Konformitäts-Testsuite: mindestens eines, das akzeptiert, und eines, das abgelehnt werden muss.
- Der Compiler meldet Verstöße als **Fehler** (keine Übersetzung). Wo die Spezifikation es verlangt, meldet er außerdem **Warnungen**; sie verhindern die Übersetzung nicht (Kapitel 8, R-8.1).
- Die Grammatik steht in ANTLR4-Notation: `:` Definition, `|` Alternative, `?` optional, `*` beliebig oft, `+` mindestens einmal, `'...'` Terminal.

## Kapitelübersicht

| Nr. | Kapitel | Inhalt | Stand |
|---|---|---|---|
| 1 | Einleitung | Ziele, Begriffe, Konformität | Entwurf 0.1 („R++ Einleitung“) |
| 2 | Lexikalische Struktur | Zeichen, Bezeichner, Keywords, Literale, Kommentare | Entwurf, getestet |
| 3 | Grammatik | Kontextfreie Syntax | ausführbar, getestet |
| 4 | Typen | Basistypen, Klassen-, Array-, Tupel-, Funktions-, nullbare Typen | Entwurf 0.2 („R++ Typen“), Regeln R-4.1 bis R-4.48, V-4.0 bis V-4.11 entschieden |
| 5 | Ownership und Borrowing | Besitzformen, Move, Borrow, Exklusivität; als Kernkalkül mit Inferenzregeln | Entwurf 0.5 („R++ Ownership-Kernkalkül“), Borrow-Rückgabe von `this` (R-5.23, R-5.24) ergänzt, V-1 bis V-11 entschieden, ausführbares Modell in `fr-modell/` |
| 6 | Null-Sicherheit | Nullbare Typen, Prüfung, Typverengung | Entwurf 0.1 („R++ Null-Sicherheit“), Regeln R-6.1 bis R-6.23, alles entschieden |
| 7 | Klassen, Mixins, Typ-Parameter | Member, Vererbung, Import, Constraints | Entwurf 0.1 („R++ Klassen und Generics“), Regeln R-7.1 bis R-7.28, alles entschieden |
| 8 | Anweisungen und Ausdrücke | Semantik, Auswertungsreihenfolge, Operatoren | Entwurf 0.1 („R++ Anweisungen und Ausdrücke“), Regeln R-8.1 bis R-8.28, alles entschieden |
| 9 | Exceptions | Werfen, Fangen, Unwinding | Entwurf 0.1 („R++ Exceptions“), Regeln R-9.1 bis R-9.19, alles entschieden |
| 10 | Lambdas | Erfassen per Borrow und per Wert, Funktionstypen | Entwurf 0.1 („R++ Lambdas“), Regeln R-10.1 bis R-10.18, alles entschieden |
| 11 | Pakete und Projekte | `package`, `using`, `export`, Projektdatei, Abhängigkeiten, zentrales Verzeichnis, bedingte Kompilierung | Entwurf 0.1 („R++ Pakete und Projekte“), Regeln R-11.1 bis R-11.25 |
| 12 | Ausführungsmodell | Debug- und Releasemodus, zulässige Unterschiede | Entwurf 0.1 („R++ Ausführungsmodell“), Regeln R-12.1 bis R-12.12 |
| 13 | Standardbibliothek | Mindestumfang: `string`, Arrays, Exceptions, `Func` | Entwurf 0.8 („R++ Standardbibliothek“), erster Schnitt für Selbstübersetzung, alles entschieden; Ausbau mit `rpp.random`, `Vectors`, `rpp.math`, `Text`, Datei-Operationen, `Deque`/`getOrPut` (Abschnitte 9–15) |
| 14 | Konformitätstests | Aufbau der Testsuite, Zuordnung zu Regeln | Entwurf 0.2 („R++ Konformitätstests“), alle Regeln genannt oder begründet |

## 2. Lexikalische Struktur

- Quelltext ist UTF-8.
- Kommentare: `// bis Zeilenende` und `/* ... */`.
- Bezeichner: Buchstabe oder `_`, gefolgt von Buchstaben, Ziffern oder `_`.
- Gleitkommazahlen brauchen eine Ziffer vor und nach dem Punkt (`0.5`, nicht `.5`). Optional folgt ein Exponent: `e` oder `E`, ein Vorzeichen und Ziffern (`1.0e10`, `2.5E-3`). Ohne Punkt gibt es keinen Exponenten: `1e10` ist ein Syntaxfehler.
- Ganzzahlen dezimal oder hexadezimal (`0xFF`).
- Ziffern dürfen mit `_` getrennt werden (`1_000_000`, `0xFF_FF`), aber nicht am Anfang oder Ende einer Ziffernfolge. Es gibt keine Typ-Suffixe.
- `..` ist ein eigenes Token für Bereiche. `1..3` wird als `1`, `..`, `3` gelesen, nicht als Gleitkommazahl.
- Es gibt keine Tokens `<<` und `>>`. Schiebeoperatoren bestehen aus zwei aufeinanderfolgenden `<` bzw. `>`, damit `List<List<i32>>` lesbar bleibt.

Reservierte Wörter:

```
abstract as base bool break byte case cast catch char class const
constructor continue copy default destructor discard do else enum export f32 f64 false
finally for i8 i16 i32 i64 if import mixin new nodiscard null override own package
private protected public ref return shared static string switch this throw
true try ui8 ui16 ui32 ui64 using var virtual void weak while
```

`main` ist nicht reserviert und darf auch als Name verwendet werden.

Bedingte Kompilierung: Eine Zeile, deren erstes Zeichen nach Leerraum ein `#` ist, ist eine Direktive (`#if Bedingung`, `#elif Bedingung`, `#else`, `#endif`). Bedingungen bestehen aus Symbolen, `!`, `&&`, `||` und Klammern; `&&` bindet stärker als `||`. Direktiven werden vor dem Parsen ausgewertet; die Zeilen inaktiver Zweige werden übersprungen und nicht geprüft. Jede andere Direktive ist ein Fehler, auch `#define`.

## 3. Grammatik

Normativ ist das Projektdokument „RPP.g4“. Dieser Abschnitt hält fest, was über die Designentscheidungen hinaus beim Umsetzen festgelegt wurde.

### 3.1 Festlegungen aus der Umsetzung

1. **Ausdrucksanweisungen.** Als Anweisung sind nur drei Formen erlaubt: Zuweisung, `++`/`--` und ein Ausdruck, der mit einem Aufruf endet. `a + b;`, `foo;` und `new Foo();` sind Fehler. Zuweisungen gibt es nur als Anweisung und im `for`-Kopf, nicht innerhalb von Ausdrücken (`x = y = 0;` ist ein Syntaxfehler); `++`/`--` dürfen auch in Ausdrücken stehen.
2. **Zuweisungsziel.** Links von `=` steht ein Name, Feldzugriff, Indexzugriff oder Tupel, kein Rechenausdruck.
3. **Lambdas.** Ein Lambda steht auf Ausdrucksebene; sein Rumpf reicht bis zum Ende des umgebenden Ausdrucks. Als Operand oder für einen direkten Aufruf muss es geklammert werden: `((i32 a) => a)(3)`.
4. **Case-Marken.** Erlaubt sind Literale (auch negativ), Namen von Konstanten oder Enum-Varianten und Varianten mit Bindungen wie `Circle(r)`. Beliebige Ausdrücke sind nicht erlaubt.
5. **Eine Variable pro Deklaration.** `i32 a, b;` gibt es nicht. Mehrere Variablen werden per Tupel-Deklaration eingeführt, auch im `for`-Kopf: `for ((i32 i, i32 j) = (0, n); i < j; i++, j--)`.
6. **`else`** gehört zum nächstgelegenen `if`.
7. **Enums** dürfen nach der letzten Variante ein Komma haben.
8. **Kopie.** `copy(a)` ist ein eigener Ausdruck; `a.copy()` ist ein Fehler.
9. **Statischer Zugriff mit `::`.** Statische Methoden, Konstanten und Enum-Varianten werden mit `::` angesprochen: `Math::max(a, b)`, `Shape::PI`, `Color::Red`, `List<Foo>::create()`, `my.pkg.Registry<Foo>::count()`. Typargumente stehen in Ausdrücken nur direkt vor `::`; `x = List<Foo>(a);` bleibt ein Vergleich. Auch Case-Marken und Array-Größen qualifizieren mit `::` (`case Color::Blue:`, `i32[Limits::N]`); `case Color.Blue:` und `byte[Buffers.SIZE]` sind Syntaxfehler.
10. **Generische Methoden** tragen ihre Typ-Parameter nach dem Namen: `T max<T : Comparable>(T a, T b)`. Beim Aufruf werden keine Typargumente geschrieben.
11. **Mehrere Constraints** werden mit `&` verbunden; ein Komma trennt Typ-Parameter: `<T : A, B>` deklariert `T` mit Constraint `A` und einen zweiten Parameter `B`.
12. **`void` als Typargument** ist syntaktisch überall erlaubt; dass es nur an letzter Stelle von `Func` stehen darf, prüft ein späterer Schritt.
13. **Feste Arrays.** Die Größe in `T[...]` ist ein Ganzzahl-Literal oder ein Name; Ausdrücke wie `i32[2 + 2]` sind nicht erlaubt. Ob ein Name eine Konstante ist, prüft ein späterer Schritt. Deshalb ist `List[Foo] x;` syntaktisch ein festes Array.
14. **Bereiche** stehen nur in Indexklammern: `a[1..3]`, `a[..3]`, `a[1..]`, `a[..]`. `x = 1..3;` ist ein Fehler.
15. **Slices** haben keine eigene Syntax; `Slice<T>` ist ein gewöhnlicher generischer Typ.
16. **`export`** steht vor `class`, `mixin` oder `enum`, bei abstrakten Klassen vor `abstract`: `export abstract class Shape`. An Methoden, Feldern, `main` und `using` ist es nicht erlaubt.

17. **Benannte Borrows** werden mit `ref` deklariert: `ref Foo f = list[0];` (lesend) und `ref Foo& g = &x.child;` (veränderbar, `&` an Typ und Initialisierer). Ohne Initialisierer, mit Besitzform, mit `var`, als Feld, als Parameter und im `for`-Kopf ist `ref` ein Syntaxfehler. Ob der Initialisierer ein Ort ist, prüft ein späterer Schritt.

18. **Downcast mit `as`.** `e as T` ist ein zweistelliger Ausdruck, der stärker bindet als Vergleiche und schwächer als Schiebeoperatoren: `s as Circle != null` ist `(s as Circle) != null`, `a + b as Foo` ist `(a + b) as Foo`.
19. **Enum-Werte.** Eine Variante trägt entweder Daten oder einen Wert aus einem Ganzzahlliteral, auch negativ: `enum Level { Low = 1, Mid, High = 10 }`. Ausdrücke und Konstantennamen als Wert sind Syntaxfehler.

20. **Constraint `new()`.** Ein Constraint ist ein Typname oder `new()` ohne Parameter: `class Pool<T : Comparable<T> & new()>`.
21. **Mixins** enthalten Methoden, Anforderungen und `import`-Anweisungen, aber keine Felder, Konstruktoren und Sichtbarkeits-Labels.

22. **`discard`** steht vor einem Ausdruck als eigene Anweisung: `discard list.pop();`.
23. **`nodiscard`** steht vor `static`, `virtual`, `override` oder `abstract` einer Methode und vor Mixin-Anforderungen, nicht an Feldern und Konstruktoren.

24. **try** hat mindestens ein `catch` oder ein `finally`; `finally` steht nach allen `catch`.

25. **`&` in Typargumenten** ist syntaktisch überall erlaubt (`Func<Foo&, void>`); dass es nur an Parametern von `Func` stehen darf, prüft ein späterer Schritt.

26. **Borrow-Rückgabe.** Ein Rückgabetyp kann `ref T` oder `ref T&` sein (`ref V? get(K key)`, `ref V& at(K key) &`); `return &e;` liefert einen veränderbaren Borrow.
27. **Statische Member primitiver Typen** werden wie bei Klassen mit `::` angesprochen: `i32::MAX`, `i32::parse(s)`, `string::join(sep, parts)`.

28. **using** nennt einen Typ (`using a.b.C;`) oder vergibt einen anderen Namen (`using X = a.b.C;`); `using a.b.*;` ist ein Syntaxfehler.

### 3.2 Operatoren

Vorrang von schwach nach stark bindend:

| Stufe | Operatoren | Assoziativität |
|---|---|---|
| 1 | `=` `+=` `-=` `*=` `/=` `%=` (nur in Zuweisungsanweisungen) | – |
| 2 | `? :` | rechts |
| 3 | `??` | rechts |
| 4 | `\|\|` | links |
| 5 | `&&` | links |
| 6 | `\|` | links |
| 7 | `^` | links |
| 8 | `&` | links |
| 9 | `==` `!=` | links |
| 10 | `<` `>` `<=` `>=` | links |
| 10a | `as` | links |
| 11 | `<<` `>>` | links |
| 12 | `+` `-` | links |
| 13 | `*` `/` `%` | links |
| 14 | unär: `!` `~` `-` `++` `--` | rechts |
| 15 | Postfix: `.` `?.` `()` `[]` `++` `--` | links |

### 3.3 Geprüfte Lesarten

Die im Entwurf vermuteten Mehrdeutigkeiten sind wie folgt aufgelöst. ANTLR meldet nur für die letzte Zeile der Tabelle eine Mehrdeutigkeit.

| Eingabe | Lesart | Grund |
|---|---|---|
| `a < b > c;` | Deklaration von `c` mit Typ `a<b>` | Ein Vergleich ist keine gültige Anweisung |
| `x = a < b > (c);` | Vergleich | In Ausdrücken gibt es keine Typargumente |
| `List<List<Foo>> x = ...;` | Deklaration | Kein eigenes Token `>>` |
| `Foo? f = flag ? a : b;` | Deklaration mit Bedingungsausdruck | `Foo? f` ist kein Ausdruck |
| `a?.b();` | Aufruf mit Null-Prüfung | `?.` ist ein eigenes Token; `.5` ist kein Literal |
| `(i32 a, i32 b) = f();` | Tupel-Deklaration | Typ gefolgt von Name |
| `(i32, i32) t = f();` | Variable mit Tupel-Typ | |
| `(a, b) = f();` | Zuweisung an vorhandene Variablen | |
| `f((i32 x) => x + 1, y);` | Lambda mit Rumpf `x + 1`, dann zweites Argument | Lambda auf Ausdrucksebene |
| `List<Foo>::create();` | Statischer Aufruf | Nach `>` folgt `::`, das kann kein Vergleich sein |
| `x = flag ? Color::Red : Color::Blue;` | Bedingungsausdruck mit zwei statischen Zugriffen | `::` ist ein eigenes Token und wird nie als zwei `:` gelesen |
| `x = a < b > c.d();` | Vergleich | Nach `>` folgt kein `.` |
| `x = max < a > (b);` | Vergleich | Typargumente werden beim Aufruf nicht geschrieben |
| `a[N] b;` | Deklaration eines festen Arrays | Name nach dem Typ |
| `a[N] = b;` | Zuweisung an ein Element | `=` nach der Klammer |
| `if (a) if (b) f(); else g();` | `else` gehört zum inneren `if` | einzige gemeldete Mehrdeutigkeit, per Regel aufgelöst |

### 3.4 Testlauf

Stand des letzten Laufs: 18 positive Dateien, 127 negative Tests, 80 Lesarten, 258 Konformitätstests (Syntax; 234 von 245 Regeln genannt, 11 begründet ohne Test); 0 Fehler, 0 unerwartete Mehrdeutigkeiten. Erzeugt wurde der Parser mit antlr-ng 1.0.10 (Python-Ziel); mit dem Java-Werkzeug von ANTLR ist die Grammatik nicht getestet. Dieselben Fälle liest der Rust-Parser `rppc` (Meilenstein M1) mit demselben Ergebnis.

Regeln, die die Grammatik nicht ausdrückt und die ein späterer Prüfschritt übernehmen muss:

- Eine Ausdrucksanweisung ohne Zuweisung muss mit Aufruf, `++` oder `--` enden (im Testlauf bereits geprüft).
- `own` nur an Parametern und Lambdas.
- `var` nur mit Initialisierung.
- `void` als Typargument nur an letzter Stelle von `Func`.
- Paketname passt zum Verzeichnis der Datei.
- Größe eines festen Arrays ist eine Konstante, die zur Compilezeit feststeht.
- `Slice<T>` nie als Feldtyp.
- Öffentliche Member exportierter Typen verwenden in ihrer Signatur nur exportierte Typen.
- Pakete einer Bibliothek beginnen mit dem Bibliotheksnamen.
- Höchstens ein `main` im Programm.
- Der Initialisierer einer `ref`-Deklaration ist ein Ort (Variable, Feld- oder Indexzugriff), kein temporärer Wert.
- Vor `::` steht ein Typ; vor `.` ein Wert oder ein Paketname. `Foo.create()` für eine statische Methode ist deshalb erst bei der Namensauflösung ein Fehler, weil `foo.create()` syntaktisch gleich aussieht.

### 3.5 Befunde

Alle beim Testen aufgefallenen Punkte sind entschieden (siehe „R++ Designentscheidungen“, Abschnitt 16). Bewusst in Kauf genommen ist, dass `a > > b` mit Leerzeichen als Schiebeoperator akzeptiert wird.

## 5. Ownership und Borrowing

Ausgearbeitet im Dokument „R++ Ownership-Kernkalkül“ (Entwurf 0.5):

- Kernsprache FR++ mit Klassen, Feldern, Methoden, `own`, `shared`, `weak`, lesendem und veränderbarem Borrow.
- Typregeln R-5.1 bis R-5.24 als Inferenzregeln; R-5.23 und R-5.24 erlauben Borrows, die von `this` ausgehen, als Rückgabe. Borrows leben lexikalisch (Aufruf oder Block); der Compiler führt eine Menge aktiver Loans statt Lifetimes.
- Big-Step-Semantik mit Heap, Drop-Flags und einem Sperrzustand für `shared`-Objekte.
- Sätze 5.1 bis 5.3 (Speichersicherheit, Iteratoren, keine Laufzeitprüfung ohne `shared`) mit Beweisskizze; ein vollständiger Beweis steht aus.
- Abbildung der Oberflächensprache auf FR++.
- Konformitätstests in `konformitaet/kap05_ownership/` (6 akzeptierte, 20 abgelehnte Programme).
- Ausführbares Modell in `fr-modell/`: Typprüfer und Interpreter für FR++ mit Invariantenprüfung, 28 handgeschriebene Programme und Zufallstests. Im letzten Lauf (2000 Programme, 816 angenommen) gab es keinen Verstoß gegen Satz 5.1 oder 5.3; Gegenproben mit abgeschalteten Regeln zeigen, dass die Regeln echte Fehler verhindern (Kernkalkül, Abschnitt 10).

Die dabei nötigen Festlegungen V-1 bis V-11 und die Schreibweise `ref` für benannte lokale Borrows sind entschieden (siehe „R++ Designentscheidungen“, Abschnitt 17).

## Nächste Schritte

1. M6 fortsetzen: Die ganze Prüfung in R++ (`rpp-compiler/`: Lexer, Präprozessor, Parser, Projekte, Deklarationen, Rümpfe mit Flussanalyse) ist fertig und gleichwertig zu `rppc`. Als Nächstes die Angaben für die Ausführung und ein Codegenerator, damit sich der Compiler selbst übersetzen kann; dazu die Laufzeit der Prüfung im Interpreter (Fixpunkt an verschachtelten Schleifen). Offen aus M5 sind Typargumente generischer Methoden zur Laufzeit.
2. Ein Bibliotheksverzeichnis für die Konformitätstests festlegen, damit R-11.19 und R-11.20 prüfbar werden.
3. Satz 5.1 maschinell beweisen (Coq, Lean oder Isabelle); das ausführbare Modell dient dabei als Referenz.
