# R++ Null-Sicherheit (Kapitel 6)

[English](R%2B%2B%20Null%20Safety.md)

Stand: 21.11.2026, Entwurf 0.2. Dieses Dokument füllt Kapitel 6 des „R++ Spezifikationsgerüsts“. Es legt fest, wann ein nullbarer Wert benutzt werden darf, wie der Compiler das prüft (Typverengung) und was die Kurzformen `?.` und `??` bedeuten. Welche Typen nullbar sind, regelt Kapitel 4 (Abschnitt 11); wie Verengung mit Besitz und Borrows zusammenspielt, Kapitel 5.

Alle Festlegungen dieses Kapitels sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 4 und 19), einschließlich der Klarstellung V-6.1 aus der ersten Implementierung (Abschnitt 9). Jede normative Regel trägt eine Nummer `R-6.n`. Die Konformitätstests liegen in `conformance/ch06_null_safety/`.

## 1. Ziel

**Satz 6.1 (keine Null-Dereferenzierung).** In einem Programm, das die Regeln dieses Kapitels erfüllt, wird nie ein Member eines Werts aufgerufen oder gelesen, der `null` ist. Es gibt deshalb keine `NullPointerException` oder vergleichbare Laufzeitfehler.

Der Satz ist hier nicht formal bewiesen. Er folgt aus R-6.2 (nur nicht nullbare Werte werden dereferenziert) und R-6.4 bis R-6.14 (Verengung nur dort, wo der Wert auf jedem Weg geprüft ist). Eine Erweiterung des Kernkalküls FR++ um `null` würde ihn beweisbar machen.

## 2. Grundregeln

**R-6.1 (Nicht nullbar als Standard).** Ein Typ ohne `?` enthält `null` nicht. Nie `null` sind außerdem `this`, das Ergebnis von `new`, Literale außer `null`, Parameter ohne `?` und Ergebnisse von Methoden mit nicht nullbarem Rückgabetyp.

**R-6.2 (Dereferenzierung).** Feldzugriff `x.f`, Methodenaufruf `x.m(…)`, Indexzugriff `x[i]`, `for (… : x)`, die Arithmetik und Vergleiche aus Kapitel 4 und das Zerlegen eines Tupels verlangen einen Operanden mit nicht nullbarem Typ. Ist der statische Typ `T?` und der Operand an dieser Stelle nicht verengt (Abschnitt 3), ist das ein Fehler.

**R-6.3 (Vergleich mit null).** `x == null` und `x != null` (auch `null == x`) sind nur erlaubt, wenn `x` einen nullbaren Typ hat. Bei nicht nullbarem `x` steht das Ergebnis schon fest; der Compiler lehnt den Vergleich ab. `null == null` ist ebenfalls ein Fehler.

Es gibt keinen Operator, der die Prüfung überspringt (kein `x!`).

## 3. Typverengung

### 3.1 Was verengt wird

**R-6.4 (Verengbare Ausdrücke).** Verengt werden nur lokale Variablen (auch `ref`-Variablen und Schleifenvariablen) und Parameter. Felder (`this.child`, `x.next`), Array-Elemente (`list[i]`) und Aufrufergebnisse werden nie verengt, weil sich ihr Wert zwischen Prüfung und Verwendung ändern könnte. Man holt sie zuerst in eine lokale Variable (R-6.15).

**R-6.5 (Verengter Typ).** Ist eine Variable `x` vom Typ `T?` an einer Stelle verengt, hat sie dort den Typ `T`. Besitzform und Borrow-Art bleiben unverändert: Ein `ref Foo? c` wird zu `ref Foo`, ein `shared Foo? s` zu `shared Foo`.

### 3.2 Fakten aus Bedingungen

Für jeden `bool`-Ausdruck `c` bestimmt der Compiler zwei Mengen von Variablen: `W(c)` sind nicht null, wenn `c` wahr ist; `F(c)` sind nicht null, wenn `c` falsch ist.

**R-6.6 (Fakten).**

| Ausdruck `c` | `W(c)` | `F(c)` |
|---|---|---|
| `x != null`, `null != x` | `{x}` | `{}` |
| `x == null`, `null == x` | `{}` | `{x}` |
| `!a` | `F(a)` | `W(a)` |
| `a && b` | `W(a) ∪ W(b)` | `F(a) ∩ F(b)` |
| `a \|\| b` | `W(a) ∩ W(b)` | `F(a) ∪ F(b)` |
| `(a)` | `W(a)` | `F(a)` |
| alles andere | `{}` | `{}` |

**R-6.7 (Innerhalb von Ausdrücken).** In `a && b` wird `b` unter `W(a)` geprüft, in `a || b` unter `F(a)`. In `c ? a : b` wird `a` unter `W(c)` und `b` unter `F(c)` geprüft. Beispiel: `x != null && x.ok()` und `x == null || x.empty()` sind erlaubt.

### 3.3 Anweisungen

Eine Anweisung **endet nicht normal**, wenn sie `return`, `throw`, `break` oder `continue` ist, ein Aufruf von `fail` (Kapitel 9, R-9.19), ein Block, dessen letzte erreichbare Anweisung nicht normal endet, ein `if` mit `else`, dessen beide Zweige nicht normal enden, oder ein vollständiges `switch`, dessen Fälle alle nicht normal enden.

**R-6.8 (if).** In `if (c) S₁ else S₂` wird `S₁` unter `W(c)` und `S₂` unter `F(c)` geprüft. Danach gilt:
- endet `S₁` nicht normal: der Stand nach `S₂` (bei fehlendem `else`: `F(c)`);
- endet `S₂` nicht normal: der Stand nach `S₁`;
- sonst: die Schnittmenge beider Stände.

Daraus folgt die bisherige Form: Nach `if (x == null) { return; }` ist `x` für den Rest des Blocks verengt.

**R-6.9 (Schleifen).** In `while (c) S` und `for (…; c; …) S` wird `S` unter `W(c)` geprüft. Am Schleifenkopf gilt die größte Menge von Fakten, die vor der Schleife gilt und am Ende jedes Durchlaufs (auch bei `continue`) wieder gilt. Nach der Schleife gilt `F(c)` geschnitten mit den Ständen an allen `break`. Für `do S while (c)` gilt dasselbe, nur wird `S` ohne `W(c)` geprüft.

**R-6.10 (assert).** `assert(c, text)` wird in beiden Modi ausgewertet und wirft bei `false` eine Exception. Danach gilt `W(c)`.

**R-6.11 (Zuweisung).** Nach `x = e;` ist `x` genau dann verengt, wenn `e` einen nicht nullbaren Typ hat (oder selbst eine verengte Variable ist). `x = null;` hebt die Verengung auf, `x = new Foo();` verengt `x`. Ein Move (Kapitel 5) beendet die Verfügbarkeit von `x` ohnehin.

**R-6.12 (try).** In einem `catch`- oder `finally`-Block gelten nur Fakten, die vor dem `try` galten und im `try`-Block nicht durch eine Zuweisung aufgehoben werden.

**R-6.13 (Lambdas).** Ein normales Lambda sieht die Fakten an der Stelle, an der es steht; die erfassten Variablen können während des Aufrufs nicht geändert werden (Kapitel 5). Ein `own`-Lambda erfasst den Wert mit dem verengten Typ.

**R-6.14 (Gültigkeit).** Ein Fakt gilt nur innerhalb der Methode, in der er entsteht, und nur bis zur nächsten Zuweisung an die Variable.

## 4. Nullbare Felder und Array-Elemente

**R-6.15 (Holen).** Ein nullbares Feld oder Element wird vor der Prüfung in eine lokale Variable geholt, je nach Besitzform:

| Feld | Holen zum Prüfen | Bemerkung |
|---|---|---|
| `Foo? child` (own) | `ref Foo? c = this.child;` | ein normales `Foo? c = this.child;` wäre ein Move aus dem Feld (Kapitel 5, V-3) |
| `Foo? child` (own), herausnehmen | `Foo? c = take(mut this.child);` | das Feld ist danach `null` |
| `shared Foo? next` | `shared Foo? n = this.next;` | kopiert den Verweis (Zähler + 1) |
| `weak Foo w` | `shared Foo? n = this.w.lock();` | |
| Element von `Foo?[] list` | `ref Foo? e = list[i];` | |
| primitiv, z. B. `i32? count` | `i32? c = this.count;` | primitive Werte werden kopiert |

Danach wird die lokale Variable wie in Abschnitt 3 geprüft.

## 5. Kurzformen

**R-6.16 (Sicherer Zugriff `?.`).** `x?.m(…)` und `x?.f` verlangen einen nullbaren Operanden `x` (bei nicht nullbarem `x` ist `?.` ein Fehler). Ist `x` `null`, ist das Ergebnis `null`, und der **Rest der Kette** wird übersprungen, einschließlich der Argumente: In `a?.b.c(f())` wird bei `a == null` weder `.b`, `.c` noch `f()` ausgewertet. Sonst wird die Kette normal ausgewertet.

**R-6.17 (Typ von `?.`).** Hat die Kette ohne `?.` den Typ `R`, hat sie mit `?.` den Typ `R?` (bei `R = S?` bleibt es `S?`). Liefert die Kette `void`, ist sie nur als Anweisung erlaubt. Ist das Ergebnis ein `own`-Feld, ist es ein Borrow `ref F?` und kein Move.

**R-6.18 (Ersatzwert `??`).** `a ?? b` verlangt ein nullbares `a` vom Typ `T?` (sonst Fehler). `b` wird nur ausgewertet, wenn `a` `null` ist. Hat `b` den Typ `T` (oder wird automatisch in `T` umgewandelt), ist das Ergebnis `T`; hat `b` den Typ `T?`, ist es `T?`. `??` ist rechtsassoziativ: `a ?? b ?? c` ist `a ?? (b ?? c)`. Für Besitz gilt: Ist `a` eine `own`-Variable und das Ziel ein Besitzer, wird `a` verschoben (Kapitel 5).

**R-6.19 (Kein `??=`).** Einen Zuweisungsoperator `??=` gibt es in Version 1 nicht; man schreibt `x = x ?? y;`.

## 6. Zusammenspiel mit anderen Kapiteln

**R-6.20 (Generics).** Ist ein Typ-Parameter `T` mit einem nullbaren Typ belegt, fällt `T?` mit ihm zusammen: Für `T = Foo?` ist `T?` einfach `Foo?`. `Map<K, V>.get(k)` mit Rückgabetyp `V?` funktioniert deshalb für jedes `V`; ein `null`-Ergebnis unterscheidet dann nicht zwischen „kein Eintrag“ und „Eintrag ist null“.

**R-6.21 (Downcast).** `e as C` liefert `C?` (Kapitel 4, R-4.23) und wird wie jeder nullbare Wert verengt: `ref Circle? c = s as Circle; if (c != null) { … }`.

**R-6.22 (weak).** `w.lock()` liefert `shared T?`; erst nach der Prüfung kann man auf das Objekt zugreifen (Kapitel 5, V-6).

**R-6.23 (Primitive).** Nullbare primitive Werte (`i32?`) haben keine Arithmetik und keine Vergleiche außer mit `null`, solange sie nicht verengt sind: `a + 1` mit `a : i32?` ist ein Fehler; `(a ?? 0) + 1` ist erlaubt.

## 7. Beispiel

```
string describe(Node? n, Shape s) {
    if (n == null) {
        return "leer";
    }
    // ab hier ist n verengt
    ref Node? child = n.child;    // Feld holen (R-6.15)
    string c = child?.name ?? "-";
    ref Circle? circle = s as Circle;
    if (circle != null && circle.r > 1.0) {
        return n.name + " groß";
    }
    assert(child != null, "Kind fehlt");
    return child.name;            // nach assert verengt
}
```

## 8. Konformitätstests

Die Testprogramme liegen in `conformance/ch06_null_safety/`; geprüft werden sie mit `rppc test` (erste Implementierung, Meilenstein M3).

| Regeln | Akzeptiert | Abgelehnt |
|---|---|---|
| R-6.2 Dereferenzierung | `a01_narrowing.rpp` | `n01_unchecked.rpp`, `n10_nullable_arithmetic.rpp` |
| R-6.3 Vergleich mit null | `a01_narrowing.rpp` | `n06_compare_non_nullable.rpp` |
| R-6.4 Felder, Elemente | `a03_fields.rpp` | `n02_narrow_field.rpp`, `n11_narrow_element.rpp` |
| R-6.6 – R-6.8 Bedingungen, if | `a01_narrowing.rpp` | `n04_if_without_exit.rpp`, `n05_wrong_side_of_or.rpp` |
| R-6.9 Schleifen | `a01_narrowing.rpp` | `n12_loop_invalidates_narrowing.rpp` |
| R-6.10 assert | `a01_narrowing.rpp` | – |
| R-6.11 Zuweisung | `a01_narrowing.rpp` | `n03_assignment_invalidates_narrowing.rpp` |
| R-6.15 Holen | `a03_fields.rpp` | `n13_move_out_of_field.rpp` |
| R-6.16 – R-6.18 `?.`, `??` | `a02_shorthand.rpp` | `n07_safe_access_on_non_nullable.rpp`, `n08_fallback_not_nullable.rpp`, `n09_void_as_value.rpp` |
| R-6.13, R-6.20 Lambdas, Generics | `a04_lambdas_generics.rpp` | – |

## 9. Klarstellung aus der ersten Implementierung

| Nr. | Vorschlag |
|---|---|
| V-6.1 | (angenommen am 21.11.2026) Ob ein Ausdruck „einen nullbaren Typ hat“ (R-6.3, R-6.16, R-6.18), richtet sich bei einer Variablen nach ihrem deklarierten Typ, nicht nach dem verengten. Nach `Node? v = new Node();` ist `v` verengt (R-6.11), aber `v?.run()`, `v ?? w` und `v == null` bleiben erlaubt. Für Felder, Elemente und Aufrufe zählt ihr Typ. Begründung: Sonst würde jede Verengung benachbarten, bisher korrekten Code ungültig machen; der Laufzeittest `l02_safe_access.rpp` verwendet genau diese Form. |
