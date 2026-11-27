# R++ Null Safety (Chapter 6)

[Deutsch](R%2B%2B%20Null%20Safety.de.md)

Status: 21.11.2026, Draft 0.2. This document fills in Chapter 6 of the R++ Specification Outline. It specifies when a nullable value may be used, how the compiler checks it (type narrowing) and what the abbreviations `?.` and `??` mean. Which types are nullable is governed by Chapter 4 (section 11); how narrowing interacts with ownership and borrows, Chapter 5.

All determinations of this chapter have been decided (see “R++ Design Decisions”, sections 4 and 19), including clarification V-6.1 from the first implementation (Section 9). `R-6.n`. The conformance tests are in `conformance/ch06_null_safety/`.

## 1. Objective

**Theorem 6.1 (No null dereferences).** A program that meets the rules of this chapter never calls or reads a member of a value that is `null`; therefore, there are no `NullPointerException` or comparable runtime errors.

A formal proof would require extending the FR++ core calculus with `null`. R-6.2 requires a non-nullable operand, and R-6.4 through R-6.14 narrow a value only where it has been checked along every path.

## 2. Basic rules

**R-6.1 (Non-nullable by default).** A type without `?` excludes `null`. `this`, the result of `new`, literals other than `null`, parameters without `?`, and results of methods with a non-nullable return type are also never `null`.

**R-6.2 (Dereferencing).** Field access `x.f`, method calls `x.m(…)`, indexing `x[i]`, `for (… : x)`, arithmetic and comparisons from Chapter 4, and tuple destructuring require a non-nullable operand. An operand of static type `T?` causes an error unless it is narrowed at that point (Section 3).

**R-6.3 (Comparison with null).** `x == null`, `x != null`, and `null == x` require `x` to have a nullable type. The compiler rejects a comparison for a non-nullable `x` because the result is already known. `null == null` is also an error.

There is no operator that skips the test (no `x!`).

## 3. Type narrowing

### 3.1 What is narrowed

**R-6.4 (Expressions eligible for narrowing).** Narrowing applies to local variables, including `ref` variables and loop variables, and to parameters. Fields (`this.child`, `x.next`), array elements (`list[i]`), and call results may change between checking and use. Assign them to a local variable before checking them (R-6.15).

**R-6.5 (Narrowed type).** When a variable `x` of type `T?` is narrowed, it has type `T` at that point. The narrowing preserves its ownership mode and borrow kind. A `ref Foo? c` becomes `ref Foo`, and a `shared Foo? s` becomes `shared Foo`.

### 3.2 Facts from Conditions

For each `bool` expression `c`, the compiler computes two sets of variables. Each member of `W(c)` is non-null when `c` is true. Each member of `F(c)` is non-null when `c` is false.

**R-6.6 (Facts).**

| Expression `c` | `W(c)` | `F(c)` |
|---|---|---|
| `x != null`, `null != x` | `{x}` | `{}` |
| `x == null`, `null == x` | `{}` | `{x}` |
| `!a` | `F(a)` | `W(a)` |
| `a && b` | `W(a) ∪ W(b)` | `F(a) ∩ F(b)` |
| `a \|\| b` | `W(a) ∩ W(b)` | `F(a) ∪ F(b)` |
| `(a)` | `W(a)` | `F(a)` |
| everything else | `{}` | `{}` |

**R-6.7 (Within expressions).** In `a && b`, the compiler checks `b` under `W(a)`. In `a || b`, it checks `b` under `F(a)`. In `c ? a : b`, it checks `a` under `W(c)` and `b` under `F(c)`. Both `x != null && x.ok()` and `x == null || x.empty()` are valid.

### 3.3 Statements

A statement **does not complete normally** if it is `return`, `throw`, `break`, `continue`, or a call to `fail` (R-9.19). The same applies to a block whose last reachable statement does not complete normally, an `if` with an `else` where neither branch completes normally, or an exhaustive `switch` where no case completes normally.

**R-6.8 (if).** In `if (c) S₁ else S₂`, the compiler checks `S₁` under `W(c)` and `S₂` under `F(c)`. The facts after the statement depend on which branches complete normally.
- If `S₁` does not complete normally, use the facts after `S₂`. For a missing `else`, use `F(c)`.
- If `S₂` does not complete normally, use the facts after `S₁`.
- Otherwise, use the intersection of the facts from both branches.

From this follows the previous form: After `if (x == null) { return; }` `x` is narrowed for the rest of the block.

**R-6.9 (Loops).** In `while (c) S` and `for (…; c; …) S`, the compiler checks `S` under `W(c)`. At the loop header, it uses the largest set of facts that hold before the loop and at the end of every iteration, including at `continue`. After the loop it intersects `F(c)` with the facts at every `break`. The same rule applies to `do S while (c)`, with `S` checked without `W(c)`.

**R-6.10 (assert).** `assert(c, text)` is evaluated in both modes and throws an exception if `c` is `false`. `W(c)` holds after the assertion.

**R-6.11 (Assignment).** After `x = e;`, `x` is narrowed exactly when `e` has a non-nullable type or is itself a narrowed variable. `x = null;` clears the narrowing. `x = new Foo();` narrows `x`. A move makes `x` unavailable (Chapter 5).

**R-6.12 (try).** A `catch` or `finally` block inherits only facts that held before the `try` and survive every assignment in the `try` block.

**R-6.13 (Lambdas).** An ordinary lambda sees the facts at its definition. Its captured variables cannot change during the call (Chapter 5). An `own` lambda captures the value with its narrowed type.

**R-6.14 (Validity).** A fact holds within the method that establishes it, until the next assignment to the variable.

## 4. Nullable fields and array elements

**R-6.15 (Reading a field or element).** Bring a nullable field or element into a local variable before checking it. The form depends on its ownership mode.

| Field | Getting to check | Remarks |
|---|---|---|
| `Foo? child` (own) | `ref Foo? c = this.child;` | a normal `Foo? c = this.child;` would be a move from the field (chapter 5, V-3) |
| `Foo? child` (own), remove | `Foo? c = take(mut this.child);` | the field is afterwards `null` |
| `shared Foo? next` | `shared Foo? n = this.next;` | copies the reference (counter + 1) |
| `weak Foo w` | `shared Foo? n = this.w.lock();` | |
| Element of `Foo?[] list` | `ref Foo? e = list[i];` | |
| Primitive, e.g. `i32? count` | `i32? c = this.count;` | primitive values are copied |

Then the local variable is checked as in Section 3.

## 5. Short forms

**R-6.16 (Safe access `?.`).** `x?.m(…)` and `x?.f` require a nullable operand `x`. If `x` is `null`, the result is `null` and evaluation skips the **rest of the chain**, including arguments. In `a?.b.c(f())`, when `a == null`, evaluation skips `.b`, `.c`, and `f()`. For a non-null operand, evaluation proceeds normally.

**R-6.17 (Type of `?.`).** If the chain has type `R` without `?.`, its type with `?.` is `R?`. If `R = S?`, the type remains `S?`. A chain that returns `void` is allowed only as a statement. An `own` field yields a `ref F?` borrow.

**R-6.18 (Null coalescing `??`).** `a ?? b` requires a nullable `a` of type `T?`. It evaluates `b` only if `a` is `null`. The result has type `T` if `b` has type `T` or can be implicitly converted to it. It has type `T?` if `b` has type `T?`. `??` is right-associative, so `a ?? b ?? c` means `a ?? (b ?? c)`. When `a` is an `own` variable and the destination is an owner, the expression moves `a` (Chapter 5).

**R-6.19 (No `??=`).** Version 1 spells null-coalescing assignment `x = x ?? y;`.

## 6. Interaction with other chapters

**R-6.20 (Generics).** If a type argument `T` is already nullable, `T?` is the same type. For `T = Foo?`, `T?` means `Foo?`. `Map<K, V>.get(k)` can therefore return `V?` for any `V`. When `V` is nullable, a `null` result can mean either that the key is absent or that the value is `null`.

**R-6.21 (Downcast).** `e as C` returns `C?` (Chapter 4, R-4.23). Narrow it like any other nullable value, as in `ref Circle? c = s as Circle; if (c != null) { … }`.

**R-6.22 (weak).** `w.lock()` returns `shared T?`. Check the result before accessing the object (Chapter 5, V-6).

**R-6.23 (Primitive values).** Nullable primitive values such as `i32?` allow only comparisons with `null` until narrowed. For `a : i32?`, `a + 1` is an error and `(a ?? 0) + 1` is valid.

## 7. Example

```
string describe(Node? n, Shape s) {
    if (n == null) {
        return "leer";
    }
    // n is narrowed from this point
    ref Node? child = n.child;    // Read the field (R-6.15)
    string c = child?.name ?? "-";
    ref Circle? circle = s as Circle;
    if (circle != null && circle.r > 1.0) {
        return n.name + " groß";
    }
    assert(child != null, "Kind fehlt");
    return child.name;            // Narrowed after assert
}
```

## 8. Conformance tests

The test programs are in `conformance/ch06_null_safety/`; they are tested with `rppc test` (first implementation, milestone M3).

| Rules | Accepted | Rejected |
|---|---|---|
| R-6.2 Dereferencing | `a01_narrowing.rpp` | `n01_unchecked.rpp`, `n10_nullable_arithmetic.rpp` |
| R-6.3 Comparison with zero | `a01_narrowing.rpp` | `n06_compare_non_nullable.rpp` |
| R-6.4 Fields, elements | `a03_fields.rpp` | `n02_narrow_field.rpp`, `n11_narrow_element.rpp` |
| R-6.6 – R-6.8 Conditions, if | `a01_narrowing.rpp` | `n04_if_without_exit.rpp`, `n05_wrong_side_of_or.rpp` |
| R-6.9 Loops | `a01_narrowing.rpp` | `n12_loop_invalidates_narrowing.rpp` |
| R-6.10 assertive | `a01_narrowing.rpp` | – |
| R-6.11 Allocation | `a01_narrowing.rpp` | `n03_assignment_invalidates_narrowing.rpp` |
| R-6.15 Getting | `a03_fields.rpp` | `n13_move_out_of_field.rpp` |
| R-6.16 – R-6.18 `?.`, `??` | `a02_shorthand.rpp` | `n07_safe_access_on_non_nullable.rpp`, `n08_fallback_not_nullable.rpp`, `n09_void_as_value.rpp` |
| R-6.13 R-6.20 Lambdas, Generics | `a04_lambdas_generics.rpp` | – |

## 9. Clarification from the first implementation

| No | Proposal |
|---|---|
| V-6.1 | (assumed on 21.11.2026) Whether an expression "has a nullable type" (R-6.3, R-6.16, R-6.18), for a variable depends on its declared type, not on the narrowed. `Node? v = new Node();` is: `v` Constricted ()R-6.11), but `v?.run()`, `v ?? w` and `v == null` Remaining allowed. Reason: Otherwise, any narrowing would invalidate adjacent code that was previously correct; the runtime test `l02_safe_access.rpp` uses exactly this form. |
