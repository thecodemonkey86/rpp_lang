# R++ Statements and Expressions (Chapter 8)

[Deutsch](R%2B%2B%20Statements%20and%20Expressions.de.md)

Status: 11/21/2026, Draft 0.1. This document fills Chapter 8 of the R++ Specification Outline, which regulates the meaning of expressions and statements, the order of evaluation and the control structures. The syntax is in grammar (scaffold, chapter 3); the operator type rules in chapter 4; ownership and borrows in chapter 5; zero exams in chapter 6; method selection when overloading in chapter 7; exceptions in chapter 9; lambdas in chapter 10.

All determinations have been decided (see R++ Design Decisions, Sections 11 and 21), including two minor additions from the formulation (section 0), which have also been adopted. `R-8.n`. The conformance tests are in `conformance/ch08_statements/`.

## 0. Amendments to the formulation (adopted on 21.11.2026)

| No | Settlement |
|---|---|
| V-8.1 | A local variable must not obscure another visible local variable or parameter (as in C#). `this.name`. |
| V-8.2 | `nodiscard` Belongs to the signature: An overriding method or a mix-in method that meets a requirement must `nodiscard` take over if the overwritten method or requirement bears it. `nodiscard` A `void` method is a mistake. |
| V-8.3 | (Adopted on 21.11.2026) A class method that meets a `nodiscard` requirement of a mixin (`i64 hash()` for `Hashable`) need not be repeated `nodiscard`; its result is nevertheless `nodiscard`. For overriding methods, V-8.2 remains: there must be `nodiscard`. |

## 1. Diagnoses

**R-8.1 (Errors and warnings).** A compiler reports violations of the specification as **error**; a program with errors is not translated. In addition, there are **warnings**: they must be reported but do not prevent translation. The specification specifies where a warning is mandatory; in version 1 only for unreachable code (R-8.27).

## 2. Evaluation order

**R-8.2 (Left to right).** Partial expressions are evaluated from left to right: the operands of two-digit operators, the arguments of a call, the elements of array and tuple literals, and the arguments of `new`.

**R-8.3 (Short circuit).** `a && b` evaluates `b` only if `a` is true; `a || b` only if `a` is false. `c ? a : b` evaluates exactly one of the two branches. `a ?? b` evaluates `b` only if `a` `null` is true; `x?.…` skippes the rest of the chain if `x` `null` is false (Chapter 6).

**R-8.4 (Allocation).** In `ziel = e;` the partial expressions of the target are evaluated first (in `a[i()].f = g();`, then `a`, then `i()`), then `e`, then assigned. In `(a, b) = e;`, `e` is evaluated fully before assigned; `(a, b) = (b, a);` therefore reverses the values, in `own` values with two moves.

## 3. Expressions

**R-8.5 (Species).** Expressions are literals, names, field accesses, method calls, static accesses with `::`, index accesses, operators according to Chapter 4, `? :`, `??`, `?.`, `as`, `cast`, `copy`, `new`, array and tuple literals and lambdas. Assignments are **not** expressions (R-8.19).

**R-8.6 (Priority).** Priority and associativity of operators are listed in the table of the framework (section 3.2). R-8.2.

**R-8.7 (Index access and ranges).** `a[i]` is defined for fixed arrays, vectors, slices and strings (Strings: Chapter 4, R-4.19); an out-of-range index raises an exception; ranges `a[i..j]`, `a[..j]`, `a[i..]`, `a[..]` are only available for fixed arrays, vectors and slices; they provide a `Slice<T>` as Borrow (Chapter 5). There are no sections on strings; partial strings are supplied by the standard library. `0 ≤ i ≤ j ≤ length` It must apply, otherwise an exception is thrown.

**R-8.8 (`++` and `--`).** `++` and `--` exist for integers; their operand is a variable, a field or an element; they may also be expressed in terms: `i++` gives the old value, `++i` the new value. overflow according to Chapter 4, R-4.14.

**R-8.9 (Conditional expression).** In `c ? a : b` is `c` of type `bool`.
- in the case of number types, the common type of `a` and `b` (R-4.12),
- Otherwise, the type of branch into which the other is automatically converted (`Circle` and `Shape` gives `Shape`; `Foo` and `null` gives `Foo?`).

If there is no such type, the expression is an error; a common base class is not sought.

**R-8.10 (Literals).** Array literals `[a, b]` and Tuple Literals `(a, b)` take their type out of context (chapter 4), R-4.32 and R-4.35The elements shall be: R-8.2 evaluated; `own`-values are shifted into it.

**R-8.11 (Other expressions).** `cast`: Chapter 4, Section 3; `as`: Chapter 4, R-4.23; `copy` and `new`: Chapter 7; `?.` and `??`: Chapter 6; Lambdas: Chapter 10.

## 4. Statements

### 4.1 Printing statements

**R-8.12 (Permitted forms).** Only one call (also `x?.m()`), `++`/`--` and one assignment (R-8.19) are used as an expression statement. `a + b;`, `x;` and `new Foo();` are errors.

**R-8.13 (Unused results).** If a call provides a value as an statement (not a return type `void`), this is an error, even for built-in functions such as `replace` and `take`; the rule does not apply to `++`/`--` as an statement.

**R-8.14 (`discard`).** `discard e;` evaluates `e` and discards the result; a `own` value is released (chapter 5). `e` is a call with a return type except `void`; otherwise `discard` is an error.

**R-8.15 (`nodiscard`).** A method with the addition `nodiscard` (`nodiscard i32 parse(string s)`) has a result that must always be used: `discard parse(s);` is an error. `nodiscard` is preceded `static`, `virtual`, `override` and `abstract` and is also allowed to meet mixin requirements. For override and `void` see V-8.2.

### 4.2 Declarations and assignments

**R-8.16 (Local variables).** A declaration introduces exactly one variable (`T x = e;`, `const T x = e;`, `var x = e;`, `ref T x = p;`) or several by tuple (`(T₁ a, T₂ b) = e;`), and its scope extends from the declaration to the end of the surrounding block. Before the first reading, it must be assigned on each path (Chapter 4, R-4.46).

**R-8.17 (concealment, V-8.1).** A local variable must not obscure any other visible local variable or parameter, even in an inner block.

**R-8.18 (const.).** A `const` variable is set at the declaration and is never reassigned. `const` applies low: no `mut` methods can be called over it and no fields can be set.

**R-8.19 (Allocations).** Assignments are statements and are otherwise only in the head of a `for` loop. Target is a variable, field, element or tuple of targets. Value is converted according to Chapter 4; for ownership, Chapter 5 applies. An assignment to a string index, a `const` variable, a parameter without `own` (such parameters are only readable, even primitive) or a `ref` variable is an error.

**R-8.20 (Interconnection allocation).** `x op= e;` with `op` from `+ - * / %` means `x = x op e;`, the partial expressions of `x` being interpreted only once; the result of `x op e` must be capable of being automatically converted to the type of `x`: At `i8 x` is allowed `x += 1;` (the literal adjusts), `x += n;` with `n : i32` is an error.

### 4.3 Branchings and loops

**R-8.21 (if).** The condition has the type `bool`. `else` belongs to the nearest `if`.

**R-8.22 (While, do, for).** The conditions are of type `bool`. At `do S while (c);` runs `S` at least once. In `for (init; c; update) S` runs `init` once; before each run is tested `c` (missing `c`, it is considered true); after each run and at `continue` run `update`. Variables from `init` apply only in the loop.

**R-8.23 (for-each).** `for (T x : c) S` Is running over
- fixed arrays, vectors and slices (elements in index order),
- Strings (signs as `char`) and
- any class importing the library mixin `Iterable<T>` (form of `Iterable`: Chapter 13).

`c` It is borrowed for the whole loop, `x` is a reading borrow on the element (primitive values and strings: a copy). `for (mut T x : mut c)` The collection and element are borrowed modifiably. During the loop, `c` therefore cannot grow or shrink (Chapter 5, sentence 5.2).

**R-8.24 (Switch switch).** `switch (e)` is allowed for integers, `char`, `string` and enums.
- Each `case` names one or more brands: literals, constants or enum variants, even with bindings (`case Shape::Circle(r):`).
- There is no fallthrough: after the last command of a `case` ends the `switch`.
- Without `default` a `switch` must name over an enum all variants; over other types `default` is mandatory.
- Tied data are read borrows on the data of the variant; primitive values are copied.

**R-8.25 (Break and Continue).** `break` terminates the innermost surrounding loop or the innermost surrounding loop `switch`, whichever is nearer. `continue` continues the innermost surrounding loop, even out of a `switch`. Jump marks (`break outer;`) do not exist in version 1. `break` and `continue` outside a loop or a `switch` are errors.

### 4.4 Return and accessibility

**R-8.26 (Return).** In a `void` method, `return` is without value and is not necessary at the end; in a method with return type `R` `return e;` is `e` convertible to `R`, and any path through the hull ends with `return` or `throw`; otherwise this is an error. Before returning, the local variables are released (chapter 5).

**R-8.27 (Unreachable code).** An statement directly following an statement that does not end normally (Chapter 6, Section 3.3: `return`, `throw`, `break`, `continue` and composed thereof) is unreachable and the compiler reports a **warning** (R-8.1) and translates the program.

**R-8.28 (Throw, try).** Exceptions are governed by Chapter 9.

## 5. Conformance tests

The test programs are in `conformance/ch08_statements/`; they are checked with `rppc test` (first implementation, milestone M3). Programs with `// WARNUNG` must be translated and generate a warning at this point.

| Rules | Accepted | Rejected |
|---|---|---|
| R-8.2 – R-8.4, R-8.8 evaluation, `++` | `a01_evaluation.rpp` | `n01_increment_float.rpp` |
| R-8.9 Conditional expression | `a01_evaluation.rpp` | `n02_no_common_type.rpp` |
| R-8.7 Areas | `a01_evaluation.rpp` | `n03_range_on_string.rpp` |
| R-8.13 – R-8.15 results, `discard`, `nodiscard` | `a03_discard.rpp` | `n04_unused_result.rpp`, `n05_discard_nodiscard.rpp`, `n06_discard_void.rpp`, `n07_nodiscard_void.rpp`, `n08_override_without_nodiscard.rpp` |
| R-8.17 – R-8.20 Declaration, assignment | `a01_evaluation.rpp` | `n09_shadowing.rpp`, `n10_assign_to_const.rpp`, `n11_compound_assignment_narrows.rpp` |
| R-8.23 for-each | `a02_control_flow.rpp` | `n12_foreach_over_non_iterable.rpp` |
| R-8.24 R-8.25 switch, break | `a02_control_flow.rpp` | `n13_duplicate_label.rpp`, `n14_missing_default.rpp`, `n15_non_exhaustive_enum.rpp`, `n16_break_outside_loop.rpp` |
| R-8.26 Return | `a04_return.rpp` | `n17_missing_return.rpp` |
| R-8.27 Unreachable code | `a05_warning.rpp` | – |
