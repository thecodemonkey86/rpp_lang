# R++ Ownership-Kernkalkül (Kapitel 5)

Stand: 22.11.2026, Entwurf 0.9. Dieses Dokument füllt Kapitel 5 des „R++ Spezifikationsgerüsts“. Es beschreibt Ownership und Borrowing als kleine Kernsprache **FR++** mit Typregeln, Auswertungsregeln und einem Sicherheitssatz, nach dem Vorbild von Featherweight Java.

Die Oberflächensprache wird in Abschnitt 7 auf FR++ abgebildet. Die Regeln dort sind normativ für den Compiler; FR++ selbst ist ein Hilfsmittel für Spezifikation und Beweis, keine Sprache, die jemand schreibt.

Abschnitt 2 enthält Festlegungen, die beim Ausarbeiten über die bisherigen Designentscheidungen hinaus nötig wurden. Sie sind am 18.11.2026 entschieden worden (siehe „R++ Designentscheidungen“, Abschnitt 17).

## 1. Idee in drei Sätzen

1. **Eigentumsbaum.** Jedes Objekt hat genau einen Besitzer: eine lokale Variable, ein Feld eines anderen Objekts oder – bei `shared` – einen Referenzzähler. Freigegeben wird ein Objekt genau dann, wenn sein Besitzer es aufgibt.
2. **Lexikalische Borrows.** Weil Borrows nie in Feldern stehen und nie zurückgegeben werden, lebt jeder Borrow genau so lange wie ein Aufruf oder ein Block. Der Compiler verwaltet deshalb eine einfache Menge aktiver „Loans“ statt Lifetimes und verbietet für deren Dauer alle Zugriffe, die den Besitzweg ändern könnten.
3. **Dynamische Sperre für `shared`.** Auf ein `shared`-Objekt können mehrere Variablen zeigen, die der Compiler nicht unterscheiden kann. Für sie gilt die Exklusivitätsregel deshalb zur Laufzeit: Das Objekt trägt einen Sperrzustand, ein Konflikt wirft eine Exception. Programme ohne `shared` haben keine Laufzeitprüfung (Satz 5.3).

## 2. Neue Festlegungen

V-1 bis V-11 sind entschieden; V-3 ist gegenüber dem ersten Vorschlag um `take` erweitert. V-12 ist beim Umsetzen in der ersten Implementierung entstanden und am 21.11.2026 angenommen worden; V-13 ist beim Vergleich mit dem Modell aufgefallen und am 21.11.2026 angenommen worden; V-14 ist mit dem Interpreter (Meilenstein M4) entstanden und am 22.11.2026 angenommen worden. V-15 ist in Meilenstein M5 entstanden und am 22.11.2026 angenommen worden.

| Nr. | Festlegung | Begründung |
|---|---|---|
| V-1 | `shared`-Objekte sind veränderbar. Die Exklusivität wird für sie zur Laufzeit geprüft (Sperrzustand pro Objekt); ein Verstoß wirft `BorrowError` – in **beiden** Modi, weil die Prüfung die Speichersicherheit trägt. | Ohne Prüfung könnte über einen zweiten `shared`-Alias ein Vektor wachsen, über den gerade iteriert wird. Alternative: `shared` nur lesend; dann lassen sich Graphen und Beobachter-Strukturen nicht verändern. |
| V-2 | Ein Pfad geht nie *durch* ein `shared`- oder `weak`-Feld hindurch. `this.next.name()` übersetzt der Compiler in eine lokale Kopie: `shared Node t = this.next; t.name();` | Die Sperre braucht eine Variable, die das Objekt für die Dauer des Zugriffs am Leben hält. Entspricht der Regel aus Abschnitt 4 der Designentscheidungen, dass nullbare Felder vor dem Zugriff in eine lokale Variable geholt werden. |
| V-3 | Aus einem Feld kann nicht herausbewegt werden. Stattdessen gibt es zwei eingebaute Funktionen: `replace(&x.f, neu)` setzt den neuen Wert ein und gibt den alten zurück; `take(&x.f)` gibt es nur für nullbare Felder, es setzt das Feld auf `null` und gibt den alten Wert zurück. | Ein Feld ohne Wert wäre ein Loch im Objekt. `take(&x.f)` ist gleichbedeutend mit `replace(&x.f, null)` und im Kalkül deshalb kein eigenes Konstrukt. |
| V-4 | ~~In Version 1 geben Methoden keine Borrows zurück.~~ **Geändert am 21.11.2026:** Eine Methode darf einen Borrow zurückgeben, der von `this` ausgeht (`ref V get(K k)`). An der Aufrufstelle gilt er als Loan auf den Empfänger, bis zum Ende der Anweisung oder, mit `ref` gebunden, bis zum Blockende (R-5.23, R-5.24). | Ohne Borrow-Rückgabe wären `map.get(k)`, `list.first()` und Iteratoren über `own`-Elemente unmöglich. Weil der Borrow nur von `this` ausgehen darf, reicht ein Loan auf den Empfänger; Lifetimes sind weiter nicht nötig. |
| V-5 | Ein Move darf in nur einem Zweig eines `if` stehen. Danach gilt die Variable als verschoben; ob sie freigegeben werden muss, entscheidet zur Laufzeit ein Drop-Flag. | Wie in Rust; vermeidet unnötige Fehlermeldungen. |
| V-6 | `weak` entsteht nur aus `shared`. Zugriff über `w.lock()`, das `shared T?` liefert; danach greift die Null-Prüfung aus Kapitel 6. | Kein neues Sprachmittel nötig. |
| V-7 | Freigabereihenfolge: lokale Variablen am Blockende in umgekehrter Deklarationsreihenfolge. Bei einem Objekt zuerst der Destruktor, dann die Felder in umgekehrter Deklarationsreihenfolge, die Felder der Basisklasse zuletzt. | Wie C++. |
| V-8 | Ein lesender Borrow schützt alles, was über `own`-Kanten erreichbar ist. Über ein `shared`-Feld erreichte Objekte bleiben veränderbar (mit Laufzeitprüfung nach V-1). | Wer ein `shared`-Feld lesen kann, kann sich ohnehin eine eigene `shared`-Kopie holen; ein tieferer Schutz wäre nicht durchsetzbar. `const` bleibt eine reine Oberflächenprüfung. |
| V-9 | Werte vom Typ `shared T` werden bei Zuweisung und Übergabe implizit kopiert (Zähler + 1). Aus `own` wird `shared` nur, wenn das Ziel ausdrücklich den Typ `shared` hat (Variable, Feld oder Parameter): `shared Foo s = f;` verschiebt `f`. Umgekehrt wird aus `shared` nie `own`; dafür gibt es `copy(s)`. | Präzisiert „shared wird nie implizit abgeleitet“. |
| V-10 | Im Konstruktor müssen alle Felder ohne Standardwert gesetzt sein, bevor `this` anders als zum Setzen von Feldern verwendet wird. | Sonst gäbe es halb initialisierte Objekte. |
| V-11 | Eine Exception, die einen Destruktor verlässt, beendet das Programm. | Wie C++; vermeidet doppelte Ausnahmen beim Unwinding. |
| V-12 | (angenommen am 21.11.2026) Klarstellung zu R-5.15 und R-5.16 für die Oberfläche: Ein Zweig, der nicht normal endet (Kapitel 6, Abschnitt 3.3: `return`, `throw`, `break`, `continue`, `fail`), trägt zum Zustand nach dem `if` nichts bei; ebenso bei `switch`, Schleifen und `try`. Nach `if (fehler) { verbrauche(a); return; } a.run();` ist `a` verfügbar. Ein `break` bringt seinen Zustand an das Ende der Schleife, ein `continue` an den Schleifenkopf. | FR++ hat kein `return` und kein `throw`, deshalb sagt R-5.15 dazu nichts. Die Regel ist dieselbe wie für die Verengung (R-6.8) und wie in Rust; zur Laufzeit gibt es auf dem fortgesetzten Weg keinen Move, also auch kein Drop-Flag-Problem. Ohne sie wäre jeder Move vor einem frühen Ausstieg ein Fehler. |
| V-13 | (angenommen am 21.11.2026) Ein `own`-Wert an einem `weak`-Ziel (Variable, Feld, Parameter), etwa `weak Foo w = new Foo();` oder `n.w = a;`, ist ein Fehler (R-4.41): `weak` entsteht nur aus `shared` oder `weak` (V-6). Man schreibt `shared Foo s = a; weak Foo w = s;`. FR++ `demote share e` entspricht an der Oberfläche diesem Umweg über eine `shared`-Variable. | Das Objekt wäre sonst sofort freigegeben und der `weak`-Verweis tot; so etwas ist fast immer ein Versehen. Beim Vergleich mit dem FR++-Modell aufgefallen. |
| V-14 | (angenommen am 22.11.2026) Klarstellung zu V-7 bei Vererbung: Zuerst laufen alle Destruktoren, von der Klasse des Objekts bis zur obersten Basisklasse; danach werden alle Felder in umgekehrter Deklarationsreihenfolge freigegeben, die Felder der Unterklasse zuerst, die der Basisklasse zuletzt. Wirft ein Konstruktor (R-9.13), laufen die Destruktoren des fertigen Basisanteils, danach werden alle schon gesetzten Felder ebenso freigegeben. Während der Destruktoren ist das Objekt vollständig; virtuelle Aufrufe gehen an die Klasse des Objekts. | So verlangen es die Laufzeittests `l02_destruktoren.rpp` (Kapitel 7) und `l03_konstruktor_wirft.rpp` (Kapitel 9); die andere Lesart (Destruktor und Felder je Klasse abwechselnd) widerspricht dem zweiten. |
| V-15 | (angenommen am 22.11.2026) Ein `own`-Objekt einer Klasse ohne Destruktor lässt sich in seine Felder zerlegen: `(K k, V v) = e;` (R-5.25). Das Objekt wird dabei verschoben und ohne Destruktor freigegeben; seine Felder gehen an die neuen Variablen über. V-3 bleibt: Aus einem Feld wird weiter nie einzeln verschoben. Damit lässt sich etwa `V? Map.remove(K key)` in R++ schreiben. | Moves aus einzelnen Feldern (wie Rust) hätten halb verschobene Objekte, Drop-Flags je Feld und erweiterte Invarianten verlangt; das Zerlegen braucht nur eine neue Anweisung, `Γ` und die Invarianten bleiben unverändert. Ein Destruktor sähe ein Objekt ohne Felder, deshalb gilt die Regel nur für Klassen ohne Destruktor. |

## 3. Die Kernsprache FR++

### 3.1 Syntax

```
Programm     P ::= K₁ … Kₙ
Klasse       K ::= class C : D { F₁ … Fₙ  M₁ … Mₖ  destructor() { s̄ } }
Feld         F ::= S f;
Methode      M ::= S m(T₁ x₁, …, Tₙ xₙ) μ { s̄ return e; }
               |  brw(κ, C') m(T₁ x₁, …, Tₙ xₙ) μ { s̄ return &κ q; }    liefert einen Borrow (R-5.23)
Modus        μ ::= ro | mut

Speichertyp  S ::= int | own C | shared C | weak C
Parametertyp T ::= S | ref C | mut C
Borrow-Art   κ ::= ro | mut

Ort          p ::= x | p.f
Ausdruck     e ::= n                     Zahl (steht für alle primitiven Werte und string)
               |  copy p                 Wert kopieren (nur int, shared, weak)
               |  move x                 Besitz übergeben
               |  new C(e₁, …, eₙ)       Objekt erzeugen, ein Wert pro Feld
               |  share e                own C  →  shared C
               |  demote e               shared C  →  weak C
               |  replace(p.f, e)        Feld neu setzen, alter Wert ist das Ergebnis
               |  p.m(b₁, …, bₙ)         Methodenaufruf
Argument     b ::= e | &κ p
Anweisung    s ::= S x = e;              Deklaration
               |  x = e;  |  p.f = e;    Zuweisung, alter Wert wird freigegeben
               |  e;                     Ergebnis wird freigegeben
               |  if (p) { s̄ } else { s̄ }
               |  while (p) { s̄ }
               |  borrow x = &κ p { s̄ }              Borrow mit Block
               |  upgrade x = p { s̄ } else { s̄ }     weak → shared, falls lebendig
               |  borrow x = p.m(b̄) { s̄ }            Borrow-Rückgabe binden (R-5.24)
               |  (S₁ x₁, …, Sₙ xₙ) = move y;          own-Objekt zerlegen (R-5.25)
```

Entsprechung zur Oberfläche: `ref C` ist ein Parameter `Foo`, `mut C` ein Parameter `Foo&`, `own C` eine lokale Variable `Foo` oder ein Parameter `own Foo`; `μ = mut` ist eine Methode mit `&` hinter der Parameterliste. Felder haben nur Speichertypen; dass Borrows nicht in Feldern stehen, ist damit schon syntaktisch erzwungen.

Annahmen: Alle Variablennamen eines Methodenrumpfs sind verschieden (sonst umbenennen). `D` ist die Basisklasse, `Object` hat keine Felder und Methoden. `felder(C)` und `methode(m, C)` liefern wie in Featherweight Java die geerbten und eigenen Felder bzw. die Signatur.

### 3.2 Hilfsbegriffe

**Umgebung.** `Γ` ordnet jeder Variablen einen Typ und einen Zustand zu: `Γ(x) = (τ, z)` mit `τ` ein Speicher- oder Parametertyp und `z ∈ {●, ○}` (verfügbar, verschoben). Für Ortsprüfungen gilt nur `●`.

**Wurzel und Präfix.** `wurzel(p)` ist die Variable am Anfang von `p`. `p ⊑ q` heißt: `p` ist Präfix von `q` (`x.a ⊑ x.a.b`).

**Objektorte.** `obj(p) = C` gilt, wenn `Γ ⊢ p : τ` und
- `τ ∈ {own C, ref C, mut C}`, oder
- `τ = shared C` und `p` ist eine Variable (V-2).

Nur über Objektorte darf man weiter in Felder gehen oder einen Borrow nehmen.

**Ortstypisierung** `Γ ⊢ p : τ`:

```
(O-Var)   Γ(x) = (τ, ●)                ⟹   Γ ⊢ x : τ
(O-Feld)  obj(p) = C    S f ∈ felder(C)  ⟹   Γ ⊢ p.f : S
```

**Schreibrecht.** `schreibbar(p)` gilt, wenn `Γ(wurzel(p))` nicht den Typ `ref C` hat.

**Loans.** `L` ist eine Menge von Paaren `(κ, p)`: In diesem Rahmen besteht gerade ein Borrow der Art `κ` auf den Ort `p`.

**Überlappung.** `p ⋈ q` gilt, wenn `wurzel(p) = wurzel(q)` und
- `p ⊑ q` oder `q ⊑ p`, oder
- die gemeinsame Wurzel hat den Typ `shared C` (ein `shared`-Objekt wird als Ganzes gesperrt, siehe 4.3).

**Erlaubter Zugriff.** `erlaubt_L(κ, p)` gilt, wenn für alle `(κ', q) ∈ L` gilt: `¬(p ⋈ q)` oder `κ = κ' = ro`.

Lesen ist ein Zugriff der Art `ro`; Schreiben, Move und `&mut` sind Zugriffe der Art `mut`.

**Vereinigung von Zuständen.** `Γ₁ ⊔ Γ₂` ist punktweise `● ⊔ ● = ●`, sonst `○`. `Γ₁ ⊒ Γ₂` heißt: Jede in `Γ₂` verfügbare Variable ist auch in `Γ₁` verfügbar. `Γ|V` schränkt auf die Variablen `V` ein.

## 4. Statische Semantik

Urteile:

- `Γ; L ⊢ e : S ⊣ Γ'` – Ausdruck `e` hat Typ `S`; danach gilt `Γ'` (Moves sind eingetragen).
- `Γ; L ⊢ b : T ⊣ Γ'; L'` – Argument `b` passt zu Parametertyp `T`; `L'` enthält zusätzlich den Loan für die Dauer des Aufrufs.
- `Γ; L ⊢ s̄ ⊣ Γ'` – Anweisungsfolge.

Jede Regel trägt die Nummer, unter der sie in der Spezifikation und der Testsuite geführt wird.

### 4.1 Ausdrücke

```
R-5.1 (T-Zahl)
    ─────────────────────────
    Γ; L ⊢ n : int ⊣ Γ

R-5.2 (T-Kopie)
    Γ ⊢ p : S    S ∈ {int, shared C, weak C}    erlaubt_L(ro, p)
    ─────────────────────────────────────────────────────────────
    Γ; L ⊢ copy p : S ⊣ Γ

R-5.3 (T-Move)
    Γ(x) = (S, ●)    S ∈ {own C, shared C}    erlaubt_L(mut, x)
    ─────────────────────────────────────────────────────────────
    Γ; L ⊢ move x : S ⊣ Γ[x ↦ (S, ○)]

R-5.4 (T-Neu)
    felder(C) = S₁ f₁ … Sₙ fₙ    Γ₀ = Γ    Γᵢ₋₁; L ⊢ eᵢ : Sᵢ ⊣ Γᵢ  (i = 1…n)
    ──────────────────────────────────────────────────────────────────────────
    Γ; L ⊢ new C(e₁, …, eₙ) : own C ⊣ Γₙ

R-5.5 (T-Share)
    Γ; L ⊢ e : own C ⊣ Γ'
    ──────────────────────────────
    Γ; L ⊢ share e : shared C ⊣ Γ'

R-5.6 (T-Demote)
    Γ; L ⊢ e : shared C ⊣ Γ'
    ──────────────────────────────
    Γ; L ⊢ demote e : weak C ⊣ Γ'

R-5.7 (T-Ersetzen)
    Γ; L ⊢ e : S ⊣ Γ'    Γ' ⊢ p.f : S    schreibbar(p.f)    erlaubt_L(mut, p.f)
    ───────────────────────────────────────────────────────────────────────────
    Γ; L ⊢ replace(p.f, e) : S ⊣ Γ'
```

Bei R-5.3 kann nur eine Variable mit Speichertyp verschoben werden. Parameter vom Typ `ref C` oder `mut C` und `this` haben keinen Speichertyp und sind damit ausgeschlossen. Aus Feldern wird nie verschoben (V-3).

Bei R-5.7 wird `e` zuerst geprüft und erst danach der Ort; `replace(x.f, f(move x))` ist deshalb ein Fehler.

### 4.2 Aufrufe und Argumente

```
R-5.8 (T-Arg-Wert)
    Γ; L ⊢ e : S ⊣ Γ'
    ─────────────────────────
    Γ; L ⊢ e : S ⊣ Γ'; L

R-5.9 (T-Arg-Borrow)
    obj(p) = C    erlaubt_L(κ, p)    κ = mut ⟹ schreibbar(p)
    ──────────────────────────────────────────────────────────
    Γ; L ⊢ &κ p : brw(κ, C) ⊣ Γ; L ∪ {(κ, p)}
          mit brw(ro, C) = ref C,  brw(mut, C) = mut C

R-5.10 (T-Aufruf)
    obj(p) = C    methode(m, C) = S m(T₁, …, Tₙ) μ
    erlaubt_L(μ, p)    μ = mut ⟹ schreibbar(p)
    Γ₀ = Γ    L₀ = L ∪ {(μ, p)}
    Γᵢ₋₁; Lᵢ₋₁ ⊢ bᵢ : Tᵢ ⊣ Γᵢ; Lᵢ   (i = 1…n)
    ───────────────────────────────────────────────
    Γ; L ⊢ p.m(b₁, …, bₙ) : S ⊣ Γₙ
```

Der Empfänger und die Argumente werden von links nach rechts geprüft; jeder Borrow bleibt bis zum Ende des Aufrufs in der Loan-Menge. Damit ist ausgeschlossen:

- `p.m(&mut p.f)` bei einer `&`-Methode `m` (Empfänger und Argument überlappen),
- `f(&mut a, &a)` und `f(&mut a, &mut a)`,
- `f(move a, &a)` (nach dem Move ist `a` nicht mehr verfügbar) und `f(&a, move a)` (Move während eines Loans).

Erlaubt ist `swap(&mut p.links, &mut p.rechts)`, weil sich die Orte nicht überlappen.

### 4.3 Anweisungen

```
R-5.11 (T-Dekl)
    x ∉ dom(Γ)    Γ; L ⊢ e : S ⊣ Γ'
    ─────────────────────────────────────────
    Γ; L ⊢ S x = e; ⊣ Γ', x ↦ (S, ●)

R-5.12 (T-Zuw-Var)
    Γ(x) = (S, z)    Γ; L ⊢ e : S ⊣ Γ'    erlaubt_L(mut, x)
    ─────────────────────────────────────────────────────────
    Γ; L ⊢ x = e; ⊣ Γ'[x ↦ (S, ●)]

R-5.13 (T-Zuw-Feld)
    Γ; L ⊢ replace(p.f, e) : S ⊣ Γ'
    ─────────────────────────────────
    Γ; L ⊢ p.f = e; ⊣ Γ'

R-5.14 (T-Ausdr)
    Γ; L ⊢ e : S ⊣ Γ'
    ─────────────────────
    Γ; L ⊢ e; ⊣ Γ'

R-5.15 (T-If)
    Γ ⊢ p : int    erlaubt_L(ro, p)    Γ; L ⊢ s̄₁ ⊣ Γ₁    Γ; L ⊢ s̄₂ ⊣ Γ₂
    ──────────────────────────────────────────────────────────────────────
    Γ; L ⊢ if (p) { s̄₁ } else { s̄₂ } ⊣ (Γ₁ ⊔ Γ₂)|dom(Γ)

R-5.16 (T-While)
    Γ ⊢ p : int    erlaubt_L(ro, p)    Γ; L ⊢ s̄ ⊣ Γ₁    Γ₁|dom(Γ) ⊒ Γ
    ────────────────────────────────────────────────────────────────────
    Γ; L ⊢ while (p) { s̄ } ⊣ Γ

R-5.17 (T-Borrow)
    obj(p) = C    erlaubt_L(κ, p)    κ = mut ⟹ schreibbar(p)
    Γ, x ↦ (brw(κ, C), ●);  L ∪ {(κ, p)} ⊢ s̄ ⊣ Γ'
    ──────────────────────────────────────────────────────────
    Γ; L ⊢ borrow x = &κ p { s̄ } ⊣ Γ'|dom(Γ)

R-5.18 (T-Upgrade)
    Γ ⊢ p : weak C    erlaubt_L(ro, p)
    Γ, x ↦ (shared C, ●); L ⊢ s̄₁ ⊣ Γ₁    Γ; L ⊢ s̄₂ ⊣ Γ₂
    ──────────────────────────────────────────────────────
    Γ; L ⊢ upgrade x = p { s̄₁ } else { s̄₂ } ⊣ (Γ₁|dom(Γ)) ⊔ Γ₂

R-5.19 (T-Folge)
    Γ; L ⊢ s ⊣ Γ''    Γ''; L ⊢ s̄ ⊣ Γ'
    ─────────────────────────────────────          Γ; L ⊢ ε ⊣ Γ
    Γ; L ⊢ s s̄ ⊣ Γ'
```

Anmerkungen:

- R-5.12 erlaubt die Zuweisung an eine verschobene Variable; sie ist danach wieder verfügbar. `x = f(move x)` ist zulässig.
- Nach R-5.15 gilt eine Variable als verschoben, wenn sie in mindestens einem Zweig verschoben wurde (V-5).
- R-5.16 verlangt, dass am Ende des Schleifenrumpfs alles wieder verfügbar ist, was vorher verfügbar war. Ein Move in der Schleife ist nur zulässig, wenn die Variable im selben Durchlauf neu belegt wird.
- In R-5.17 ist während des Blocks jeder Zugriff auf `p` verboten, der dem Loan widerspricht: bei `&mut` jeder Zugriff, bei `&ro` jedes Schreiben und jeder Move – auch auf Präfixe von `p`.
- Ist die Wurzel von `p` eine `shared`-Variable, überlappen nach 3.2 alle Orte mit dieser Wurzel. Was zur Laufzeit ein Sperrkonflikt wäre, ist damit innerhalb einer Methode schon ein Compilerfehler.

### 4.4 Methoden und Klassen

```
R-5.20 (T-Methode)
    Γ₀ = this ↦ (brw(μ, C), ●), x₁ ↦ (T₁, ●), …, xₙ ↦ (Tₙ, ●)
    Γ₀; ∅ ⊢ s̄ ⊣ Γ₁    Γ₁; ∅ ⊢ e : S ⊣ Γ₂
    ──────────────────────────────────────────────────────────
    S m(T₁ x₁, …, Tₙ xₙ) μ { s̄ return e; }  ok in C

R-5.21 (T-Destruktor)
    this ↦ (mut C, ●); ∅ ⊢ s̄ ⊣ Γ'
    ────────────────────────────────
    destructor() { s̄ }  ok in C

R-5.22 (T-Überschreiben)
    methode(m, D) = S m(T̄) μ  ⟹  die Methode m in C hat genau die Signatur S m(T̄) μ
    ────────────────────────────────────────────────────────────────────────────────
    class C : D { … }  ok

R-5.23 (T-Borrow-Methode)
    Γ₀ wie in R-5.20    Γ₀; ∅ ⊢ s̄ ⊣ Γ₁
    wurzel(q) = this    Γ₁ ⊢ obj(q) = C''    C'' <: C'    κ = mut ⟹ μ = mut
    ─────────────────────────────────────────────────────────────────────────
    brw(κ, C') m(T₁ x₁, …, Tₙ xₙ) μ { s̄ return &κ q; }  ok in C

R-5.25 (T-Zerlegen)
    Γ(y) = (own C, ●)    erlaubt_L(mut, y)    keine Klasse in C und ihren Basisklassen hat einen Destruktor
    felder(C) = S'₁ f₁ … S'ₙ fₙ    S'ᵢ <: Sᵢ    x₁ … xₙ ∉ dom(Γ), paarweise verschieden
    ──────────────────────────────────────────────────────────────────────────────────────
    Γ; L ⊢ (S₁ x₁, …, Sₙ xₙ) = move y; ⊣ Γ[y ↦ (own C, ○)], x₁ ↦ (S₁, ●), …, xₙ ↦ (Sₙ, ●)

R-5.24 (T-Borrow-Aufruf)
    p.m(b̄) erfüllt die Voraussetzungen von R-5.10 mit Ergebnis brw(κ, C') ⊣ Γ'
    x ∉ dom(Γ')    Γ', x ↦ (brw(κ, C'), ●);  L ∪ {(μ, p)} ⊢ s̄ ⊣ Γ''
    ───────────────────────────────────────────────────────────────────
    Γ; L ⊢ borrow x = p.m(b̄) { s̄ } ⊣ Γ''|dom(Γ')
```

R-5.10 gilt nur für Methoden mit Speichertyp als Rückgabe; eine Borrow-Methode kann nur über R-5.24 aufgerufen werden. Der zurückgegebene Borrow darf nur von `this` ausgehen (R-5.23). Deshalb genügt an der Aufrufstelle ein Loan auf den Empfänger `p`, und zwar mit dem Modus `μ` der Methode (bei `κ = mut` ist `μ = mut`). Solange er besteht, verbietet `erlaubt_L` jede Änderung an `p` und an allem, was über `own`-Kanten von `p` erreichbar ist; damit bleibt auch das zurückgegebene Objekt am Leben (Lemma 5.4). Einen Borrow auf einen Parameter zurückzugeben, ist verboten: Den Parameter schützt an der Aufrufstelle kein Loan, der über den Aufruf hinaus gilt (Gegenbeispiel `n20_borrow_von_parameter.fr`).

Untertypen: Aus `C <: D` folgt `own C <: own D`, `shared C <: shared D`, `weak C <: weak D`, `ref C <: ref D` und `mut C <: mut D`. Das letzte ist sicher, weil man über einen `mut`-Borrow nur Felder setzen, aber nie das ganze Objekt ersetzen kann.

## 5. Dynamische Semantik

### 5.1 Laufzeitzustand

```
Wert       v ::= n | own ℓ | sh ℓ | wk ℓ | ref ℓ | ⊥
Heap       H : ℓ ↦ ⟨C, F, k⟩  oder  †
                F : Feldname ↦ Wert
                k = –                bei Objekten mit eindeutigem Besitzer
                k = ⟨rc, β⟩          bei shared-Objekten; rc ∈ ℕ, β ∈ {frei, lesend(n), schreibend}
Rahmen     E : Variable ↦ Wert
```

`†` markiert ein freigegebenes Objekt. Adressen werden im Modell nicht wiederverwendet; eine echte Implementierung hält dafür einen Kontrollblock, solange `weak`-Verweise bestehen. `⊥` ist der Wert einer verschobenen Variablen und dient zugleich als Drop-Flag.

Ein **Fehlerzustand** ist erreicht, wenn eine Auswertung auf eine Adresse `ℓ` mit `H(ℓ) = †` zugreift, eine solche Adresse freigibt oder einen Ort mit Wert `⊥` liest. Das Kalkül hat dafür keine Regel; die Auswertung bleibt stecken.

`⚡` ist das Ergebnis eines Sperrkonflikts (Exception `BorrowError`). In FR++ gibt es kein `catch`; `⚡` bricht die Auswertung ab und ist ein sicheres Ergebnis.

### 5.2 Orte

`ziel(p)` liefert die Adresse des Objekts an einem Objektort:

- `ziel(x) = ℓ`, wenn `E(x) ∈ {own ℓ, sh ℓ, ref ℓ}` und `H(ℓ) ≠ †`.
- `ziel(p.f) = ℓ'`, wenn `H(ziel(p)).F(f) = own ℓ'` und `H(ℓ') ≠ †`.

`wert(x) = E(x)`, `wert(p.f) = H(ziel(p)).F(f)`.

**Prüfung über shared-Wurzeln.** Ist `E(wurzel(p)) = sh ℓ₀`, prüft jeder Zugriff der Art `κ` auf `p` den Sperrzustand `β` von `ℓ₀`: `ro` verlangt `β ≠ schreibend`, `mut` verlangt `β = frei`. Andernfalls ist das Ergebnis `⚡`. Bei Wurzeln anderer Art findet keine Prüfung statt.

**Sperren.** `sperre(p, κ)`: Ist `E(wurzel(p)) = sh ℓ₀`, wird nach der Prüfung `β` von `frei` oder `lesend(n)` auf `lesend(n+1)` (bei `ro`) bzw. von `frei` auf `schreibend` (bei `mut`) gesetzt. `freigabe(p, κ)` macht das rückgängig. Bei anderen Wurzeln tun beide nichts.

### 5.3 Freigabe

`drop(v)`:

| Wert | Wirkung |
|---|---|
| `n`, `ref ℓ`, `wk ℓ`, `⊥` | nichts |
| `own ℓ` | Destruktor von `ℓ` mit `this = ref ℓ` ausführen, dann `drop` auf alle Felder in umgekehrter Reihenfolge, dann `H(ℓ) := †` |
| `sh ℓ` | `rc` um 1 verringern; wird `rc = 0`: wie bei `own ℓ` |

Solange der Destruktor eines `shared`-Objekts läuft, ist `rc = 0`; `upgrade` schlägt dann fehl.

### 5.4 Auswertung

Big-Step-Urteile `H, E ⊢ e ⇓ v ⊣ H', E'` und `H, E ⊢ s̄ ⇓ H', E'`; jede Regel kann stattdessen `⚡` liefern, das unverändert weitergereicht wird.

| Regel | Konstrukt | Auswertung |
|---|---|---|
| E-Zahl | `n` | Ergebnis `n` |
| E-Kopie | `copy p` | Lesezugriff auf `p` prüfen; `v = wert(p)`; bei `v = sh ℓ` wird `rc` um 1 erhöht |
| E-Move | `move x` | `v = E(x)`; `E(x) := ⊥` |
| E-Neu | `new C(ē)` | Argumente von links nach rechts zu `v̄`; neue Adresse `ℓ`; `H(ℓ) := ⟨C, f̄ ↦ v̄, –⟩`; Ergebnis `own ℓ` |
| E-Share | `share e` | `e ⇓ own ℓ`; `k(ℓ) := ⟨1, frei⟩`; Ergebnis `sh ℓ` |
| E-Demote | `demote e` | `e ⇓ sh ℓ`; Ergebnis `wk ℓ`; danach `drop(sh ℓ)` |
| E-Ersetzen | `replace(p.f, e)` | `e ⇓ v`; Schreibzugriff auf `p.f` prüfen; `v_alt = wert(p.f)`; Feld auf `v` setzen; Ergebnis `v_alt` |
| E-Aufruf | `p.m(b̄)` | `sperre(p, μ)`; `ℓ = ziel(p)`; Argumente von links nach rechts: Ausdrücke zu Werten, `&κ q` zu `ref ziel(q)` nach `sperre(q, κ)`; Methode nach der Klasse von `H(ℓ)` wählen; neuer Rahmen `this ↦ ref ℓ, xᵢ ↦ vᵢ`; Rumpf ausführen, `return`-Ausdruck zu `v`; alle Variablen des Rahmens in umgekehrter Reihenfolge `drop`; alle Sperren des Aufrufs in umgekehrter Reihenfolge freigeben; Ergebnis `v` |
| E-Dekl | `S x = e;` | `e ⇓ v`; `E(x) := v` |
| E-Zuw-Var | `x = e;` | `e ⇓ v`; `v_alt = E(x)`; `E(x) := v`; `drop(v_alt)` |
| E-Zuw-Feld | `p.f = e;` | `replace(p.f, e) ⇓ v_alt`; `drop(v_alt)` |
| E-Ausdr | `e;` | `e ⇓ v`; `drop(v)` |
| E-Zerlegen | `(S̄ x̄) = move y;` | `E(y) = own ℓ`, `H(ℓ) = ⟨C, f̄ ↦ v̄, –⟩`; `E(xᵢ) := vᵢ`; `E(y) := ⊥`; `H(ℓ) := †` ohne Destruktor (es gibt keinen) |
| E-If, E-While | | wie üblich; Bedingung `≠ 0` |
| E-Borrow | `borrow x = &κ p { s̄ }` | `sperre(p, κ)`; `E(x) := ref ziel(p)`; Block ausführen; `freigabe(p, κ)` |
| E-Borrow-Aufruf | `borrow x = p.m(b̄) { s̄ }` | wie E-Aufruf, aber die Sperre `sperre(p, μ)` des Empfängers bleibt bis zum Ende des Blocks bestehen; der Rumpf endet mit `return &κ q`, das zu `ref ziel(q)` ausgewertet wird; `E(x) := ref ℓ'`; Block ausführen; dann `freigabe(p, μ)` |
| E-Upgrade | `upgrade x = p { s̄₁ } else { s̄₂ }` | `wert(p) = wk ℓ`; ist `H(ℓ) ≠ †` und `rc(ℓ) > 0`: `rc` um 1 erhöhen, `E(x) := sh ℓ`, Block `s̄₁`, danach `drop(E(x))`; sonst Block `s̄₂` |
| E-Block | jeder Block | am Ende alle im Block deklarierten Variablen in umgekehrter Reihenfolge `drop` (bei `⊥` nichts) |

Bei Zuweisungen wird erst der neue Wert eingesetzt und dann der alte freigegeben. Ein Destruktor, der dabei läuft, sieht deshalb nie ein Objekt mit einem gerade freigegebenen Feld.

## 6. Sicherheit

### 6.1 Sätze

**Satz 5.1 (Speichersicherheit).** Ist ein FR++-Programm wohlgetypt, dann erreicht seine Auswertung keinen Fehlerzustand. Sie liefert ein Ergebnis, endet mit `⚡` oder läuft unendlich.

Folgerungen: Kein Objekt wird nach der Freigabe benutzt (kein Use-after-free), keines wird doppelt freigegeben (kein Double-free), und der Destruktor jedes Objekts läuft höchstens einmal.

**Satz 5.2 (Iteratoren).** Solange ein Borrow auf ein Objekt besteht, wird kein über `own`-Kanten von ihm erreichbares Objekt freigegeben. Für Vektoren (Abschnitt 7) heißt das: Während einer Iteration oder solange ein Slice besteht, wächst oder schrumpft der Vektor nicht.

**Satz 5.3 (Keine Laufzeitprüfung ohne shared).** Enthält ein wohlgetyptes Programm keine Typen `shared C` und `weak C`, kommt `⚡` nicht vor.

Satz 5.3 folgt direkt daraus, dass nur Zugriffe über `shared`-Wurzeln geprüft werden.

Nicht Teil der Garantie ist, dass jedes Objekt freigegeben wird: Zyklen aus `shared`-Verweisen werden nie freigegeben (Leck, aber kein Fehler).

### 6.2 Beweisskizze

Der Beweis zeigt, dass folgende Invarianten bei jedem Schritt erhalten bleiben. Er ist hier nur skizziert; ein vollständiger, möglichst maschinell geprüfter Beweis (Coq, Lean oder Isabelle) steht aus. Das ausführbare Modell (Abschnitt 10) prüft I-1, I-2, I-4 und I-5 nach jeder Anweisung.

- **I-1 Eindeutiger Besitz.** Für jedes lebende Objekt `ℓ` mit `k = –` gibt es genau einen Wert `own ℓ`, und zwar entweder in einem Rahmen oder in einem Feld eines lebenden Objekts. Für jedes lebende `shared`-Objekt gibt es keinen Wert `own ℓ`, und `rc(ℓ)` ist die Zahl der Werte `sh ℓ` in Rahmen und lebenden Feldern.
- **I-2 Lebende Besitzer.** Jeder Wert `own ℓ` oder `sh ℓ` in einem Rahmen oder in einem lebenden Feld verweist auf ein lebendes Objekt.
- **I-3 Übereinstimmung von Γ und E.** Gilt statisch `Γ(x) = (τ, ●)`, so ist `E(x) ≠ ⊥`.
- **I-4 Gerechtfertigte Borrows.** Jeder Wert `ref ℓ` in einem Rahmen geht auf einen aktiven Loan `(κ, p)` in einem tieferen Rahmen zurück, mit `ziel(p) = ℓ` zum Zeitpunkt des Borrows.
- **I-5 Gesperrte Objekte leben.** Ist `β(ℓ) ≠ frei`, so hält eine `shared`-Variable in einem Rahmen das Objekt, also `rc(ℓ) ≥ 1`.

Kern des Beweises ist:

**Lemma 5.4 (Pfadstabilität).** Solange in einem Rahmen ein Loan `(κ, p)` aktiv ist, ändert sich keine Besitzkante auf dem Weg von `wurzel(p)` zu `ziel(p)` und keine Besitzkante unterhalb von `ziel(p)` – außer über den Borrow selbst, wenn `κ = mut`.

Begründung nach Art der Wurzel von `p`:

- *Lokale `own`-Variable:* Nach I-1 erreicht kein anderer Rahmen die Objekte auf dem Weg, außer über Borrows, die derselbe Rahmen ausgegeben hat. Alle diese Borrows sind wegen `erlaubt_L` mit dem Loan verträglich, und im Rahmen selbst verbietet `erlaubt_L` jedes Schreiben, jeden Move und jeden widersprechenden Borrow.
- *Borrow-Variable (`this`, Parameter, Borrow-Block):* wie oben, und für den Weg bis `ziel(wurzel(p))` gilt das Lemma per Induktion für den Loan im aufrufenden Rahmen.
- *`shared`-Variable:* Andere Rahmen können das Objekt über eigene `shared`-Verweise erreichen. Jeder Zugriff über eine `shared`-Wurzel prüft aber den Sperrzustand (5.2), und der Loan hält eine passende Sperre. Ein `mut`-Loan schließt jeden fremden Zugriff aus, ein `ro`-Loan jedes fremde Schreiben. Wegen I-5 kann der Zähler während des Loans nicht auf 0 fallen.

Destruktoren sind dabei gewöhnliche Aufrufe: Ein Destruktor erhält nur `this` und erreicht sein eigenes Objekt auf keinem anderen Weg, weil der Besitzer es schon aufgegeben hat (I-1) bzw. `rc = 0` ist und `upgrade` fehlschlägt.

Für R-5.25 bleibt I-1 erhalten: Die Werte der Felder gehen aus dem freigegebenen Objekt in den Rahmen über, jeder Besitzwert steht danach wieder genau einmal; `y` ist `⊥` (I-3), und weil die Klasse keinen Destruktor hat, sieht kein Code das leere Objekt. Ein Borrow auf `y` oder einen Ort darunter kann nicht bestehen, weil `erlaubt_L(mut, y)` verlangt ist (I-4).

Aus Lemma 5.4, I-2 und I-4 folgt, dass `ziel(p)` für jeden geprüften Ort definiert ist. Aus I-3 folgt, dass nie `⊥` gelesen wird. Weil `drop(own ℓ)` nur aufgerufen wird, wenn der einzige Wert `own ℓ` gerade aus Rahmen oder Feld entfernt wurde (I-1), wird nichts doppelt freigegeben. Das ergibt Satz 5.1; Satz 5.2 ist Lemma 5.4 für den Loan der Iteration bzw. des Slices.

## 7. Abbildung der Oberflächensprache

| Oberfläche | FR++ | Regeln |
|---|---|---|
| Lokale Variable `Foo x = e;` | `own C x = e;` | R-5.11 |
| Verwendung von `x` (Typ `own`) als Wert, Feldwert oder `own`-Argument | `move x` | R-5.3 |
| Verwendung von `x` als Argument für Parameter `Foo` | `&ro x` | R-5.9 |
| Argument `&x` für Parameter `Foo&` | `&mut x` | R-5.9 |
| Parameter `Foo` / `Foo&` / `own Foo` / `shared Foo` | `ref C` / `mut C` / `own C` / `shared C` | R-5.20 |
| Methode mit `&` nach der Parameterliste | `μ = mut` | R-5.10, R-5.20 |
| `shared Foo s = f;` mit `f` vom Typ `own` | `shared C s = share move f;` | R-5.5, V-9 |
| `shared Foo t = s;` mit `s` vom Typ `shared` | `shared C t = copy s;` | R-5.2 |
| `weak Foo w = s;` | `weak C w = demote copy s;` | R-5.6 |
| `shared Foo? p = w.lock(); if (p != null) { … }` | `upgrade p = w { … } else { }` | R-5.18, V-6 |
| `this.next.name()` mit `shared`-Feld `next` | `shared C t = copy this.next; t.name();` (Hilfsvariable des Compilers) | V-2 |
| `this.payload = p;` | `this.payload = move p;` | R-5.13 |
| `Foo alt = replace(&this.payload, neu);` | `own C alt = replace(this.payload, …);` | R-5.7, V-3 |
| `Foo? alt = take(&this.cache);` (nur bei nullbarem Feld) | wie `replace(this.cache, null)` | R-5.7, V-3 |
| `copy(a)` | Aufruf der Kopiermethode mit `&ro a`; Ergebnis `own C` | R-5.10 |
| `(K k, V v) = e;` mit `e` vom Typ einer Klasse | `(own K k, own V v) = move y;` mit `y` der lokalen Variablen bzw. einer Hilfsvariablen für einen temporären Wert; Felder in Deklarationsreihenfolge, geerbte zuerst; jedes Feld muss an der Stelle sichtbar sein (R-7.2) | R-5.25 |
| `new Foo(args)` mit Konstruktor | Konstruktor als Methode, die mit `new C(…)` endet; Felder vorher vollständig gesetzt | V-10 |
| `for (Foo f : list) { … }` | `borrow l = &ro list { … borrow f = &ro l[i] { … } … }` | R-5.17 |
| `for (Foo& f : &list) { … }` | wie oben mit `&mut` | R-5.17 |
| `Slice<T> s = v[1..3];` (lokal) | `borrow s = &ro v { Rest des Blocks }` | R-5.17 |
| `ref Foo f = p;` | `borrow f = &ro p { Rest des Blocks }` | R-5.17 |
| `ref Foo& g = &p;` | `borrow g = &mut p { Rest des Blocks }` | R-5.17 |
| Methode `ref V get(K k)` / `ref V& at(K k) &` | `brw(ro, V) get(…) ro` / `brw(mut, V) at(…) mut`; Rumpf endet mit `return this…;` bzw. `return &this…;` | R-5.23 |
| `ref V v = map.get(k);` | `borrow v = map.get(k) { Rest des Blocks }` | R-5.24 |
| `print(map.get(k).name);` (Ergebnis nicht gebunden) | `borrow tmp = map.get(k) { print(tmp.name); }`: Der Loan gilt bis zum Ende der Anweisung | R-5.24 |
| Parameter `Slice<T>& s` | `mut` auf den Vektor | R-5.9 |
| Normales Lambda als Argument | jede erfasste Variable wird für die Dauer des Aufrufs ausgeliehen, lesend oder veränderbar je nach Rumpf | R-5.9, R-5.10 |
| `own`-Lambda | `new Closure(…)` mit einem Feld pro Erfassung: primitive Werte und `shared` kopiert, `own` verschoben | R-5.4 |

**Vektoren und Indizes.** Ein Vektor besitzt seine Elemente wie ein Objekt seine Felder. `a[i]` ist ein Ort mit unbekanntem Index: `a[i]` und `a[j]` gelten immer als überlappend, `swap(&a[i], &a[j])` ist deshalb ein Fehler. Grenzverletzungen werfen eine Exception. Herausnehmen geht nur über `&`-Methoden wie `pop()` und `remove(i)`, die `own T` liefern.

**Exceptions.** Beim Unwinding werden Sperren freigegeben und die Variablen jedes verlassenen Blocks freigegeben wie am normalen Blockende; Drop-Flags (`⊥`) verhindern doppelte Freigaben. Verlässt eine Exception einen Destruktor, endet das Programm (V-11).

**Threads.** An einen anderen Thread geht nur ein `own`-Wert, dessen Besitzbaum keine `shared`- oder `weak`-Felder enthält, weil der Referenzzähler nicht atomar ist. Strings sind davon ausgenommen und brauchen einen atomaren Zähler.

**Primitive Borrows.** Parameter wie `i32& n` und von Lambdas veränderbar erfasste primitive Variablen funktionieren wie Borrows auf Objektorte; das Kalkül lässt sie zur Vereinfachung weg. Für die Sicherheit ändert sich nichts.

**Strings** verhalten sich im Kalkül wie `int`: unveränderlich, kopierbar, ohne Borrows.

## 8. Grenzen und mögliche Erweiterungen

- **Borrows zurückgeben (V-4).** Umgesetzt in R-5.23 und R-5.24, aber nur für Borrows, die von `this` ausgehen. Borrows auf Parameter zurückzugeben (wie `T& max(T& a, T& b)`) würde Lifetimes oder eine Regel verlangen, welche Argumente der Loan an der Aufrufstelle umfasst; das ist nicht Teil von Version 1.
- **Lebensdauer benannter Borrows.** Ein `ref`-Borrow und ein lokaler Slice gelten bis zum Blockende, nicht nur bis zur letzten Verwendung. `ref Foo f = list[0]; f.run(); list.push(x);` ist deshalb ein Fehler; man setzt einen eigenen Block. Eine Regel „bis zur letzten Verwendung“ (wie Rusts Non-Lexical Lifetimes) wäre bequemer, verlangt aber eine Datenflussanalyse und eine Erweiterung des Kalküls.
- **Feinere Sperren.** Ein `shared`-Objekt wird als Ganzes gesperrt; zwei Borrows auf verschiedene Felder desselben `shared`-Objekts sind deshalb ein Fehler.
- **Rückrufe auf shared-Objekte.** Ruft eine `&`-Methode eines `shared`-Objekts über einen zweiten Verweis eine Methode desselben Objekts auf, wirft das `BorrowError`. Das ist die bekannte Schwäche dieses Ansatzes (wie `RefCell` in Rust) und der Preis für V-1.
- **Lecks durch Zyklen** werden nicht verhindert; `weak` ist das Mittel dagegen.

## 9. Konformitätstests

Die Testprogramme liegen in `konformitaet/kap05_ownership/`. Programme in `akzeptiert/` müssen übersetzt werden, Programme in `abgelehnt/` müssen mit genau dem im Kopfkommentar genannten Fehler abgelehnt werden. Geprüft werden sie mit `rppc test` (erste Implementierung, Meilenstein M3).

| Regel | Akzeptiert | Abgelehnt |
|---|---|---|
| R-5.3 Move | `a01_moves.rpp` | `n01_verwendung_nach_move.rpp`, `n10_borrow_in_feld.rpp` |
| R-5.7 / R-5.13 Feld setzen, `replace`, `take` | `a02_felder.rpp` | `n08_move_aus_feld.rpp`, `n09_feld_zurueckgeben.rpp`, `n16_take_bei_nicht_nullbarem_feld.rpp` |
| R-5.9 / R-5.10 Argumente | `a03_borrows.rpp` | `n05_doppelter_mut_borrow.rpp`, `n06_move_und_borrow_im_aufruf.rpp`, `n07_empfaenger_ueberlappt.rpp` |
| R-5.15 If | `a01_moves.rpp` | `n02_bedingter_move.rpp` |
| R-5.16 While | `a01_moves.rpp` | `n03_move_in_schleife.rpp` |
| R-5.17 Borrow-Block (for, Slice) | `a03_borrows.rpp` | `n04_vektor_waechst_in_schleife.rpp`, `n11_vektor_waechst_bei_slice.rpp` |
| Schreibrecht | `a03_borrows.rpp` | `n12_schreiben_ueber_lesenden_borrow.rpp` |
| Überlappung bei shared-Wurzeln | `a04_shared_weak.rpp` | `n13_shared_wurzel_doppelt.rpp` |
| R-5.5 / R-5.6 / R-5.18 shared, weak | `a04_shared_weak.rpp` | `n14_shared_als_own.rpp` |
| Lambdas | `a05_lambdas.rpp` | `n15_move_aus_normalem_lambda.rpp` |
| V-13 `own` an `weak` | `a04_shared_weak.rpp` | `n21_own_an_weak.rpp` |
| R-5.25 Zerlegen | `a07_zerlegen.rpp`, Laufzeit `l01_zerlegen.rpp` | `n22_zerlegen_mit_destruktor.rpp`, `n23_zerlegen_falsche_feldzahl.rpp`, `n24_zerlegen_geliehen.rpp`, `n25_zerlegen_privat.rpp`, `n26_nach_zerlegen_benutzt.rpp` |
| `ref`-Deklaration (R-5.17) | `a06_ref.rpp` | `n17_ref_lebt_bis_blockende.rpp`, `n18_ref_auf_temporaeren_wert.rpp`, `n19_ref_lesend_veraendern.rpp`, `n20_ref_mut_und_direkter_zugriff.rpp` |

## 10. Ausführbares Modell

Im Verzeichnis `fr-modell/` liegt ein Typprüfer und Interpreter für FR++ (Python, ohne weitere Pakete). Er setzt die Regeln aus Abschnitt 3 bis 5 möglichst wörtlich um; die Fehlermeldungen nennen die Regelnummer. Für Testprogramme kommen `print(e);` und die int-Operatoren `+`, `-`, `<` hinzu. Bedienung und Grenzen beschreibt `fr-modell/README.md`, den letzten Lauf `fr-modell/testbericht.txt`.

### 10.1 Handgeschriebene Programme

- 10 Programme, die angenommen werden und ein festgelegtes Ergebnis liefern müssen: Moves mit Drop-Flags, `replace`, disjunkte Borrows, `shared`/`weak`/`upgrade`, Freigabereihenfolge nach V-7, Sperrkonflikt und lesende Mehrfachsperre nach V-1, `upgrade` im Destruktor, Zyklus als Leck, Borrow-Rückgabe (R-5.23, R-5.24).
- 24 Programme, die mit einer bestimmten Regel in einer bestimmten Zeile abgelehnt werden müssen.
- 11 davon sind **nachweislich unsicher**: Führt man sie ohne Typprüfung aus, laufen 10 in einen Fehlerzustand und eines endet mit `BorrowError`. Damit ist für die Move-Regel, die `if`-Vereinigung, die Schleifenregel, die Loan-Prüfung bei Aufrufen und Borrow-Blöcken, das Schreibrecht und die Borrow-Rückgabe (nur von `this`, Loan auf den Empfänger) belegt, dass sie echte Fehler verhindern.

### 10.2 Zufallstests

`fr-modell/fuzz.py` erzeugt Programme mit fester Klassentabelle und zufälligen Methodenrümpfen, Destruktoren und Hauptprogrammen, prüft sie und führt sie mit Invariantenprüfung aus. Seit dem 21.11.2026 enthält die Klassentabelle Borrow-Methoden, und der Generator erzeugt `borrow x = p.m(…) { … }`.

Lauf vom 21.11.2026, 2000 Programme:

| | Anzahl |
|---|---|
| angenommen | 644 |
| davon normal beendet | 384 |
| davon mit `BorrowError` (zulässig, V-1) | 94 |
| davon Schrittbudget erschöpft (zulässig) | 166 |
| **davon Fehlerzustand oder verletzte Invariante** | **0** |
| abgelehnt | 1356, davon 151 bei Ausführung tatsächlich im Fehlerzustand |

Abgedeckt waren unter anderem 155 954 Freigaben, 13 435 Sperren über `shared`-Wurzeln, 9 084 Borrow-Aufrufe und 110 erfolgreiche bzw. 5 138 fehlgeschlagene `upgrade`s. Gegen Satz 5.1 und 5.3 und gegen „ohne `shared` kein Leck“ gab es keinen Verstoß. Ein früherer Lauf ohne Borrow-Rückgabe (20.11.2026, 2000 Programme, 816 angenommen) war ebenfalls ohne Verstoß.

### 10.3 Gegenproben

Für jede Gegenprobe wird eine Regel des Typprüfers abgeschaltet und der Zufallstest mit 600 Programmen wiederholt:

| Abgeschaltet | Verstöße | Bewertung |
|---|---|---|
| Loan-Prüfung (`erlaubt_L`) | 5 (I-4: Borrow auf freigegebenes Objekt) | Regel nötig, vom Zufallstest erreicht |
| Zustand ○ nach Move | 12 (Lesen von ⊥) | Regel nötig, vom Zufallstest erreicht |
| Schreibrecht | 1 (I-4) | Regel nötig, vom Zufallstest erreicht; zusätzlich `n19_schreiben_ueber_alias.fr` |
| Loan auf den Empfänger in R-5.24 | 1 (I-4) | Regel nötig, vom Zufallstest erreicht; zusätzlich `n21_empfaenger_veraendert.fr` |
| Vereinigung in R-5.15 | 0 | Zufallstest erreicht das Muster nicht; Beleg durch `n02_bedingter_move.fr` |
| Schleifenbedingung in R-5.16 | 0 | Zufallstest erreicht das Muster nicht; Beleg durch `n03_move_in_schleife.fr` |
| Überlappung ganzer `shared`-Wurzeln | 0 | erwartet: Die Regel ist nicht sicherheitsrelevant, die Laufzeitsperre fängt alle Fälle ab (siehe 4.3); `n11_shared_wurzel_doppelt.fr` endet ohne sie mit `BorrowError` |

Dass Borrows nur von `this` ausgehen dürfen (R-5.23), lässt sich mit dem Zufallstest nicht abschalten, weil die Klassentabelle wohlgetypt sein muss; den Beleg liefert `n20_borrow_von_parameter.fr`.

### 10.4 Befunde

- Am Kalkül war nichts zu korrigieren. Typprüfer und Interpreter ließen sich aus den Regeln ohne zusätzliche Festlegungen bauen; das gilt auch für die Erweiterung um Borrow-Rückgaben.
- Die Annahme aus 4.3, dass die statische Überlappung ganzer `shared`-Wurzeln nur Laufzeitkonflikte vorwegnimmt und die Sicherheit nicht trägt, ist bestätigt.
- Lücke der Tests: Für Vereinigung und Schleifenregel hängt der Beleg an je einem handgeschriebenen Programm; der Zufallsgenerator erzeugt die nötigen Muster zu selten.
- Die Oberflächenregel V-13.6 (Borrow-Rückgabe über `ref`-Variablen und Schleifenelemente, die von `this` ausgehen) ist im Modell nicht abgebildet, weil FR++ `return` nur am Ende eines Methodenrumpfs kennt.
- Zufallstests finden Fehler, beweisen aber keine Korrektheit. Der Beweis von Satz 5.1 bleibt offen.

### 10.5 Vergleich mit der ersten Implementierung

`fr-modell/vergleich.py` übersetzt FR++-Programme nach der Tabelle in Abschnitt 7 rückwärts in R++ (`frpp/to_rpp.py`) und prüft, ob `rppc` sie genauso annimmt oder ablehnt wie das Modell. `move x` für eine int-, weak- oder shared-Variable ist an der Oberfläche eine Kopie (V-9); verglichen wird dann die Normalform mit `copy x`. Ohne Entsprechung an der Oberfläche bleiben Pfade durch `shared`-Felder (V-2, dort erlaubt), Zuweisungen an Parameter ohne `own` (R-8.19) und ungebundene Borrow-Rückgaben als Anweisung; diese Programme zählen gesondert.

Stand 21.11.2026 (`fr-modell/vergleichsbericht.txt`): Alle 32 übersetzbaren handgeschriebenen Programme und 4466 von 5000 Zufallsprogrammen (1564 angenommen, 2902 abgelehnt) werden gleich beurteilt; es gibt keine Abweichung. Zur Gegenprobe wird je eine Regel im Modell abgeschaltet (wie in 10.3); für jede der sieben Abschwächungen findet der Vergleich Abweichungen.

Der Vergleich hat in `rppc` sechs Lücken gefunden, die behoben sind: das fehlende `&` am Argument für `Foo&` (R-5.9), das Ersetzen des Objekts hinter einem `Foo&`-Parameter (R-8.19, Abschnitt 4.4), der fehlende Move eines `own`-Werts an einen `shared`- oder `weak`-Parameter (V-9), die Borrow-Rückgabe auf ein `shared`-Feld (R-5.23, V-2), der Move einer Borrow-Rückgabe (R-5.24) und die Reihenfolge in `x.f = g(x)` (R-5.7). Nach V-13 übersetzt der Vergleich `demote share e` über eine Hilfsmethode, die den Wert erst in eine `shared`-Variable legt.

Mit `--ausfuehren` laufen Programme, die beide annehmen, auch im Modell und im Interpreter von `rppc` (Meilenstein M4, mit Prüfmodus für I-1, I-2, I-4 und I-5). Stand 22.11.2026: alle 10 handgeschriebenen und 466 von 467 ausgeführten Zufallsprogrammen liefern dieselbe Ausgabe einschließlich der Destruktoren bzw. dasselbe Ende mit `BorrowError`; keine Invariante ist verletzt. Der eine Unterschied ist erklärt: Das Modell räumt nach einem Programmfehler nicht auf, R-9.14 verlangt es.
