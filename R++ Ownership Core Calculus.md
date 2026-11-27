# R++ Ownership Core Calculus (Chapter 5)

[Deutsch](R%2B%2B%20Ownership%20Core%20Calculus.de.md)

This document fills Chapter 5 of the R++ Specification Outlines, describing ownership and borrowing as a small core language **FR++** with type rules, evaluation rules and a security set modeled on Featherweight Java.

The surface language is mapped on FR++ in Section 7; the rules there are normative for the compiler; FR++ itself is a tool for specification and proof, not a language that someone writes.

Section 2 contains provisions that have become necessary in the elaboration beyond the previous Design Decisions, which were decided on 18.11.2026 (see “R++ Design Decisions”, Section 17).

## 1. Idea in three sentences

1. **Ownership tree.** Each object has exactly one owner: a local variable, a field of another object or – at `shared` – a reference counter.
2. **Lexical Borrows.** Because borrows are never in fields and are never returned, each borrow lives exactly as long as a call or a block, so the compiler manages a simple set of active “loans” instead of lifetimes and prohibits any access that could change the way it is owned.
3. **Dynamic lock for `shared`.** A `shared` object can show several variables that the compiler cannot distinguish, so the exclusivity rule applies to it at runtime: the object has a locked state, a conflict throws an exception. Programs without `shared` do not have a runtime test (sentence 5.3).

## 2. New definitions

V-1 to V-11 have been decided; V-3 has been extended by `take` compared with the first proposal. V-12 has been created during implementation in the first implementation and adopted on 21.11.2026; V-13 has been noticed during comparison with the model and adopted on 21.11.2026; V-14 has been created with the interpreter (milestone M4) and adopted on 22.11.2026. V-15 was created in milestone M5 and accepted on 22.11.2026.

| No | Settlement | Justification |
|---|---|---|
| V-1 | `shared` Objects are mutable. Exclusivity is checked for them at runtime (lock state per object); a breach throws `BorrowError` in **both** modes because the check carries the memory security. | Without testing, a vector could grow over a second `shared` alias, over which iteration is currently taking place. alternative: `shared` only by reading; then graphs and observer structures cannot be changed. |
| V-2 | A path never passes *through* a `shared` or `weak` field. `this.next.name()` The compiler translates into a local copy: `shared Node t = this.next; t.name();` | The lock requires a variable that keeps the object alive for the duration of access, corresponding to the rule in Section 4 of the Design Decisions that nullable fields are fetched into a local variable before access. |
| V-3 | There are two built-in functions instead: `replace(mut x.f, neu)` sets the new value and returns the old one; `take(mut x.f)` is only for nullable fields, it sets the field to `null` and returns the old value. | A field without value would be a hole in the object. `take(mut x.f)` is synonymous with `replace(mut x.f, null)` and therefore in calculus no construct of its own. |
| V-4 | ~~In version 1 methods do not return Borrows.~~ **Modified on 21.11.2026:** A method may return a Borrow which has been `this` outgoing ()`ref V get(K k)` At the call point, it is considered a loan to the recipient until the end of the statement or, with `ref` bound up to the end of the block ()R-5.23, R-5.24). | Without Borrow return, `map.get(k)`, `list.first()` and iterators via `own` elements would be impossible. Because the Borrow can only start from `this`, a loan is sufficient for the recipient; lifetimes are still not necessary. |
| V-5 | A move must be in only one branch of a `if` after which the variable is considered to be moved; whether it has to be released, a drop flag decides at runtime. | As in Rust; avoids unnecessary error messages. |
| V-6 | `weak` is only produced from: `shared` Access to: `w.lock()`, which `shared T?` Then the zero test from Chapter 6 takes effect. | No new language is needed. |
| V-7 | Sharing order: local variables at the block end in reverse declaration order, first the destructor for an object, then the fields in reverse declaration order, and the fields of the base class last. | Like C++. |
| V-8 | A reading borrow protects everything that can be accessed via `own` edges. Objects reached via a `shared` field remain changeable (with runtime check after V-1). | If you can read a `shared` field, you can get your own `shared` copy anyway; a deeper protection would not be enforceable. `const` remains a pure surface inspection. |
| V-9 | Values of type `shared T` are implicitly copied at assignment and handover (counter + 1); from `own` is moved `shared` only if the target is expressly of type `shared` (variable, field or parameter): `shared Foo s = f;` `f`. Conversely, from `shared` never `own`; for this there is `copy(s)`. | Specifies “shared is never implicitly derived”. |
| V-10 | In the constructor, all fields without a default value must be set before `this` is used other than for setting fields. | Otherwise, there would be semi-initialized objects. |
| V-11 | An exception that leaves a destructor ends the program. | Like C++; avoids double exceptions when unwinding. |
| V-12 | (adopted on 21.11.2026) Clarification on R-5.15 and R-5.16 for the surface: a branch that does not end normally (Chapter 6, Section 3.3: `return`, `throw`, `break`, `continue`, `fail`), does not contribute to the condition after `if`; likewise `switch`, grinding and `try`. After `if (fehler) { verbrauche(a); return; } a.run();` `a` is available. one `break` brings its state to the end of the loop, one `continue` to the loop head. | FR++ has no `return` and no `throw`, so R-5.15 says nothing about it. The rule is the same as for the narrowing (R-6.8) and as in Rust; at runtime there is no move on the continued path, so no drop flag problem. Without them, any move before an early exit would be a mistake. |
| V-13 | (assumed on 21.11.2026) A `own` value at a `weak` target (variable, field, parameter), such as `weak Foo w = new Foo();` or `n.w = a;`, is an error (R-4.41): `weak` arises only from `shared` or `weak` (V-6). You can write `shared Foo s = a; weak Foo w = s;`. FR++ `demote share e` The surface of the surface of the lake is the same as that of the surface of the lake. `shared`-variable. | The object would otherwise be released immediately and the `weak`-Reference dead; something like this is almost always an accident. |
| V-14 | (adopted on 22.11.2026) Clarification on V-7 in case of inheritance: first all destructors run, from the class of the object to the top base class; after that all fields are released in reverse declaration order, the fields of the subclass first, those of the base class last. When a constructor (R-9.13) is thrown, the destructors of the finished base part run, after which all already set fields are released as well. | Thus, the runtime tests `l02_destructors.rpp` (Chapter 7) and `l03_constructor_throws.rpp` (Chapter 9) require it; the other reading (Destructor and fields per class alternately) contradicts the second. |
| V-15 | (assumed on 22.11.2026) A `own` object of a class without a destructor can be decomposed into its fields: `(K k, V v) = e;` (R-5.25). The object is moved and released without a destructor; its fields pass to the new variables. V-3 remains: From a field is never moved individually, so for example `V? Map.remove(K key)` can be written in R++. | Moves from individual fields (such as Rust) would have required half-shifted objects, drop flags per field, and extended invariants; the disassembly only needs a new statement, `Γ` and the invariants remain unchanged. A destructor would see an object without fields, so the rule applies only to classes without destructor. |

## 3. The core language FR++

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

Corresponding to the surface: `ref C` is a parameter `Foo`, `mut C` a parameter `mut Foo`, `own C` a local variable `Foo` or a parameter `own Foo`; `μ = mut` is a method with `mut` behind the parameter list. Fields have only memory types; the fact that Borrows are not in fields is therefore already syntactically enforced.

Assumptions: All variable names of a method body are different (otherwise renam). `D` is the base class, `Object` has no fields and methods. `felder(C)` and `methode(m, C)` provide the inherited and own fields or signature as in Featherweight Java.

### 3.2 Auxiliary terms

**environment.** `Γ` assigns each variable a type and a state: `Γ(x) = (τ, z)` with `τ` a memory or parameter type; and `z ∈ {●, ○}` (available, moved) For site checks only `●`.

**Root and prefix.** `wurzel(p)` is the variable at the beginning of `p`. `p ⊑ q` means: `p` is prefix of `q` (`x.a ⊑ x.a.b`).

**spatial locations.** `obj(p) = C` applies if `Γ ⊢ p : τ` and
- `τ ∈ {own C, ref C, mut C}` or
- `τ = shared C` and `p` is a variable (V-2).

Only via object locations can you go further into fields or take a borrow.

**Location typing** `Γ ⊢ p : τ`:

```
(O-Var)   Γ(x) = (τ, ●)                ⟹   Γ ⊢ x : τ
(O-Feld)  obj(p) = C    S f ∈ felder(C)  ⟹   Γ ⊢ p.f : S
```

**Writing right.** `schreibbar(p)` applies if `Γ(wurzel(p))` does not have the type `ref C`.

**Loans.** `L` is a set of couples `(κ, p)`: In this framework there is just one Borrow of kind `κ` on the place `p`.

**overlap.** `p ⋈ q` applies if `wurzel(p) = wurzel(q)` and
- `p ⊑ q` or `q ⊑ p`; or
- the common root is of type `shared C` (a `shared` object is locked as a whole, see 4.3).

**Allowed access.** `erlaubt_L(κ, p)` applies if all `(κ', q) ∈ L` applies: `¬(p ⋈ q)` or `κ = κ' = ro`.

Reading is an access of the species `ro`; writing, moving and `&mut` are accesses of the species `mut`.

**Unification of States.** `Γ₁ ⊔ Γ₂` is point by point `● ⊔ ● = ●`, otherwise `○`. `Γ₁ ⊒ Γ₂` means that every variable available in `Γ₂` is also available in `Γ₁`. `Γ|V` Restricts to the variables `V`.

## 4. Static Semantics

Judgments:

- `Γ; L ⊢ e : S ⊣ Γ'` – Expression `e` has type `S`; after that applies `Γ'` (moves are registered).
- `Γ; L ⊢ b : T ⊣ Γ'; L'` – Argument `b` matches parameter type `T`; `L'` additionally contains the loan for the duration of the call.
- `Γ; L ⊢ s̄ ⊣ Γ'` – Order of statement.

Each rule has the number under which it is listed in the specification and test suite.

### 4.1 Expressions

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

In case of: R-5.3 Only one variable with a memory type can be moved. `ref C` or `mut C` and `this` have no memory type and are therefore excluded.V-3).

In case of: R-5.7 shall be: `e` first checked and only then the place; `replace(x.f, f(move x))` It is therefore a mistake.

### 4.2 Calls and arguments

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

The recipient and the arguments are checked from left to right; each borrow remains in the loan set until the end of the call, thus excluding:

- `p.m(&mut p.f)` for a `mut` method `m` (recipient and argument overlap),
- `f(&mut a, &a)` and `f(&mut a, &mut a)`,
- `f(move a, &a)` (After the Move) `a` no longer available; and `f(&a, move a)` (Move during a loan).

It is allowed `swap(&mut p.links, &mut p.rechts)` because the places do not overlap.

### 4.3 Statements

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

Remarks:

- R-5.12 allows assignment to a moved variable; it is then available again. `x = f(move x)` is allowed.
- After R-5.15 a variable is considered moved if it has been moved in at least one branch (V-5).
- R-5.16 requires that at the end of the loop body everything that was previously available be made available again; a move in the loop is only allowed if the variable is reoccupied in the same run.
- In R-5.17 all access to `p` that contradicts the loan is forbidden during the block: in `&mut` every access, in `&ro` every letter and every move – also on prefixes of `p`.
- If the root of `p` is a `shared` variable, all locations overlap with this root after 3.2. What would be a blocking conflict at runtime is therefore already a compiler error within a method.

### 4.4 Methods and classes

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

R-5.10 is considered as a return only for methods with memory type; a Borrow method can only be accessed via R-5.24; the returned Borrow must only originate from `this` (R-5.23); therefore, at the call point, a loan to the recipient `p` is sufficient, using the mode `μ` of the method (where `κ = mut` is `μ = mut`). As long as it exists, `erlaubt_L` prohibits any change to `p` and anything that can be reached via `own` edges of `p`; thus the returned object remains alive (Lemma 5.4). Returning a borrow to a parameter is forbidden: the parameter is not protected at the call point by a loan that applies beyond the call (counterexample `n20_borrow_from_parameter.fr`).

Subtypes: From `C <: D` follows `own C <: own D`, `shared C <: shared D`, `weak C <: weak D`, `ref C <: ref D` and `mut C <: mut D`. The last one is certain because you can only put fields over a `mut`-Borrow, but never replace the whole object.

## 5. Dynamic Semantics

### 5.1 Duration state

```
Wert       v ::= n | own ℓ | sh ℓ | wk ℓ | ref ℓ | ⊥
Heap       H : ℓ ↦ ⟨C, F, k⟩  oder  †
                F : Feldname ↦ Wert
                k = –                bei Objekten mit eindeutigem Besitzer
                k = ⟨rc, β⟩          bei shared-Objekten; rc ∈ ℕ, β ∈ {frei, lesend(n), schreibend}
Rahmen     E : Variable ↦ Wert
```

`†` marks a shared object. Addresses are not reused in the model; a true implementation holds a control block for this as long as `weak` references exist. `⊥` is the value of a shifted variable and also serves as a drop flag.

A **error state** is reached when an evaluation accesses an address `ℓ` with `H(ℓ) = †`, releases such an address or reads a location with value `⊥`. The calculation has no rule for this; the evaluation remains stuck.

`⚡` is the result of a blocking conflict (exception `BorrowError`). In FR++ there is no `catch`; `⚡` breaks off the evaluation and is a safe result.

### 5.2 Locations

`ziel(p)` provides the address of the object at a location:

- `ziel(x) = ℓ` If `E(x) ∈ {own ℓ, sh ℓ, ref ℓ}` and `H(ℓ) ≠ †`.
- `ziel(p.f) = ℓ'` If `H(ziel(p)).F(f) = own ℓ'` and `H(ℓ') ≠ †`.

`wert(x) = E(x)`, `wert(p.f) = H(ziel(p)).F(f)`.

**Examination of shared roots.** When `E(wurzel(p)) = sh ℓ₀`, each access of the species `κ` checks `p` the restricted state `β` of `ℓ₀`: `ro` required `β ≠ schreibend`, `mut` required `β = frei`; otherwise the result is `⚡`.

**Locking.** `sperre(p, κ)` If `E(wurzel(p)) = sh ℓ₀`, after the test `β`, set from `frei` or `lesend(n)` to `lesend(n+1)` (at `ro`) or from `frei` to `schreibend` (at `mut`), respectively. `freigabe(p, κ)` Reverse this. With other roots, both do nothing.

### 5.3 Release

`drop(v)`:

| Value | Effects |
|---|---|
| `n`, `ref ℓ`, `wk ℓ`, `⊥` | Nothing |
| `own ℓ` | Destructor of `ℓ` with `this = ref ℓ` execute, then `drop` to all fields in reverse order, then `H(ℓ) := †` |
| `sh ℓ` | `rc` Decrease by 1; is `rc = 0`: as in `own ℓ` |

As long as the destructor of a `shared` object is running, `rc = 0`; `upgrade` fails.

### 5.4 Evaluation

Big step judgments `H, E ⊢ e ⇓ v ⊣ H', E'` and `H, E ⊢ s̄ ⇓ H', E'`; each rule can instead deliver `⚡`, which is passed on unchanged.

| Rules | Constructure | Evaluation |
|---|---|---|
| E number | `n` | Result `n` |
| E-copy | `copy p` | Check `p` read access; `v = wert(p)` when `v = sh ℓ` incremented `rc` by 1 |
| E-move | `move x` | `v = E(x)`; `E(x) := ⊥` |
| E-New | `new C(ē)` | arguments from left to right to `v̄`; new address `ℓ`; `H(ℓ) := ⟨C, f̄ ↦ v̄, –⟩`; result `own ℓ` |
| E-share | `share e` | `e ⇓ own ℓ`; `k(ℓ) := ⟨1, frei⟩`; Result `sh ℓ` |
| E-Demote | `demote e` | `e ⇓ sh ℓ`; result `wk ℓ`; thereafter `drop(sh ℓ)` |
| E-replacements | `replace(p.f, e)` | `e ⇓ v`; check write access to `p.f`; `v_alt = wert(p.f)`; set field to `v`; result `v_alt` |
| E-call | `p.m(b̄)` | `sperre(p, μ)`; `ℓ = ziel(p)`; arguments from left to right: expressions to values, `&κ q` to `ref ziel(q)` according to `sperre(q, κ)`; method to choose the class of `H(ℓ)`; new frame `this ↦ ref ℓ, xᵢ ↦ vᵢ`; torso execute `return` expression to `v`; all variables of the frame in reverse order `drop`; release all blocks of the call in reverse order; result `v` |
| E-Decl | `S x = e;` | `e ⇓ v`; `E(x) := v` |
| E-Zuw-Var | `x = e;` | `e ⇓ v`; `v_alt = E(x)`; `E(x) := v`; `drop(v_alt)` |
| E-Zuw field | `p.f = e;` | `replace(p.f, e) ⇓ v_alt`; `drop(v_alt)` |
| E-expressors | `e;` | `e ⇓ v`; `drop(v)` |
| E-decomposition | `(S̄ x̄) = move y;` | `E(y) = own ℓ` `H(ℓ) = ⟨C, f̄ ↦ v̄, –⟩`; `E(xᵢ) := vᵢ`; `E(y) := ⊥`; `H(ℓ) := †` without destructor (there is none) |
| E-If, E-While | | as usual; condition `≠ 0` |
| E-Borrow | `borrow x = &κ p { s̄ }` | `sperre(p, κ)`; `E(x) := ref ziel(p)`; Execute block; `freigabe(p, κ)` |
| E-Borrow call | `borrow x = p.m(b̄) { s̄ }` | as e-call, but the receiver's lock `sperre(p, μ)` remains until the end of the block; the fuselage ends with `return &κ q` being evaluated to `ref ziel(q)`; `E(x) := ref ℓ'`; execute block; then `freigabe(p, μ)` |
| E-Upgrade | `upgrade x = p { s̄₁ } else { s̄₂ }` | `wert(p) = wk ℓ`; is `H(ℓ) ≠ †` and `rc(ℓ) > 0`: `rc` increase by 1, `E(x) := sh ℓ`, block `s̄₁`, thereafter `drop(E(x))`; otherwise block `s̄₂` |
| E-block | each block | at the end all variables declared in the block in reverse order `drop` (for `⊥` nothing) |

Assignments use the new value and release the old one, so a destructor running on it never sees an object with a field that has just been released.

## 6. Safety

### 6.1 Sentences

**Clause 5.1 (Storage security).** If a FR++ program is well-typed, its evaluation does not reach an error state; it provides a result, ends with `⚡` or runs infinitely.

Conclusions: No object is used after sharing (no use-after-free), none is released twice (no double-free), and the destructor of each object runs at most once.

**Sentence 5.2 (Iterators).** As long as a borrow insists on an object, no object reachable by it via `own` edges is released; for vectors (section 7), this means that during an iteration or as long as a slice exists, the vector does not grow or shrink.

**Sentence 5.3 (No runtime test without shared).** If a well-typed program does not contain any types `shared C` and `weak C`, `⚡` does not occur.

Sentence 5.3 follows directly from the fact that only accesses over `shared` roots are checked.

Not part of the guarantee is that every object is released: cycles from `shared` references are never released (leak, but no error).

### 6.2 Sketch of evidence

The proof shows that the following invariants are preserved at each step, only outlined here; a complete proof (Coq, Lean or Isabelle) as machine-tested as possible is pending.

- **I-1 Clear ownership.** For each living object `ℓ` with `k = –` there is exactly one value `own ℓ`, either in a frame or in a field of a living object; for each living `shared` object there is no value `own ℓ`, and `rc(ℓ)` is the number of values `sh ℓ` in frames and living fields.
- **I-2 Living owners.** Any value `own ℓ` or `sh ℓ` in a frame or living field refers to a living object.
- **I-3 Consistency of Γ and E.** If it is static `Γ(x) = (τ, ●)`, then `E(x) ≠ ⊥`.
- **I-4 Justified Borrows.** Each value `ref ℓ` in a frame goes back to an active loan `(κ, p)` in a deeper frame, with `ziel(p) = ℓ` at the time of borrow.
- **I-5 Locked Objects Live.** If `β(ℓ) ≠ frei`, then a `shared` variable holds the object in a frame, thus `rc(ℓ) ≥ 1`.

The core of the evidence is:

**Lemma 5.4 (path stability).** As long as a loan `(κ, p)` is active in a frame, no ownership edge changes on the way from `wurzel(p)` to `ziel(p)` and no ownership edge below `ziel(p)` – except via the Borrow itself if `κ = mut`.

Justification by type of root of `p`:

- *Local `own` variable:* After I-1, no other frame reaches the objects on the way, except via Borrows, which is issued by the same frame, all of these Borrows are compatible with the Loan because of `erlaubt_L` and within the frame itself `erlaubt_L` prohibits any writing, move or contradictory Borrow.
- *Borrow variable (`this`, parameter, borrow block):* as above, and for the path up to `ziel(wurzel(p))`, the lemma per induction applies to the loan in the calling frame.
- *`shared` variable:* Other frames can reach the object via their own `shared` references, but each access via a `shared` root checks the locked state (5.2) and the loan holds a suitable lock. A `mut` Loan excludes any foreign access, a `ro` Loan excludes any foreign writing.

Destructors are common calls: a destructor only gets `this` and does not reach his own object by any other way, because the owner has already abandoned it (I-1) or `rc = 0` is and `upgrade` fails.

For R-5.25 I-1 is retained: the values of the fields pass from the shared object into the frame, each ownership value is then exactly once; `y` is `⊥` (I-3), and because the class has no destructor, no code sees the empty object. A borrow on `y` or a place below it cannot exist because `erlaubt_L(mut, y)` is required (I-4).

From Lemma 5.4, I-2 and I-4 it follows that `ziel(p)` is defined for each location tested. From I-3 it follows that `⊥` is never read. Because `drop(own ℓ)` is only called when the single value `own ℓ` has just been removed from the frame or field (I-1), nothing is released twice. This results in sentence 5.1; sentence 5.2 is Lemma 5.4 for the loan of the iteration or slice.

## 7. Reproduction of surface language

| Surface surface | FR++ | Rules |
|---|---|---|
| Local variable `Foo x = e;` | `own C x = e;` | R-5.11 |
| Use of `x` (type `own`) as value, cell value or `own` argument | `move x` | R-5.3 |
| Use of `x` as argument for parameters `Foo` | `&ro x` | R-5.9 |
| Argument `mut x` for parameters `mut Foo` | `&mut x` | R-5.9 |
| Parameters `Foo` / `mut Foo` / `own Foo` / `shared Foo` | `ref C` / `mut C` / `own C` / `shared C` | R-5.20 |
| Method with `mut` according to the parameter list | `μ = mut` | R-5.10, R-5.20 |
| `shared Foo s = f;` with `f` type `own` | `shared C s = share move f;` | R-5.5, V-9 |
| `shared Foo t = s;` with `s` type `shared` | `shared C t = copy s;` | R-5.2 |
| `weak Foo w = s;` | `weak C w = demote copy s;` | R-5.6 |
| `shared Foo? p = w.lock(); if (p != null) { … }` | `upgrade p = w { … } else { }` | R-5.18, V-6 |
| `this.next.name()` with `shared` field `next` | `shared C t = copy this.next; t.name();` (Auxiliary variable of the compiler) | V-2 |
| `this.payload = p;` | `this.payload = move p;` | R-5.13 |
| `Foo alt = replace(mut this.payload, neu);` | `own C alt = replace(this.payload, …);` | R-5.7, V-3 |
| `Foo? alt = take(mut this.cache);` (only for nullable field) | as `replace(this.cache, null)` | R-5.7, V-3 |
| `copy(a)` | Calling the copy method with `&ro a`; result `own C` | R-5.10 |
| `(K k, V v) = e;` with `e` of the type of class | `(own K k, own V v) = move y;` with `y` the local variable or an auxiliary variable for a temporary value; fields in declaration order, inherited first; each field must be visible at the location (R-7.2) | R-5.25 |
| `new Foo(args)` with constructor | Constructor as a method ending with `new C(…)`; fields previously fully set | V-10 |
| `for (Foo f : list) { … }` | `borrow l = &ro list { … borrow f = &ro l[i] { … } … }` | R-5.17 |
| `for (mut Foo f : mut list) { … }` | as above with `&mut` | R-5.17 |
| `Slice<T> s = v[1..3];` (local) | `borrow s = &ro v { Rest des Blocks }` | R-5.17 |
| `ref Foo f = p;` | `borrow f = &ro p { Rest des Blocks }` | R-5.17 |
| `ref mut Foo g = mut p;` | `borrow g = &mut p { Rest des Blocks }` | R-5.17 |
| Method `ref V get(K k)` / `ref mut V at(K k) mut` | `brw(ro, V) get(…) ro` / `brw(mut, V) at(…) mut`; torso ends with `return this…;` or `return mut this…;` | R-5.23 |
| `ref V v = map.get(k);` | `borrow v = map.get(k) { Rest des Blocks }` | R-5.24 |
| `print(map.get(k).name);` (Result not bound) | `borrow tmp = map.get(k) { print(tmp.name); }`: The loan is valid until the end of the statement | R-5.24 |
| Parameters `mut Slice<T> s` | `mut` on the vector | R-5.9 |
| Normal Lambda as an Argument | each variable covered shall be borrowed, read or modifiable for the duration of the call depending on the body | R-5.9, R-5.10 |
| `own` Lambda | `new Closure(…)` with one field per capture: primitive values and `shared` copied, `own` moved | R-5.4 |

**Vectors and indices.** A vector has its elements like an object has its fields. `a[i]` is a place with an unknown index: `a[i]` and `a[j]` are always considered to be overlapping, `swap(mut a[i], mut a[j])` is therefore an error. Boundary violations throw an exception. Excluding only goes through `mut` methods such as `pop()` and `remove(i)` that deliver `own T`.

**Exceptions.** Unwinding releases locks and releases the variables of each abandoned block as at the normal block end; drop flags `⊥` If an exception leaves a destructor, the program ends.V-11).

**Threads.** A different thread is provided only with a `own` value whose tree does not contain any `shared` or `weak` fields because the reference counter is not atomic, but strings are excluded and require an atomic counter.

**Primitive Borrows.** Parameters such as `mut i32 n` and variably recorded primitive variables function like borrows on object locations; the calculation leaves them out for simplification.

**Strings** In calculus, they behave as `int`: unchangeable, copyable, without Borrows.

## 8. Limits and possible extensions

- **Return Borrows (V-4).** Implemented in R-5.23 and R-5.24, but only for Borrows starting from `this`. Returning Borrows to parameters (such as `ref mut T max(mut T a, mut T b)`) would require Lifetimes or a rule which arguments the loan includes at the call point; this is not part of version 1.
- **Durability of named borrows.** A `ref` Borrow and a local slice are valid until the block ends, not only until the last use. `ref Foo f = list[0]; f.run(); list.push(x);` is therefore a mistake; you set your own block. A rule “until the last use” (such as Rust’s Non-Lexical Lifetimes) would be more convenient, but requires data flow analysis and an extension of the calculation.
- **Finer barriers.** A `shared` object is locked as a whole; two borrows on different fields of the same `shared` object are therefore an error.
- **Call back to shared objects.** If a `mut` method of a `shared` object calls a method of the same object via a second reference, this throws `BorrowError`.This is the known weakness of this approach (as `RefCell` in Rust) and the price for V-1.
- **Leakage through cycles** are not prevented; `weak` is the means against it.

## 9. Conformance tests

The test programs are in `conformance/ch05_ownership/` Programs in `accepted/` must be translated, programs in `rejected/` must be rejected with exactly the error mentioned in the head comment. They are tested with `rppc test` (first implementation, milestone M3).

| Rules | Accepted | Rejected |
|---|---|---|
| R-5.3 Move | `a01_moves.rpp` | `n01_use_after_move.rpp`, `n10_borrow_in_field.rpp` |
| R-5.7 / R-5.13 Place field, `replace`, `take` | `a02_fields.rpp` | `n08_move_out_of_field.rpp`, `n09_return_field.rpp`, `n16_take_non_nullable_field.rpp` |
| R-5.9 / R-5.10 Arguments | `a03_borrows.rpp` | `n05_duplicate_mut_borrow.rpp`, `n06_move_and_borrow_in_call.rpp`, `n07_overlapping_receiver.rpp` |
| R-5.15 If | `a01_moves.rpp` | `n02_conditional_move.rpp` |
| R-5.16 While | `a01_moves.rpp` | `n03_move_in_loop.rpp` |
| R-5.17 Borrow-Block (for, Slice) | `a03_borrows.rpp` | `n04_vector_grows_in_loop.rpp`, `n11_vector_grows_with_slice.rpp` |
| Writing right | `a03_borrows.rpp` | `n12_write_through_read_only_borrow.rpp` |
| Overlap in shared roots | `a04_shared_weak.rpp` | `n13_duplicate_shared_root.rpp` |
| R-5.5 / R-5.6 / R-5.18 shared, weak | `a04_shared_weak.rpp` | `n14_shared_as_own.rpp` |
| Lambdas | `a05_lambdas.rpp` | `n15_move_out_of_non_own_lambda.rpp` |
| V-13 `own` On `weak` | `a04_shared_weak.rpp` | `n21_own_to_weak.rpp` |
| R-5.25 Disassembly | `a07_destructure.rpp` Duration `l01_destructure.rpp` | `n22_destructure_with_destructor.rpp`, `n23_destructure_wrong_field_count.rpp`, `n24_destructure_borrowed_value.rpp`, `n25_destructure_private_fields.rpp`, `n26_used_after_destructuring.rpp` |
| `ref` Declaration (R-5.17) | `a06_ref.rpp` | `n17_ref_lives_to_end_of_block.rpp`, `n18_ref_to_temporary.rpp`, `n19_mutate_read_only_ref.rpp`, `n20_ref_mut_and_direct_access.rpp` |

## 10. Executable model

In the directory `fr-model/`, there is a type checker and interpreter for FR++ (Python, without further packets) and implements the rules from Sections 3 to 5 as literally as possible; the error messages indicate the rule number. For test programs `print(e);` and the int operators `+`, `-`, `<` are added. Operation and limits describes `fr-model/README.md`, the last run `fr-model/test-report.txt`.

### 10.1 Handwritten programs

- 10 programs that are accepted and must deliver a set result: Moves with drop flags, `replace`, disjointed Borrows, `shared`/`weak`/`upgrade`, release order after V-7, lock conflict and multiple reading lock after V-1, `upgrade` in the destructor, cycle as leak, Borrow return (R-5.23, R-5.24).
- 24 programs that have to be rejected with a certain rule in a certain line.
- 11 of them are **provenly uncertain**: If they are executed without type check, 10 run into an error state and one ends with `BorrowError`. Thus, for the Move rule, the `if` association, the loop rule, the loan check for calls and Borrow blocks, the write right and the Borrow return (only from `this`, Loan to the receiver) prove that they prevent real errors.

### 10.2 Random tests

`fr-model/fuzz.py` creates programs with a fixed class table and random method bodies, destructors and main programs, checks them and executes them with invariant testing; since 21.11.2026 the class table contains Borrow methods, and the generator generates `borrow x = p.m(…) { … }`.

Run of 21.11.2026, 2000 Programs:

| | Number |
|---|---|
| Accepted | 644 |
| of which normally terminated | 384 |
| of which `BorrowError` (permitted, V-1) | 94 |
| of which step budget exhausted (permitted) | 166 |
| **thereof fault state or injured invariant** | **0** |
| Rejected | 1356, of which 151 are actually in the fault state when executed |

Covered were, among other things, 155 954 releases, 13 435 locks over `shared` Roots, 9 084 Borrow calls and 110 successful and 5 138 failed `upgrade` Against sentence 5.1 and 5.3 and against “without” `shared` No leak, there was no breach. An earlier run without Borrow return (20.11.2026, 2000 programs, 816 accepted) was also without violation.

### 10.3 Cross-checks

For each cross-test, a rule of the type checker is switched off and the random test is repeated with 600 programs:

| Deactivated | Infringements | Assessment |
|---|---|---|
| Loan test (`erlaubt_L`) | 5 (I-4: Borrow on shared object) | Rule required, reached by the random test |
| Condition ○ after Move | 12 (reading of ⊥) | Rule required, reached by the random test |
| Writing right | 1 (I-4) | Rule required, reached by the random test; in addition `n19_write_through_alias.fr` |
| Loan on the recipient in R-5.24 | 1 (I-4) | Rule required, reached by the random test; in addition `n21_receiver_modified.fr` |
| Union in R-5.15 | 0 | Random test does not reach the sample; evidence by `n02_conditional_move.fr` |
| Loop condition in R-5.16 | 0 | Random test does not reach the sample; evidence by `n03_move_in_loop.fr` |
| Overlap of whole `shared` roots | 0 | expected: the rule is not safety-relevant, the runtime lock intercepts all cases (see 4.3); `n11_duplicate_shared_root.fr` ends without it with `BorrowError` |

The fact that Borrows can only assume from `this` (R-5.23) cannot be switched off with the random test, because the class table must be well-typed; the proof is provided `n20_borrow_from_parameter.fr`.

### 10.4 Findings

- There was nothing to correct about the calculation: type testers and interpreters could be built from the rules without additional stipulations; this also applies to the extension to include Borrow returns.
- The assumption in 4.3 that the static overlap of entire `shared` roots only anticipates runtime conflicts and does not carry safety is confirmed.
- Gap of the tests: For union and loop rule, the document depends on a handwritten program; the random generator generates the necessary patterns too rarely.
- The surface rule V-13.6 (Borrow return via `ref` variables and loop elements starting from `this`) is not shown in the model because FR++ `return` is only known at the end of a method body.
- Random tests find errors, but do not prove correctness. The proof of sentence 5.1 remains open.

### 10.5 Comparison with the first implementation

`fr-model/compare.py` translates FR++ programs backwards into R++ (`frpp/to_rpp.py`) according to the table in Section 7 and checks whether `rppc` accepts or rejects them as the model. `move x` For an int, weak or shared variable, a copy (V-9) is on the surface; then the normal form is compared with `copy x`. Without surface correspondence, paths remain through `shared` fields (V-2 allowed there), assignments to parameters without `own` (R-8.19) and unbound Borrow returns as statement; these programs count separately.

Status 21.11.2026 (`fr-model/comparison-report.txt`): All 32 translatable handwritten programs and 4466 out of 5000 random programs (1564 adopted, 2902 rejected) are judged equally; there is no deviation. For the cross-check, one rule in the model is switched off (as in 10.3); for each of the seven attenuations, the comparison finds deviations.

The comparison has found six gaps in `rppc` that have been fixed: the missing `&` argument for `Foo&` (R-5.9), the replacement of the object behind a `Foo&` parameter (R-8.19, Section 4.4), the missing move of a `own` value to a `shared` or `weak` parameter (V-9), the Borrow return to a `shared` field (R-5.23, V-2), the move of a Borrow return (R-5.24), and the order in `x.f = g(x)` (R-5.7). After V-13 the comparison `demote share e` translates via an auxiliary method, which only puts the value into a `shared` variable.

With `--run` programs that both accept run also in the model and interpreter of `rppc` (milestone M4, with test mode for I-1, I-2, I-4 and I-5). as of 22.11.2026: all 10 handwritten and 466 of 467 executed random programs deliver the same output including the destructors and the same end with `BorrowError`; no invariant is injured. One difference is explained: the model does not clean up after a program error, R-9.14 it requires.
