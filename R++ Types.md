# R++ Types (Chapter 4)

[Deutsch](R%2B%2B%20Types.de.md)

Status: 11/21/2026, Draft 0.3. This document fills out Chapter 4 of the R++ Specification Outline, which specifies what types there are, what values they have, how they are converted into each other, and what operators apply to them. How types interact with ownership and borrows is governed by Chapter 5 (“R++ Ownership Core Calculus”); how nullable values are checked, Chapter 6.

Each normative rule has a number. `R-4.n` Conformance tests shall be carried out in: `conformance/ch04_types/`.

## 0. New provisions in this chapter

Most of the rules follow from the “R++ Design Decisions” (sections 2, 4, 6–11 and 18), and some minor points had to be defined in addition to formulating them, all of which were decided on 20.11.2026 (see “R++ Design Decisions”, section 18):

| No | Settlement |
|---|---|
| V-4.0 | Correction: `i32 + ui32` has the type `i64` under the rule "common type" (R-4.12) and is not an error; it is only an error if no common type exists, e.g. `i64 + ui64` or `i64 + f64`. |
| V-4.1 | `char` has comparisons, but no arithmetic. `cast<char>(n)` With a value that is not a Unicode scalar value, throws an exception in **both** modes because an invalid `char` UTF-8 strings would spoil. |
| V-4.2 | Shift operators: A shift count outside `0` to bit width −1 throws an exception in debug mode; in release mode it is taken modulo bit width. `>>` Slides arithmetic, otherwise logic for signed types. |
| V-4.3 | Bit operators ()`&`, `\|`, `^`, `~`, `<<`, `>>`) is only available for integers and `byte` For `bool` apply only to `!`, `&&`, `\|\|`, `==` and `!=`. |
| V-4.4 | Strings are immutable. `+` combines two strings; numbers do not automatically become strings. `==` and `!=` compare the content, `<`, `<=`, `>`, `>=` lexicographically according to Unicode scalar values. |
| V-4.5 | Equality of objects over the generic library mixin `Equatable<T>` with requirement `bool equals(T other)`. Primitive types, `string` and enums without data are considered `Equatable`. Identity verifies the built-in function `same(a, b)`. |
| V-4.6 | `e as C`: `C` must be true sub-type of the static type of `e`, otherwise compiler error; the result is a Borrow `C?`; if `e` is a `shared` value and the target is express `shared`, it is `shared C?` (counter + 1). A downcast with transfer of ownership is not available in version 1. |
| V-4.7 | Enum variants without value get the value of the previous variant + 1, the first one the value 0. |
| V-4.8 | Tuples, arrays and function types do not have `==`. Tuple literals take the element types from context; otherwise, tuple types must match exactly. |
| V-4.9 | There is no automatic conversion between fixed arrays and vectors; both are only automatically borrowed as `Slice<T>`; the size of a fixed array is ≥ 0. |
| V-4.10 | `cast` from integer to floating point and from `f64` to `f32` rounds to the nearest representable value (when tied to even mantissa) and never throws; too large values become ±∞. |
| V-4.11 | `null` needs a nullable context: `var x = null;` is an error. `T??` and `void?` does not exist. |
| V-4.13 | (adopted on 21.11.2026) Clarification on R-4.12: If both operands are integers, only integer types can be considered as a common type. Without this sentence `i32`, `ui32` would not have a common type, because `i64` and `f64` both fit and neither can be converted into the other; however, the table in R-4.12 mentions `i64`. |
| V-4.12 | (assumed on 21.11.2026) An enum value is created without `new`: `Color::Red` for a variant without data, `Shape::Circle(2.0)` for one with data (R-4.25). |

## 1. Summary

| Type type | Examples | Section |
|---|---|---|
| Truth value | `bool` | 2 |
| integers | `i8` … `i64`, `ui8` … `ui64` | 2 |
| floating-point | `f32`, `f64` | 2 |
| Characters, raw data | `char`, `byte` | 2 |
| Character string | `string` | 5 |
| Classes and Mixins | `Foo`, `Printable`, `List<Foo>` | 6 |
| Enums | `Color`, `Shape` | 7 |
| Fixed arrays, vectors, slices | `i32[16]`, `Foo[]`, `Slice<i32>` | 8 |
| Tulips | `(i32, string)` | 9 |
| Functional types | `Func<i32, bool>` | 10 |
| Zero types | `Foo?`, `i32?` | 11 |
| Type parameters | `T`, `T : Comparable` | 12 |
| `void` | only as a return type and last argument of `Func` | 13 |

Class, array, tuple and function types can be preceded by a ownership mode (`own`, `shared`, `weak`) or by parameters `mut`; the meaning of which is governed by Chapter 5.

## 2. Primitive Types

**R-4.1 (Value ranges).**

| type | Values |
|---|---|
| `bool` | `true`, `false` |
| `i8`, `i16`, `i32`, `i64` | Two-complement with 8, 16, 32, 64 bits |
| `ui8`, `ui16`, `ui32`, `ui64` | 0 to 2n-1 with n=8, 16, 32, 64 |
| `f32`, `f64` | IEEE 754 binary32 and binary64, respectively, including ±0, ±∞ and NaN |
| `char` | Unicode scalar values: 0 to 0x10FFFF without 0xD800 to 0xDFFF |
| `byte` | 8 bits of raw data |

**R-4.2 (Copy).** Primitive type values are copied at assignment, handover and return; they have no ownership mode.

**R-4.3 (bool).** Conditions of `if`, `while`, `do`-`while`, `for` and `? :` as well as the operands of `!`, `&&` and `||` must be of type `bool`. There is no transformation between `bool` and other types, including with `cast`.

**R-4.4 (char, V-4.1).** For `char` are defined `==`, `!=`, `<`, `<=`, `>` and `>=` (by scalar value), but no arithmetic. `cast<T>(c)` with an integer type `T` provides the scalar value (with range checking according to R-4.9). `cast<char>(n)` throws an exception in both modes if `n` is not a scalar value.

**R-4.5 (bytes).** `byte` is a separate type for raw data. allowed are `==`, `!=`, `<`, `<=`, `>`, `>=` and the bit operators (V-4.3); arithmetic is an error. `cast<ui8>(b)` and `cast<byte>(u)` with `u : ui8` convert losslessly and never throw. There are no other conversions from and to `byte`.

## 3. Conversions between Number Types

**R-4.6 (Automatic conversion).** A numeric value is automatically converted to another numeric type if each value of the source type in the target type can be represented exactly, which results in exactly these conversions:

| From | Automatically after |
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

`bool` `char` and `byte` are never automatically converted. The automatic conversion applies to assignment, initialization, argument handover, return and operands (R-4.12).

**R-4.7 (Literals).** A number literal is of the type required by the context, and the compiler checks whether the value fits (`i8 b = 300;` is an error; `f32 x = 0.1;` is allowed and rounds); without context is an integer literal `i32`, if the value is too large `i64`, and a floating point literal `f64`. Gliding-pointerals may have an exponent (`6.022e23`, `2.5E-3`, see chapter 2); if the value in the target type is not finitely representable (`f32 x = 1.0e39;`), this is an error; if a literal as an operand stands next to a value of another number type, this type is the context (`x + 1` counts with `x : i8` in `i8`).

**R-4.8 (explicit conversion).** `cast<T>(e)` converts between number types in any direction, also between `char` and integers (R-4.4), `byte` and `ui8` (R-4.5) as well as enums with numerical values and integers (R-4.27).

**R-4.9 (cast between integers).** If the value fits into the target type, it is retained. Otherwise: Exception in debug mode, in release mode the lower bits are retained (complement of two).

**R-4.10 (cast of floating point to integer).** If the result is outside the target type, or if the value NaN is Exception in debug mode; saturation to the largest or smallest value of the target type in release mode, NaN becomes 0.

**R-4.11 (cast to floating point, V-4.10).** Integers → floating point and `f64` → `f32` round to the nearest representable value, tie to the straight mantissa.

## 4. Operators on primitive types

**R-4.12 (common type, V-4.0).** For two-digit arithmetic operators and comparisons, both operands are converted to their **common type**: the type `T`, into which both can be automatically converted (R-4.6) and which itself can be converted to any other such type; if both operands are integers, only integer types come into consideration (V-4.13). If there is no, the expression is an error and requires a `cast`.

| operands | Common type |
|---|---|
| `i8`, `i32` | `i32` |
| `i8`, `ui8` | `i16` |
| `i32`, `ui32` | `i64` |
| `i32`, `f32` | `f64` |
| `i64`, `ui64` | None – Errors |
| `i64`, `f64` | None – Errors |
| `ui64`, `f32` | None – Errors |

As a result, even mixed comparisons are mathematically correctly evaluated: `cast<i32>(-1) < cast<ui32>(3_000_000_000)` is `true`.

**R-4.13 (Arithmetic).** `+`, `-`, `*`, `/`, `%` And the one-digits `-`, `++`, `--` There are integer and floating point types, and the result has the same type. `-` on a signless type is a mistake.

**R-4.14 (Overflow).** If the mathematical result of an integer operation is outside the result type, the debug mode throws an exception; the release mode continues to calculate modulo 2n. `i32::MIN / -1`.

**R-4.15 (Division).** Integer division rounds to zero; `a % b` has the sign of `a` and applies `(a / b) * b + a % b == a`.Integer division and `%` by 0 cast an exception in both modes. floating point operations follow IEEE 754 (division by 0 gives ±∞ or NaN).

**R-4.16 (bit operators V-4.2, V-4.3).** `&` `|`, `^`, `~` exist for integer types (common type) and `byte`. `<<` and `>>` have the type of left operand; the right one is an integer. If the shift count is outside 0 to bit width −1: Exception in debug mode, in release mode it is taken modulo bit width. `>>` is arithmetic for signed types, otherwise logical.

**R-4.17 (Comparisons).** `==`, `!=`, `<`, `<=`, `>`, `>=` there are for number types (common type), `char` and `byte`; `==` and `!=` in addition to: `bool` The result is `bool` Comparisons with NaN are except `!=` always `false`.

## 5. String

**R-4.18 (Values).** A `string` is an immutable sequence of Unicode scalar values, internally UTF-8. Strings are implicitly divided and do not participate in ownership and borrows (chapter 5); assignment and handover only copy the reference.

**R-4.19 (Access).**

| Expressions | type | Significance |
|---|---|---|
| `s.length` | `i64` | Number of characters (scalar values) |
| `s[i]` | `char` | i-th character, from 0 |
| `s.byteLength` | `i64` | Number of bytes of UTF-8 representation |
| `s.byteAt(i)` | `byte` | i-th byte, from 0 |
| `for (char c : s)` | | Iteration over the signs |

Non-range indices throw an exception. `s[i] = c` is an error because strings are immutable. `s.length` and `s[i]` UTF-8 may take time proportional to length; an implementation may cache auxiliary data for this.

**R-4.20 (Operators, V-4.4).** `a + b` connects two strings. Other operands are not automatically converted (`"n=" + 3` is an error). `==` and `!=` compare the content; `<`, `<=`, `>`, `>=` compare lexicographically by scalar values.

## 6. Classes and Mixins

**R-4.21 (Nominal types).** Each class and each mixin is its own type, two types of classes are the same only if they have the same declaration and the same type arguments.

**R-4.22 (Subtypes).** `C <: D` shall apply if
- `C = D` or
- `C` inherits directly from `D` (`class C : D`), or
- `C` imported the mixin `D` (`import D;`), or
- There is one `E` with `C <: E` and `E <: D`.

A value of type `C` It can be anywhere where `D` is required, this applies with the same ownership mode (Chapter 5, Section 4.4). `own`-Object passed, it is moved.

**R-4.23 (Downcast, V-4.6).** `e as C` checks at runtime whether the object that identifies `e` has the type `C` (or a subtype thereof).
- `C` is a class or mixin type without `?` and true subtype of the static type of `e`; otherwise, the expression is an error.
- If `e` is a place, the result is a borrow of type `C?`: `ref Circle? c = s as Circle;` It is considered to be borrow on `e` for the purposes of Chapter 5.
- If `e` of type `shared D` and the target is express `shared`, the result is `shared C?` and increments the counter: `shared Circle? c = s as Circle;`.
- If the type does not fit, the result is `null`. `as` Never throws.
- A downcast that passes ownership (`own Base` → `own Derived`) does not exist in version 1.

**R-4.24 (equality of objects, V-4.5).** `a == b` and `a != b` are allowed for objects only if the class of `a` the library mixin `Equatable<T>` can be imported and `b` converted into `T`. `a == b` means then `a.equals(b)`, `a != b` means `!a.equals(b)`. Comparisons of a nullable value with `null` are always allowed (Chapter 6). The built-in function `same(a, b)` provides `true` when `a` and `b` designate the same object; it takes two borrows on objects.

## 7. Enums

**R-4.25 (Variants).** An enum is a nominal type with a fixed set of variants; a variant carries either data (`Circle(f64 radius)`) or a fixed number value (`Red = 1`) or nothing; an enum with numerical values has no variants with data. A value arises without `new` as `E::V` or, with data, as `E::V(a₁, …, aₙ)`; the arguments are passed as in a constructor (V-4.12).

**R-4.26 (numerical values, V-4.7).** Numerical values are unique integer literals, even negative, in the range of `i32` and within an enum. A variant without value in an enum without data has the value of the previous variant + 1, the first one has the value 0.

**R-4.27 (Conversion).** Enums are never converted automatically. Delivered for an enum without data `cast<T>(e)` With an integer type `T` the number of the variant (range check after: R-4.9). `cast<E>(n)` provides the variant with the value `n` and throws an exception in both modes if there is none. There is no conversion for enums with data.

**R-4.28 (Equality).** `==` and `!=` are allowed for enums without data and compare the variant. `switch`.

**R-4.29 (Ownership).** An enum value has its data. enums without data are copied like primitive values; enums with data behave like objects (chapter 5).

## 8. Fixed arrays, vectors and slices

**R-4.30 (types).** `T[N]` is a fixed array with `N` elements; `N` is an integer literal or `static const` integer constant ≥ 0. `T[]` is a vector. `Slice<T>` is a borrowed view of a section of a fixed array or vector. All elements have the type `T`. `T[N]` and `T[M]` are only the same type at `N = M`.

**R-4.31 (conversions, V-4.9).** `T[N]` and `T[]` are automatically borrowed as `Slice<T>` (reading; variable with `mut`). Between `T[N]` and `T[]` there is no automatic conversion. Arrays are invariant: `Circle[]` is not `Shape[]`. The ownership of the elements must be consistent: `Box[]` is not `Slice<shared Box>` and `shared Box[N]` is not `Box[N]`, otherwise there would be shared references to objects released with their container (Chapter 5, V-9). For element types that are not shifted (basic values, `string`), the form means nothing; `i64[]` is also a `Slice<shared i64>`.

**R-4.32 (Literals).** An array literal `[a, b, c]` has the type that the context requires; each element is converted to the element type; for fixed arrays, the number must be correct. `[]` Needs a context.

**R-4.33 (Length and index).** `.length` has the type `i64` Indices are integers that are automatically `i64` can be converted; an index outside of `0` up to `length − 1` throws an exception. Arrays have no `==` (V-4.8).

## 9. Tuples

**R-4.34 (types).** `(T₁, …, Tₙ)` where n ≥ 2 is a type of tuples; two types of tuples are the same if they have the same number of elements, each with the same type; there are no subtypes between tuples.

**R-4.35 (Use).** Tuples are created as a return value (`(i32, string) f()`) or a literal (`(1, "a")`). A tuple literal takes over the element types from the context (V-4.8), and a tuple is only decomposed: `(i32 a, string b) = t;` or `(a, b) = t;`. An element access such as `t.0` does not exist in version 1, neither does `==`.

**R-4.36 (Ownership).** A tuple possesses its elements, and when it is broken down, it passes to the new variables. Using the same spelling, a `own` object of a class without a destructor can be broken down into its fields (Chapter 5, R-5.25).

## 10. Functional types

**R-4.37 (Func).** `Func<P₁, …, Pₙ, R>` is the type of a function with parameters `P₁ … Pₙ` and return type `R`; `Func<R>` has no parameters. `void` shall be only in last place. Parameters may be marked as method parameters (`Func<mut Foo, void>`, `Func<own Foo, void>`; Chapter 10, R-10.13). Function types are structural and invariant: `Func<A, B>` and `Func<C, D>` are the same only at `A = C` and `B = D`.

**R-4.38 (Values).** Values of a function type are derived only from lambdas. A parameter of type `Func<…>` takes normal and `own` lambdas; variables, fields and return values take only `own` lambdas (Chapter 5, Section 7). Function types do not have `==`.

## 11. Nullistic types

**R-4.39 (Education, V-4.11).** For each type `T` except `void` there is the nullable type `T?`; it contains all values of `T` and `null`. `T??` There is no such thing as primitive types (`i32?`), tuples (`(i32, string)?`), arrays (`Foo[]?`) and ownership forms (`shared Foo?`, `weak Foo?`). `Foo?[]` is a vector of nullable elements, `Foo[]?` a nullable vector.

**R-4.40 (Conversion).** `T` is automatically converted to `T?`; the conversion goes back only through the tests in Chapter 6. The literal `null` needs a context of nullable type; `var x = null;` is an error.

**R-4.41 (Slight).** `weak Foo` is a reference that is never `null` but whose goal can be released. `w.lock()` therefore always provides `shared Foo?`. A `weak` value arises only from `shared` or `weak` (Chapter 5, V-6, V-13); `weak Foo w = new Foo();` is an error.

## 12. Type 12 parameters

**R-4.42 (Constraints).** A type parameter `T` is not applicable to any type `void`, even primitive types. With constraints `T : M₁ & … & Mₙ`, the type argument must be sub-type of each `Mᵢ`. In the body on `T` only operations are allowed which the constraints promise; `==` required `T : Equatable<T>`. Primitive types, `string` and enums without data fulfill `Equatable` by themselves.

**R-4.43 (Invariance).** Generic types are invariant: `List<Circle>` is not a subtype of `List<Shape>`, even if `Circle <: Shape`.

**R-4.44 (Derivatives).** In generic methods, the type arguments are derived only from the arguments; if a type parameter cannot be clearly derived, this is an error.

## 13. Void

**R-4.45.** `void` is only as a return type of a method and as the last type argument of `Func` There are no values and no variables of the type `void`.

## 14. Initialization and Type Inference

**R-4.46 (Secure initialization).** A local variable must be assigned on each path before it is read; there are no default values; `i32 x; print(x);` is an error.

**R-4.47 (Fields).** A nullable field without initializer is `null`.Any other field without initializer must be set in the constructor before `this` is used differently (V-10 from Chapter 5).

**R-4.48 (var).** `var x = e;` there are `x` the type of `e`, including ownership mode: `var f = new Foo();` is: `own Foo`, `var s = t;` with `t : shared Foo` is: `shared Foo`A literal with no further context gets the standard type from R-4.7. `var` Requires an initializer.

## 15. Conformance tests

The test programs are in `conformance/ch04_types/`. The syntax checks `rpp-grammar/tools/run_tests.py`, the semantics `rppc test` (first implementation).

| Rules | Accepted | Rejected |
|---|---|---|
| R-4.3 bool | `a01_primitive.rpp` | `n01_int_as_condition.rpp`, `n02_cast_to_bool.rpp` |
| R-4.4 R-4.5 char, byte | `a01_primitive.rpp` | `n03_byte_arithmetic.rpp`, `n04_char_arithmetic.rpp` |
| R-4.6 R-4.7 Conversion, Literals | `a02_conversions.rpp` | `n05_i64_to_f64.rpp`, `n06_literal_out_of_range.rpp` |
| R-4.12 Common type | `a02_conversions.rpp` | `n07_no_common_type.rpp` |
| R-4.13 Arithmetic | – | `n08_unsigned_negation.rpp` |
| R-4.19 R-4.20 string | `a03_string.rpp` | `n09_assign_to_string_index.rpp`, `n10_string_plus_number.rpp` |
| R-4.23 R-4.24 as, Equality | `a04_classes.rpp` | `n11_as_unrelated_type.rpp`, `n12_equality_without_equatable.rpp` |
| R-4.26 – R-4.28 Enums | `a05_enums.rpp` | `n13_duplicate_enum_value.rpp`, `n14_compare_data_carrying_enum.rpp` |
| R-4.31 Arrays | `a06_arrays_tuples.rpp` | `n15_array_covariance.rpp`, `n28_slice_ownership.rpp` |
| R-4.35 Tuple | `a06_arrays_tuples.rpp` | `n16_tuple_comparison.rpp` |
| R-4.40 Zero | `a07_nullable.rpp` | `n17_var_null.rpp` |
| R-4.43 Invariance | – | `n18_generic_covariance.rpp` |
| R-4.46 Initialization | `a07_nullable.rpp` | `n19_read_before_assignment.rpp` |
| R-4.45 void | – | `n20_void_before_end.rpp` |
