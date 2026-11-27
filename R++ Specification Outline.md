# R++ Specification Outline

[Deutsch](R%2B%2B%20Specification%20Outline.de.md)

Draft 0.47, 22 November 2026. This document lays out the chapters of the R++ language specification. The executable ANTLR4 grammar and its tests are described in Section 3.4. [Design decisions](R%2B%2B%20Design%20Decisions.md) · [Grammar](rpp-grammar/RPP.g4)

## Conventions

- Normative rule numbers follow the form `R-<chapter>.<number>`, such as `R-5.3`.
- The conformance suite pairs each rule with programs that the compiler must accept and programs it must reject.
- The compiler reports rule violations as **errors** and stops compilation. It also reports **warnings** where the specification requires them. Warnings allow compilation to proceed (Chapter 8, R-8.1).
- ANTLR4 uses `:` to define a rule, `\|` for alternatives, `?` for optional elements, `*` for zero or more repetitions, `+` for one or more, and `'...'` for terminals.

## Chapter summary

| No | Chapter | Content | Status |
|---|---|---|---|
| 1 | [Introduction](R%2B%2B%20Introduction.md) | Objectives, definitions, Conformance | Draft 0.1 (“R++ Introduction”) |
| 2 | Lexical Structure | Characters, identifiers, keywords, literals, comments | Design, tested |
| 3 | Grammar | Context-free syntax | Executable, tested |
| 4 | [Types](R%2B%2B%20Types.md) | Base types, class, array, tuple, function, nullable types | Draft 0.2 (“R++ Types”), Rules R-4.1 to R-4.48, V-4.0 to V-4.11 decided |
| 5 | [Ownership Core Calculus](R%2B%2B%20Ownership%20Core%20Calculus.md) | Ownership modes, move, borrow, exclusivity; as a core calculus with rules of inference | Draft 0.5 (“R++ Ownership Core Calculus”), Borrow return of `this` (R-5.23, R-5.24) supplemented, V-1 until V-11 decided executable model in `fr-model/` |
| 6 | [Null Safety](R%2B%2B%20Null%20Safety.md) | Nullable types, test, narrowing | Draft 0.1 (“R++ Null Safety”), rules R-6.1 to R-6.23, everything decided |
| 7 | [Classes and Generics](R%2B%2B%20Classes%20and%20Generics.md) | Member, inheritance, import, constraints | Draft 0.1 (“R++ Classes and Generics”), rules R-7.1 to R-7.28, everything decided |
| 8 | [Statements and Expressions](R%2B%2B%20Statements%20and%20Expressions.md) | Semantics, Evaluation Order, Operators | Draft 0.1 (“R++ Statements and Expressions”), rules R-8.1 to R-8.28, everything decided |
| 9 | [Exceptions](R%2B%2B%20Exceptions.md) | Throwing, catching, unwinding | Draft 0.1 (“R++ Exceptions”), rules R-9.1 to R-9.19, everything decided |
| 10 | [Lambdas](R%2B%2B%20Lambdas.md) | Capture by Borrow and by Value, Function Types | Draft 0.1 (“R++ Lambdas”), rules R-10.1 to R-10.18, everything decided |
| 11 | [Packages and Projects](R%2B%2B%20Packages%20and%20Projects.md) | `package`, `using`, `export`, project file, dependencies, central directory, conditional compilation | Draft 0.1 (“R++ Packages and Projects”), Rules R-11.1 to R-11.25 |
| 12 | [Execution Model](R%2B%2B%20Execution%20Model.md) | Debug and release mode, allowed differences | Draft 0.1 (“R++ Execution Model”), Rules R-12.1 to R-12.12 |
| 13 | [Standard Library](R%2B%2B%20Standard%20Library.md) | Minimum size: `string`, Arrays, Exceptions, `Func` | Draft 0.8 ("R++ Standard Library"), first cut for self-translation, everything decided; extension with `rpp.random`, `Vectors`, `rpp.math`, `Text`, file operations, `Deque`/`getOrPut` (sections 9–15) |
| 14 | [Conformance Tests](R%2B%2B%20Conformance%20Tests.md) | Structure of the test suite, assignment to rules | Draft 0.2 ("R++ Conformance Tests"), all rules named or justified |
| 15 | [C Interop](R%2B%2B%20C%20Interop.md) | `extern` classes, handles, types across the boundary, safety boundary | Draft 0.1 ("R++ C Interop"), rules R-15.1 to R-15.17, amendments V-15.1 to V-15.9 adopted |
| 16 | [Threads](R%2B%2B%20Threads.md) | `rpp.sync`: threads, scopes, transfer between threads, channels, order | Draft 0.1 ("R++ Threads"), rules R-16.1 to R-16.23, amendments V-16.1 to V-16.11 adopted |

## 2. Lexical structure

- The source code is UTF-8.
- Comments: `// to the end of the line` and `/* ... */`.
- Identifier: letter or `_`, followed by letters, numbers or `_`.
- floating point numbers need a digit before and after the point ()`0.5`, not `.5` Optionally, an exponent follows: `e` or `E`, a sign and numbers (`1.0e10`, `2.5E-3` Without a point, there is no exponent: `1e10` is a syntax error.
- Integers decimal or hexadecimal (`0xFF`).
- Numbers may be separated by `_` (`1_000_000`, `0xFF_FF`), but not at the beginning or end of a sequence of numbers.
- `..` is a separate token for ranges. `1..3` is read as `1`, `..`, `3`, not as a floating point number.
- There are no tokens `<<` and `>>`. shift operators consist of two consecutive `<` and `>` respectively, so that `List<List<i32>>` remains readable.

Reserved words:

```
abstract as base bool break byte case cast catch char class const
constructor continue copy default destructor discard do else enum export extern f32 f64 false
finally for i8 i16 i32 i64 if import mixin mut new nodiscard null override own package
private protected public ref return shared static string switch this throw
true try ui8 ui16 ui32 ui64 using var virtual void weak while
```

`main` is not reserved and may also be used as a name.

Conditional compilation: A line whose first character is marked after empty space. `#` It is a directive,`#if Bedingung`, `#elif Bedingung`, `#else`, `#endif` Conditions consist of symbols, `!`, `&&`, `||` and brackets; `&&` binds more strongly than `||`. Directives are evaluated before parsing; the lines of inactive branches are skipped and not checked; any other directive is an error, including `#define`.

## 3. Grammar

Normative is the project document “RPP.g4”. This section records what has been defined in the implementation beyond the design decisions.

### 3.1 Implementation determinations

1. **Printing statements.** Only three forms of statement are allowed: assignment, `++`/`--` and an expression ending with an invocation. `a + b;`, `foo;` and `new Foo();` are errors. Assignments are only available as statement and in the `for` header, not within expressions (`x = y = 0;` is a syntax error); `++`/`--` may also be in expressions.
2. **Allocation target.** To the left of `=` is a name, field access, index access or tuple, not a computational expression.
3. **Lambdas.** A lambda stands at the level of expression; its hull extends to the end of the surrounding expression; it must be clamped as an operand or for direct invocation: `((i32 a) => a)(3)`.
4. **Case marks.** Literals (also negative), names of constants or enum variants and variants with bindings such as `Circle(r)` are allowed.
5. **One variable per declaration.** `i32 a, b;` does not exist. Several variables are introduced by the Tuple declaration, also in the `for` header: `for ((i32 i, i32 j) = (0, n); i < j; i++, j--)`.
6. **`else`** It is the nearest `if`.
7. **Enums** may have a comma after the last variant.
8. **Copy.** `copy(a)` is an expression of its own; `a.copy()` is a mistake.
9. **Static access with `::`.** Static methods, constants and enum variants are addressed by `::`: `Math::max(a, b)`, `Shape::PI`, `Color::Red`, `List<Foo>::create()`, `my.pkg.Registry<Foo>::count()`. Type arguments in terms only directly precede `::`; `x = List<Foo>(a);` a comparison remains. Case brands and array sizes also qualify with `::` (`case Color::Blue:`, `i32[Limits::N]`); `case Color.Blue:` and `byte[Buffers.SIZE]` are syntax errors.
10. **Generic methods** their type parameters are named: `T max<T : Comparable>(T a, T b)`.
11. **Multiple constraints** are connected to `&`; a comma separates type parameters: `<T : A, B>` declared `T` with constraint `A` and a second parameter `B`.
12. **`void` as a typical argument** is allowed syntactically everywhere; that it may only be in the last place of `Func` checks a later step.
13. **Fixed arrays.** The size in `T[...]` is an integer literal or name; expressions such as `i32[2 + 2]` If a name is a constant, check a later step. `List[Foo] x;` syntactically a fixed array.
14. **Areas** only in index brackets: `a[1..3]`, `a[..3]`, `a[1..]`, `a[..]`. `x = 1..3;` is an error.
15. **Slices** have no syntax of their own; `Slice<T>` is an ordinary generic type.
16. **`export`** is forward `class`, `mixin` or `enum` For abstract classes, `abstract`: `export abstract class Shape` In methods, fields, `main` and `using` It is not allowed.

17. **Named Borrows** are declared with `ref`: `ref Foo f = list[0];` (reading) and `ref mut Foo g = mut x.child;` (modifiable, `mut` before type and initializer); without initializer, with ownership form, with `var`, as field, as parameter and in `for` header `ref` is a syntax error. Whether the initializer is a location, checks a later step.

18. **Downcast with `as`.** `e as T` is a two-digit expression that binds more strongly than comparisons and weaker than shift operators: `s as Circle != null` is `(s as Circle) != null`, `a + b as Foo` is `(a + b) as Foo`.
19. **Enum values.** A variant carries either data or a value from an integer literal, also negative: `enum Level { Low = 1, Mid, High = 10 }` Expressions and constant names as a value are syntax errors.

20. **Constraint `new()`.** A constraint is a type name or `new()` without parameters: `class Pool<T : Comparable<T> & new()>`.
21. **Mixines** contain methods, requirements and `import` statements, but not fields, constructors and visibility labels.

22. **`discard`** It is an expression of its own statement: `discard list.pop();`.
23. **`nodiscard`** is before `static`, `virtual`, `override` or `abstract` a method and before mixin requirements, not on fields and constructors.

24. **try** has at least one `catch` or one `finally`; `finally` after all `catch`.

25. **`mut` in Type Arguments** is allowed syntactically anywhere (`Func<mut Foo, void>`); that it may only stand on parameters of `Func`, checks a later step.

26. **Borrow return.** A return type can be `ref T` or `ref mut T` (`ref V? get(K key)`, `ref mut V at(K key) mut`); `return mut e;` provides a modifiable borrow.
27. **Static members of primitive types** are addressed with `::` as for classes: `i32::MAX`, `i32::parse(s)`, `string::join(sep, parts)`.

28. **Using** Names a type (`using a.b.C;`) or assigns another name (`using X = a.b.C;`); `using a.b.*;` is a syntax error.

### 3.2 Operators

Priority from weak to strong binding:

| Stage | Operators | Associativeness |
|---|---|---|
| 1 | [Introduction](R%2B%2B%20Introduction.md) | – |
| 2 | `? :` | Right right |
| 3 | `??` | Right right |
| 4 | `\|\|` | left-hand left |
| 5 | [Ownership Core Calculus](R%2B%2B%20Ownership%20Core%20Calculus.md) | left-hand left |
| 6 | `\|` | left-hand left |
| 7 | [Classes and Generics](R%2B%2B%20Classes%20and%20Generics.md) | left-hand left |
| 8 | [Statements and Expressions](R%2B%2B%20Statements%20and%20Expressions.md) | left-hand left |
| 9 | [Exceptions](R%2B%2B%20Exceptions.md) | left-hand left |
| 10 | [Lambdas](R%2B%2B%20Lambdas.md) | left-hand left |
| 10a | `as` | left-hand left |
| 11 | [Packages and Projects](R%2B%2B%20Packages%20and%20Projects.md) | left-hand left |
| 12 | [Execution Model](R%2B%2B%20Execution%20Model.md) | left-hand left |
| 13 | [Standard Library](R%2B%2B%20Standard%20Library.md) | left-hand left |
| 14 | [Conformance Tests](R%2B%2B%20Conformance%20Tests.md) | Right right |
| 15 | Postfix: `.` `?.` `()` `[]` `++` `--` | left-hand left |

### 3.3 Tested readings

The ambiguities suspected in the draft are resolved as follows. ANTLR reports ambiguity only for the last row of the table.

| Entry | Less | Reason |
|---|---|---|
| `a < b > c;` | Declaration of `c` with type `a<b>` | Comparison is not a valid statement |
| `x = a < b > (c);` | Comparison | In terms, there are no type arguments |
| `List<List<Foo>> x = ...;` | Declaration | No own token `>>` |
| `Foo? f = flag ? a : b;` | Conditional expression declaration | `Foo? f` is not an expression |
| `a?.b();` | Zero call | `?.` is its own token; `.5` is not a literal |
| `(i32 a, i32 b) = f();` | Tuple Declaration | Type followed by name |
| `(i32, i32) t = f();` | Variable with tuple type | |
| `(a, b) = f();` | Allocation of existing variables | |
| `f((i32 x) => x + 1, y);` | Lambda with hull `x + 1`, then second argument | Lambda at expression level |
| `List<Foo>::create();` | Static call | After `>` follows `::`, this can not be a comparison |
| `x = flag ? Color::Red : Color::Blue;` | Condition expression with two static accesses | `::` is its own token and is never read as two `:` |
| `x = a < b > c.d();` | Comparison | After `>` there is no `.` |
| `x = max < a > (b);` | Comparison | Type arguments are not written on the call |
| `a[N] b;` | Declaration of a fixed array | Name after type |
| `a[N] = b;` | Assignment to an element | `=` According to the bracket |
| `if (a) if (b) f(); else g();` | `else` Belongs to the inner `if` | Only reported ambiguity, usually resolved |

### 3.4 Test run

Status of the last run: 18 positive files, 127 negative tests, 80 readings, 258 conformance tests (syntax; called 234 out of 245 rules, 11 justified without test); 0 errors, 0 unexpected ambiguities. The parser was generated with antlr-ng 1.0.10 (Python target); the grammar is not tested with the Java tool of ANTLR. The Rust parser `rppc` (milestone M1) reads the same cases with the same result.

Rules that the grammar does not express and that a later checking step must adopt:

- A printout statement without assignment must end with call, `++` or `--` (already checked in the test run).
- `own` Only on parameters and lambdas.
- `var` Only with initialization.
- `void` as a type argument only in the last place of `Func`.
- Package name matches the file directory.
- The size of a fixed array is a constant that is fixed at compile time.
- `Slice<T>` Never as a field type.
- Public members of exported types use only exported types in their signature.
- Packages of a library start with the library name.
- At most one `main` in the program.
- The initializer of a `ref` declaration is a location (variable, field or index access), not a temporary value.
- Before `::` is a type; before `.` a value or a package name. `Foo.create()` For a static method, therefore, an error is only at the name resolution because `foo.create()` looks syntactically the same.

### 3.5 Findings

All points that are noticed during testing have been decided (see “R++ Design Decisions”, Section 16). consciously, it is accepted `a > > b` with space as the shift operator.

## 5. Ownership and Borrowing

Prepared in the document “R++ Ownership Core Calculus” (Draft 0.5):

- Core language FR++ with classes, fields, methods, `own`, `shared`, `weak`, reading and changeable borrow.
- Type rules R-5.1 to R-5.24 as inference rules; R-5.23 and R-5.24 allow Borrows starting from `this` as return. Borrows live lexically (call or block); the compiler carries a lot of active loans instead of lifetimes.
- Big step semantics with heap, drop flags and a locked state for `shared` objects.
- Theorems 5.1 to 5.3 (storage security, iterators, no runtime test without `shared`) with sketch of evidence; complete proof is pending.
- Image of the surface language on FR++.
- Conformance testing in `conformance/ch05_ownership/` (6 accepted, 20 rejected programs).
- Executable model in `fr-model/`: Type checker and interpreter for FR++ with invariant testing, 28 handwritten programs and random tests. In the last run (2000 programs, 816 adopted) there was no violation of sentence 5.1 or 5.3; cross-checks with switched off rules show that the rules prevent real errors (core calculus, section 10).

The necessary determinations V-1 to V-11 and the spelling `ref` for named local borrows have been decided (see “R++ Design Decisions”, section 17).

## Next steps

1. Continue M7: M6 is complete (the compiler in R++ translates itself). M7 makes the translated code faster by removing the C code generator (implementation plan, Section 6); the proposed target value (translated `rppcc` at least as fast as `rppc` at `diagnose rpp-compiler`) has been reached since 26.11.2026 (0.40 s against 0.47 s). Open: set the target value and decide whether the changes to the tester (loops and samples) are carried over to `rppc`.
2. Establish a library directory for the conformance tests to make R-11.19 and R-11.20 auditable.
3. Prove sentence 5.1 by machine (Coq, Lean or Isabelle); the executable model serves as a reference.
