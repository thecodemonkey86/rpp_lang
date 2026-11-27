# R++ Klassen, Mixins und Typ-Parameter (Kapitel 7)

[English](R%2B%2B%20Classes%20and%20Generics.md)

Stand: 21.11.2026, Entwurf 0.1. Dieses Dokument füllt Kapitel 7 des „R++ Spezifikationsgerüsts“. Es regelt den Aufbau von Klassen, Konstruktoren, Destruktoren und Kopien, Vererbung und Überschreiben, Überladen, Mixins und generische Typen und Methoden. Welche Typen daraus entstehen und wie sie zueinander stehen, regelt Kapitel 4; Besitz und Borrows Kapitel 5; `export` Kapitel 11.

Alle Festlegungen sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 8–10 und 20). Jede normative Regel trägt eine Nummer `R-7.n`. Die Konformitätstests liegen in `conformance/ch07_classes/`.

## 1. Klassen

**R-7.1 (Aufbau).** Eine Klasse besteht aus Feldern, Methoden, Konstruktoren, höchstens einem Destruktor, höchstens einer Kopierdeklaration `copy(…)`, Mixin-Importen und Sichtbarkeits-Labels. Feldnamen sind innerhalb der Klasse und ihrer Basisklassen eindeutig. Ein Feld und eine Methode dürfen nicht gleich heißen.

**R-7.2 (Sichtbarkeit).** Ein Label `public:`, `protected:` oder `private:` gilt für alle folgenden Member bis zum nächsten Label. Member vor dem ersten Label sind `private`.
- `public`: überall sichtbar, wo die Klasse sichtbar ist.
- `protected`: in der Klasse und in allen Unterklassen, auch in anderen Projekten (Kapitel 11).
- `private`: nur im Rumpf der Klasse selbst, dort aber für alle ihre Objekte.

Mixin-Importe stehen außerhalb der Sichtbarkeitsregeln; die Methoden eines Mixins sind immer `public` (R-7.18).

**R-7.3 (Felder).** Ein Feld kann einen Initialisierer haben (`i32 count = 0;`). Initialisierer werden beim Erzeugen in Deklarationsreihenfolge ausgewertet (R-7.6). `static const`-Felder brauchen einen Wert, der zur Compilezeit feststeht; veränderbare statische Felder gibt es nicht.

**R-7.4 (this).** In Methoden, Konstruktoren, Destruktor und `copy` bezeichnet `this` das aktuelle Objekt. In einer Methode ohne `mut` ist es ein lesender Borrow, in einer `mut`-Methode, im Konstruktor, im Destruktor und in `copy` ein veränderbarer Borrow (Kapitel 5). Statische Methoden haben kein `this`.

## 2. Konstruktoren

**R-7.5 (Konstruktoren).** Eine Klasse kann mehrere Konstruktoren mit verschiedenen Parameterlisten haben; `new C(args)` wählt einen nach den Regeln für Überladen (Abschnitt 5). Hat eine Klasse keinen Konstruktor, bekommt sie einen parameterlosen, wenn jedes Feld einen Initialisierer hat oder nullbar ist; sonst ist `new C()` ein Fehler.

**R-7.6 (Ablauf der Erzeugung).** `new C(args)` läuft in dieser Reihenfolge ab:
1. Der Basiskonstruktor: der mit `: base(…)` genannte, sonst der parameterlose der Basisklasse. Hat die Basisklasse keinen parameterlosen Konstruktor, ist `: base(…)` Pflicht.
2. Die Feldinitialisierer der Klasse in Deklarationsreihenfolge.
3. Der Rumpf des Konstruktors.

**R-7.7 (Vollständige Initialisierung).** Am Ende des Konstruktors ist jedes nicht nullbare Feld zugewiesen, auf jedem Weg durch den Rumpf. Bevor das der Fall ist, darf `this` nur zum Setzen und Lesen bereits gesetzter Felder verwendet werden: keine Methodenaufrufe auf `this`, keine Übergabe von `this` (Kapitel 5, V-10). Nullbare Felder ohne Initialisierer sind `null`.

**R-7.8 (Virtuelle Aufrufe).** Ist `this` vollständig initialisiert, sind Aufrufe virtueller Methoden im Konstruktor dynamisch: Es läuft die Fassung der tatsächlichen Klasse. Im Basiskonstruktor ist die Unterklasse aber noch nicht initialisiert; dort sind Aufrufe auf `this` deshalb nach R-7.7 ohnehin nicht möglich, solange der Basiskonstruktor läuft.

## 3. Destruktor und Kopie

**R-7.9 (Destruktor).** Ein Destruktor `destructor() { … }` hat keine Parameter und wird nie direkt aufgerufen. Bei der Freigabe laufen die Destruktoren von der tatsächlichen Klasse bis zur obersten Basisklasse, danach werden die Felder freigegeben (Kapitel 5, V-7). Aufrufe virtueller Methoden im Destruktor sind dynamisch, denn die Felder abgeleiteter Klassen leben zu diesem Zeitpunkt noch. Verlässt eine Exception einen Destruktor, endet das Programm (V-11).

**R-7.10 (Kopierdeklaration).** `copy(C other) { … }` legt fest, wie eine Kopie von `C` entsteht. `other` ist ein lesender Borrow. Der Rumpf initialisiert `this` wie ein Konstruktor (R-7.7). Hat die Basisklasse ein `copy`, wird zuerst deren `copy` für den Basisanteil ausgeführt; hat sie keines, ist `copy` in der Unterklasse ein Fehler.

**R-7.11 (Kopie ist virtuell).** `copy(a)` wählt die Kopierdeklaration nach der tatsächlichen Klasse des Objekts. Das Ergebnis hat den statischen Typ von `a` als `own`-Wert und ist ein vollständiges Objekt der tatsächlichen Klasse. Deshalb gilt: Hat eine Klasse ein `copy`, muss jede Unterklasse ein eigenes `copy` haben. Hat die statische Klasse von `a` kein `copy`, ist `copy(a)` ein Fehler.

## 4. Vererbung und Überschreiben

**R-7.12 (Vererbung).** Eine Klasse erbt von höchstens einer Klasse (`class A : B`); `B` ist eine Klasse, kein Mixin. Vererbungszyklen sind ein Fehler. Die Unterklasse erbt alle Felder und Methoden.

**R-7.13 (virtual, abstract, override).**
- Nur Methoden mit `virtual` oder `abstract` können überschrieben werden. Eine überschriebene Methode bleibt überschreibbar; ein `sealed` gibt es in Version 1 nicht.
- Eine Methode, die eine geerbte überschreibt oder eine abstrakte implementiert, trägt `override`. Ohne `override` ist eine gleichnamige Methode mit gleichen Parametertypen ein Fehler.
- `override` verlangt dieselben Parametertypen einschließlich Besitzformen und `mut`, denselben `mut`-Modus der Methode und denselben Rückgabetyp. Die Sichtbarkeit darf nicht enger werden (`public` bleibt `public`).
- Statische Methoden sind nie virtuell.

**R-7.14 (Abstrakte Klassen).** Eine Klasse mit mindestens einer abstrakten Methode, ob eigen, geerbt oder als nicht erfüllte Mixin-Anforderung, muss `abstract class` sein. Abstrakte Klassen können Konstruktoren haben, aber nicht mit `new` erzeugt werden.

**R-7.15 (Aufruf von Basis-Fassungen).** In einer Methode der Klasse ruft `base.m(…)` die Fassung von `m` aus der Basisklasse auf und `M::m(…)` die aus dem importierten Mixin `M`, beide jeweils für `this`. Außerhalb der Klasse und für andere Objekte als `this` gibt es diese Aufrufe nicht.

## 5. Überladen

**R-7.16 (Überladene Methoden).** Mehrere Methoden oder Konstruktoren einer Klasse dürfen gleich heißen, wenn sich ihre Parameterlisten unterscheiden. Nicht erlaubt sind Varianten, die sich nur
- im Rückgabetyp oder
- in Besitzform oder Borrow-Markierung der Parameter (`f(Foo x)` neben `f(own Foo x)` oder `f(mut Foo x)`)

unterscheiden. Eine statische und eine Instanzmethode dürfen nicht gleich heißen. Geerbte und aus Mixins importierte Methoden zählen bei der Auflösung mit.

**R-7.17 (Auflösung).** Bei einem Aufruf kommen alle sichtbaren Varianten mit dem Namen und der passenden Anzahl von Argumenten in Frage. Für jedes Argument bekommt jede Variante einen Rang:

| Rang | Argument → Parameter |
|---|---|
| 0 – genau | gleicher Typ (Besitzform nach Kapitel 5 erfüllbar); Literal mit seinem Standardtyp (`1` → `i32`, `1.0` → `f64`); `null` → nullbarer Typ; Lambda mit genau passenden Parametertypen |
| 1 – Zahlanpassung | Literal passt in einen anderen Zahltyp; automatische Zahlumwandlung (Kapitel 4, R-4.6) |
| 2 – Upcast | Klasse → Basisklasse oder Mixin (R-4.22) |
| 3 – nullbar | `T` → `T?` (zusammen mit einem der obigen Ränge zählt der schlechtere) |

Haben zwei Varianten bei einem Argument denselben Rang, ist die mit dem **spezielleren** Parametertyp besser: dem, der sich automatisch in den anderen umwandeln lässt (`i32` vor `i64`, `Circle` vor `Shape`).

Gewählt wird die Variante, die bei keinem Argument schlechter und bei mindestens einem besser ist als jede andere. Bei Gleichstand gewinnt eine nicht-generische Variante gegen eine generische. Gibt es keine solche Variante, ist der Aufruf mehrdeutig und ein Fehler; man hilft mit `cast` oder einer Variable des gewünschten Typs.

Beispiele:
- Bei `f(i32)` und `f(i64)` wählt `f(1)` `f(i32)` (genau), und `f(x)` mit `x : i8` ebenfalls (beide Zahlanpassung, `i32` ist spezieller).
- Bei `f(i64)` und `f(f64)` ist `f(1)` mehrdeutig: Beides ist Zahlanpassung, und weder `i64` noch `f64` lässt sich automatisch in den anderen umwandeln.
- Bei `f(Shape)` und `f(Circle)` wählt `f(circle)` `f(Circle)`.

## 6. Mixins

**R-7.18 (Aufbau).** Ein Mixin enthält Instanzmethoden mit Rumpf, Anforderungen (Methoden ohne Rumpf) und Importe anderer Mixins. Es hat keine Felder, keine Konstruktoren, keinen Destruktor, kein `copy`, keine statischen Methoden und keine Konstanten. Alle seine Methoden sind `public`. Ein Mixin kann Typ-Parameter haben (`mixin Equatable<T>`).

**R-7.19 (Prüfung an der Definition).** Die Rümpfe eines Mixins werden einmal geprüft, an der Definition. `this` hat dabei den Typ des Mixins; verfügbar sind nur die eigenen Methoden, die Anforderungen und die Methoden importierter Mixins.

**R-7.20 (Import).** `import M;` in einer Klasse wirkt so, als stünden die Methoden von `M` in der Klasse. Jede Anforderung von `M` muss von einer Methode der Klasse erfüllt werden: eigen, geerbt oder aus einem anderen Mixin, mit genau der Signatur der Anforderung. Importiert ein Mixin ein anderes, gehen dessen Methoden und Anforderungen mit über. Wird dasselbe Mixin auf mehreren Wegen importiert, zählt es einmal.

**R-7.21 (Konflikte).** Bringen zwei importierte Mixins, oder ein Mixin und die Klasse, eine Methode mit gleichem Namen und gleichen Parametertypen mit, ist das ein Fehler, es sei denn, die Klasse definiert diese Methode selbst mit `override`. Darin kann sie eine Fassung gezielt aufrufen (`Printable::describe()`, R-7.15). Gleiche Namen mit verschiedenen Parametertypen sind kein Konflikt, sondern Überladungen.

**R-7.22 (Mixin als Typ).** Ein Mixin ist ein Typ (Kapitel 4, R-4.22). Über einen Wert vom Mixin-Typ lassen sich seine Methoden und Anforderungen aufrufen; der Aufruf geht an die Fassung der tatsächlichen Klasse. Mixin-Methoden mit `virtual` können in Unterklassen überschrieben werden.

## 7. Typ-Parameter

**R-7.23 (Deklaration).** Klassen, Mixins und Methoden können Typ-Parameter haben. Ein Constraint ist ein Mixin-Typ, `new()` oder `Transferable` (R-16.8); mehrere werden mit `&` verbunden: `class Pool<T : Comparable<T> & new()>`.

**R-7.24 (Prüfung an der Definition).** Ein generischer Rumpf wird einmal für alle Typargumente geprüft. Auf Werten vom Typ `T` sind nur erlaubt:
- die Methoden der Mixins aus den Constraints,
- Zuweisung, Move und Borrow nach Kapitel 5,
- `==` und `!=`, wenn `T : Equatable<T>` (Kapitel 4, R-4.42),
- `new T()`, wenn `T : new()`,
- `T?` und die Null-Prüfungen aus Kapitel 6.

**R-7.25 (new()).** Das Typargument zu einem Parameter mit `new()` muss eine nicht abstrakte Klasse mit einem sichtbaren parameterlosen Konstruktor sein (R-7.5). Primitive Typen erfüllen `new()` nicht.

**R-7.26 (Instanziierung).** Bei jeder Verwendung eines generischen Typs oder Aufruf einer generischen Methode prüft der Compiler, ob die Typargumente die Constraints erfüllen. Generische Typen sind invariant (R-4.43). Im Interpreter bleiben Typargumente zur Laufzeit erhalten, im Release wird pro Typargument eigener Code erzeugt.

**R-7.27 (Statische Member).** Statische Methoden und Konstanten einer generischen Klasse werden mit Typargumenten angesprochen: `List<Foo>::create()`. Eine statische Konstante darf nicht von einem Typ-Parameter abhängen.

**R-7.28 (Generische Methoden).** Typargumente einer generischen Methode werden nur aus den Argumenten abgeleitet, nie ausgeschrieben. Lässt sich ein Typ-Parameter nicht eindeutig ableiten, ist das ein Fehler. Ein als Argument übergebenes Lambda trägt seine Parametertypen bei und, wo ein Typ-Parameter der Methode im Rückgabetyp des Funktionsparameters steht, seinen Rückgabetyp (R-10.3); `void` wird nie als Typargument abgeleitet (R-4.42).

## 8. Konformitätstests

Die Testprogramme liegen in `conformance/ch07_classes/`; geprüft werden sie mit `rppc test` (erste Implementierung, Meilenstein M3).

| Regeln | Akzeptiert | Abgelehnt |
|---|---|---|
| R-7.2 Sichtbarkeit | `a01_classes.rpp` | `n01_private_access_from_outside.rpp`, `n02_default_visibility_is_private.rpp` |
| R-7.5 – R-7.7 Konstruktoren | `a01_classes.rpp` | `n03_no_default_constructor.rpp`, `n04_missing_base_call.rpp`, `n05_uninitialized_field.rpp` |
| R-7.10, R-7.11 copy | `a03_copy.rpp` | `n06_missing_copy_in_subclass.rpp` |
| R-7.13, R-7.14 Überschreiben | `a02_inheritance.rpp` | `n07_missing_override.rpp`, `n08_non_virtual.rpp`, `n09_different_mode.rpp`, `n10_narrower_visibility.rpp`, `n11_instantiate_abstract_class.rpp` |
| R-7.16, R-7.17 Überladen | `a05_overloading.rpp` | `n12_return_type_only.rpp`, `n13_ownership_only.rpp`, `n14_ambiguous.rpp` |
| R-7.18 – R-7.21 Mixins | `a04_mixins.rpp` | `n15_mixin_conflict.rpp`, `n16_missing_requirement.rpp`, `n17_static_mixin_member.rpp` |
| R-7.23 – R-7.28 Generics | `a06_generics.rpp` | `n18_new_without_constraint.rpp`, `n19_unsatisfied_constraint.rpp`, `n20_method_without_constraint.rpp` |
