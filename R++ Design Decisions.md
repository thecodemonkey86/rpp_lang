# R++ Design Decisions

[Deutsch](R%2B%2B%20Design%20Decisions.de.md)

This document specifies what has been decided for the R++ language and what is still open. It is the basis for the document “R++ Specification Outline”.

Status abbreviation: **[E]** expressly decided, **[A]** proposed and accepted without objection, **[O]** open.

## 1. Objectives

- [E] Debug Mode: Code is interpreted or executed via JIT in a VM, for short development cycles.
- [E] Release mode: Compilation to native executables.
- [E] Certain error classes are formally excluded (see Sections 4 and 5).
- [E] Object oriented with classes; syntax close to C/C++/Java/C#, but simpler.
- [E] No macros, but conditional compilation. No header files.
- [A] Both modes share frontend and intermediate representation. Type and ownership check also run in debug mode.
- [A] The debug interpreter serves as a reference implementation against which the native compiler is tested.

## 2. Base Types

- [E] `void`, `bool`, `char`, `byte`, `string`
- [E] Integers: `i8`, `i16`, `i32`, `i64`, `ui8`, `ui16`, `ui32`, `ui64`
- [E] floating point: `f32`, `f64`
- [E] Primitive values are always copied.
- [E] Strings are implicitly shared (excluded from the ownership mechanism) and internally UTF-8.
- [A] `char` is a 32-bit Unicode scalar value.
- [A] Lengths and indices are of type `i64`; there is no type of their own like `usize`.
- [A] Integer overflow: Exception in debug mode, defined wraparound in release mode.
- [E] Conversion between number types: automatically only if each value of the source type is exactly preserved in the target type, e.g. `i8` → `i32`, `ui32` → `i64`, `i32` → `f64`, `f32` → `f64` Not automatically: all downcasts and `i32` ↔ `ui32`, `i64` → `f64`, `i32` → `f32`.
- [E] Explicit Casts: `cast<i32>(x)`.
- [E] Does the value at `cast` not fit into the target type: Exception in debug mode, cutting off the upper bits in release mode (as in the case of integer overflow).
- [E] Number literals get their type from context; the compiler checks whether the value fits (`i8 b = 300;` is an error); without context: integers `i32`, if too large a value `i64`; floating point numbers `f64`.
- [E] Numbers may be separated by `_` (`1_000_000`), but not at the beginning or end. Type suffixes like `42i64` do not exist.
- [E] `bool` is strict: conditions must be `bool`; between numbers and `bool` there is no conversion, even with `cast`.
- [E] `byte` is a separate type for raw data: comparisons and bit operations yes, arithmetic no. Conversion to and from `ui8` with `cast`.
- [E] Mixed arithmetic: both operands are converted to their common type (the smallest type into which both can be automatically converted; `i8 + i32` → `i32`). Integer division and `%` throw by 0 in both modes; division rounds to zero; `%` has the sign of dividend; floating point according to IEEE 754.
- [E] `cast` from floating point to integer cuts the decimal places. Outside the range or for NaN: Exception in debug mode, saturation in release mode (NaN becomes 0).
- [E] `string`: `s.length` is the number of signs, `s[i]` The i-te provides the i-te `char`. Additional `s.byteLength` and `s.byteAt(i)` for the UTF-8 bytes.

## 3. Ownership and Borrowing

- [E] Three ownership modes: `own` (clear ownership, handover by move), `shared` (reference count), `weak` (weak reference, against cycles).
- [E] Parameters without marking are borrowed (borrow) and readable only. Objects are never implicitly copied; copying is explicit.
- [E] Borrows are not allowed to be stored in fields, so there are no lifetime annotations.
- [A] Consequence: fields and local variables without marking have their object uniquely. `own` Stands only on parameters and lambdas; `shared` and `weak` can stand anywhere.
- [E] Loanable is marked with the keyword `mut` before the parameter type: `void rename(mut Foo f)` (changed on 22.11.2026, before `Foo& f`).
- [A] The call point also says `mut`: `rename(mut foo)`.
- [A] Only borrowed parameters are marked. Local variables and fields can be changed without marking.
- [E] Local variables that only borrow are declared as `ref`: `ref Foo f = list[0];`, mutable `ref mut Foo g = mut x;` (see Section 17).
- [A] Methods that change `this` are marked `mut` after the parameter list: `void setName(string n) mut { ... }`.
- [A] Exclusivity: As long as a `mut` Borrow exists, no other access to the object is allowed.

## 4. Null safety

- [E] Types are not nullable by default. Nullable types have a question mark: `Foo?`.
- [E] The compiler reports an error when a nullable value is dereferenced without prior checking, thus excluding null dereferences.
- [E] Type narrowing test: After `if (x != null)` `x` is considered non-nullable in the block; after `if (x == null) { return; }` this is valid for the rest of the function.
- [E] Abbreviations: `x?.methode()` Calls only when `x` is not null; `x ?? ersatz` delivers `ersatz` when `x` is null.
- [E] There is no operator that skips the test (no `x!`).
- [A] The narrowing applies only to local variables and parameters. A nullable field must be fetched into a local variable before access.
- [E] Primitive types and tuples can also be nullable (`i32?`, `(i32, string)?`); with ownership `shared Foo?` and `weak Foo?`. `weak Foo` without `?` is a reference whose goal may be released; `lock()` always provides `shared Foo?`.

## 5. Excluded error classes

- [A] At compile time: use-after-free, double-free, zero dereference, invalid iterators.
- [A] At runtime by exception: Index error for arrays.
- [E]Clarification (21.11.2026): Such runtime tests of the language do not trigger a catchable exception, but a program error that terminates the program after cleaning up (Section 22).
- [A] Data Races: excluded in version 1 by the fact that each object belongs to exactly one thread (section 12).

## 6. Functions and Return Values

- [E] Classical definition without `fn`: `<Rückgabetyp> name(<Typ> p1, <Typ> p2) { ... }`
- [E] Multiple return values in tuple lettering: `(i32, string) f()` and `(i32 a, string b) = f();`
- [E] Tuples are only disassembled in version 1; there is no element access like `t.0`.
- [E] No freely definable functions outside classes; maximum built-in functions such as: `print` The only exception is `main`.
- [E] Static methods with the keyword `static`.
- [E] Static members are addressed with `::` and instance members with `.`: `Math::max(a, b)`, `Shape::PI`, `List<Foo>::create()`; the same applies to enum variants (`Color::Red`, `case Shape::Circle(r):`) and array sizes from other classes (`i32[Limits::N]`). The reason is readability: static access is recognizable at first glance, and packet path and member access are clearly separated (`my.pkg.Registry::count()`).
- [E] `main` is handled separately: it can also be defined outside of a class and returns the exit code. `main` contains.
- [E] Signature of `main`: `i32 main(string[] args)`.
- [E] Static fields are only available in version 1 as constants (`static const`). Variable static fields would be global states and do not conform to exclusivity and the thread rule.

## 7. Arrays, Vectors and Slices

- [E] Three separate types: fixed arrays, dynamic arrays (vectors) and slices.
- [E] Fixed array: `i32[16]` or `i32[N]` with a `static const` constant `N`. The size is fixed at compile time; the elements are directly in the variable or in the object. No `push` or `pop`.
- [E] Vector: `T[]` grows dynamically with methods such as `push`.
- [E] Array literals `[1, 2, 3]` become the type that the context requires; for fixed arrays, the length must fit.
- [E] Slice: Library type `Slice<T>`, a borrowed view of a portion of a fixed array or vector. `mut Slice<T> s`.
- [E]slices are borrows: allowed as parameters and local variables, never as fields.
- [E] As long as a slice exists on a vector, the vector cannot grow or shrink (exclusivity rule).
- [E] Areas with `..`, end exclusive: `a[1..3]`, `a[..3]`, `a[1..]`, `a[..]`.
- [E] Fixed arrays and vectors are automatically borrowed as slices: `sum(fixed)` and `sum(vec)` call the same method `sum(Slice<i32> values)`.
- [E] All three types have `.length`, tested index access and work in `for (x : a)`.
- [A] Unified element type; an empty `[]` gets its type from the context.
- [A] access outside the borders throws an exception; there are no gaps.
- [A] An array has its elements. `arr[i]` delivers a borrow for objects; taking it out of a vector only via `pop()` / `remove(i)` or by copy.
- [A] `push` and other altering methods need `mut` access.

## 8. Classes and heredity

- [E] Classes with `constructor(...)`, visibility over sections `public:`, `private:`, `protected:`.
- [E] Explicit copy of `copy(MyClass other) { ... }`.
- [E] Call of the copy: `Foo b = copy(a);` provides a new object with unique ownership, even if `a` `shared` or only borrowed. If the class does not `copy(...)`, this is a compiler error; thus, classes can be blocked against copying.
- Inheritance in C# spelling: `class A : B`.
- [E] Object generation with `new`: `Foo f = new Foo(...);`
- [A] Simple inheritance plus mixins; overwrite only with the keyword `override`.
- [E] Call of the base constructor as in C#: `constructor(i32 x) : base(x) { ... }`
- [E] Destructor: `destructor() { ... }`
- [E]Classes can have virtual and abstract methods that are overwritten or implemented in subclasses.
- [E] Spelling as in C#: Keywords `virtual` and `abstract`. Methods without `virtual` are not overwritable. A class with abstract methods is declared `abstract class` and cannot be generated with `new`. `override` is also mandatory when implementing an abstract method.
- [E] Properties as in C#(`int number {get; set;}`): deferred to a later version.
- [E] Downcasts as a checking query with the new keyword `as`: `ref Circle? c = s as Circle;` provides `null` when `s` is not `Circle`; then the null check takes effect.
- [E] `==` on objects is only allowed if the class imports the mixin `Equatable`; then `==` calls the method `equals`. Comparison with `null` is always allowed. Identity checks the built-in function `same(a, b)`.

## 9. Mixins

- [E] A mixin brings only methods, not its own state. `import MyMixin;` In the class it acts as if the methods were defined directly there.
- [E] A mixin declares its requirements for the class as methods without a hull. The compiler checks the mixin on the definition and only the requirements when imported.
- [E] A mixin is also a type: it replaces interfaces and serves as a constraint for type parameters (`class List<T : Printable>`).
- [A] Name conflicts between Mixins or with the class are a compiler error that must be explicitly resolved.
- [E] A conflict is resolved by the class itself defining the method with `override`; in this `Printable::describe()` specifically calls the version of a mixin (section 20).

## 10. Type 10 parameters

- [E] Type parameters are possible without the problems of C++ templates and Java type erasure.
- [A] Implementation: Generics are checked against their constraints on the definition. Type arguments are retained in the interpreter at runtime, and in the release their own code is generated per type.
- [E] Several constraints are connected to `&`: `class SortedList<T : Comparable & Printable>`.
- [E] Static access to a generic type: `List<Foo>::create()`.
- [E] Generic types are invariant: `List<Circle>` is not `List<Shape>`.
- [E] Generic methods: `static T max<T : Comparable>(T a, T b) { ... }` Type arguments are only derived from the arguments when called, never written out (`max<i32>(a, b)` would be a comparison).

## 11. Control structures, enums, exceptions, lambdas

- [E] `if`, `for`, `while`, `do`-`while`, `switch`-`case` as in the C family.
- [A] `switch` without fallthrough and with completeness check.
- [A] Enums with optional data per variant.
- [E] variants without data may have a fixed numerical value: `enum Color { Red = 1, Green = 2 }`. `cast<i32>(c)` provides the value (without indication the position), `cast<Color>(n)` flips when no value fits. `==` is available for enums without data.
- [A] As a stand-alone expression statement, only calls, assignments and `++`/`--` are allowed (such as Java/C#).
- [E] Assignments (also `+=` etc.) are only statements, not expressions; `++` and `--` may also be in expressions (section 21).
- [E] Exceptions.
- [A] Exceptions are unchecked (no `throws` lists); `try`/`catch`/`finally`; Destructors run at Unwinding.
- [E] Only `own` objects of subclasses of `rpp.core.Exception` are thrown; program errors are not catchable (Section 22).
- [E] Lambdas in C# spelling: `(i32 a) => ausdruck` or `(i32 a) => { ... }`.
- [E] Normal Lambda: captures variables automatically by borrow (reading or changing, depending on the body) and may only be passed as an argument, not saved.
- [E] `own`-Lambda (`own (i32 x) => x + offset`): recorded by value. Primitive values are copied, strings and `shared` objects are shared, `own` objects are moved in. Only `own`-Lambdas may be returned or stored.
- [E] Function types: `Func<Param1, ..., Rückgabetyp>` Parameters bear the same markings as method parameters: `Func<mut Foo, void>` borrows variable, `Func<own Foo, void>` adopts (Section 23).
- [E]Loop over arrays: `for (string c : colors) { ... }`; the element is borrowed. `for (mut Foo f : mut list) { ... }`.
- [E] Function types without return value: `Func<i32, void>`; without parameters: `Func<void>` or `Func<string>`. `void` is allowed as a type argument only in the last place of `Func`.

## 12. Threads and C-Connection

- [A] Version 1: Each object belongs to one thread; transfer to another thread only by `own`.
- [A] C-link (FFI): reset until the voice core stands.

## 13. Source files and project structure

- [E] Source files only. `package my.pkg;` At the beginning of the file.
- [E] Package names must not contain keywords (`package my.package;` is an error).
- [E] Optional `using` statements to use classes without fully qualified names.
- [E] The package name must correspond to the directory relative to the source folder: `src/my/pkg/Foo.rpp` contains `package my.pkg;`.
- [E] Project file `rpp.yaml` in the project directory with entries `name`, `version`, `type` (`executable` or `library`), `main` (for programs only: source file with `main`), `sources` (source folder), `dependencies` and `symbols` (own icons per mode).
- [E] Dependencies are entered in `dependencies`: local with `name` and `path`, from the central register with `name` and `version` (Section 13b).
- [E] The packages of a library begin with their name (`mathlib.geometry`), so two libraries can never contain the same package.
- [E] Only directly entered dependencies can be used. Types from the dependencies of a library can only use a project if it enters them itself.
- [E] Libraries are shipped in the interim representation, sharing the debug and release modes, so that the release compiler can generate generic types of a library for new type arguments.
- [E]Clarification (21.11.2026): Because `#if` is resolved before parsing, the intermediate representation depends on the mode and operating system. A library is therefore published as source text; the intermediate representation is built from it for each combination (section 13b). The compiler thus always has the intermediate representation available for generic types.
- [E] Conditional compilation over lines beginning with `#`: `#if debug`, `#else`, `#endif`. Inactive branches are skipped during parsing, i.e. checked neither syntactically nor semantically.
- [E] In addition `#elif`; conditions with `!`, `&&`, `||` and brackets, e.g. `#if debug && !windows`.
- [E] Predefined symbols: exactly one of `debug` or `release` and the operating system ()`windows`, `linux`, `macos` Own icons only in the project file, not in the code; there is no `#define`.

## 13a. Export

- [E] `export` ahead `class`, `mixin` or `enum` makes one guy usable in other projects. `export` A guy is only visible in his own project.
- [E] Members are not exported individually. In another project, a member is visible if his type is exported and the member is `public`; for subclasses in other projects, additionally `protected`.
- [E] Public members of exported types may use only exported types in their signature; otherwise, the compiler will report an error.

## Central directory for libraries

Points with [A] are from the proposal and were adopted without objection.

- [E] Name with Owner: A library in the directory is called `besitzer.name` (`acme.json`); the owner is a user or organization account; the library's packages begin with this name (`acme.json.parser`). Owner and library names must not contain keywords.
- [E] Version resolution after Minimal Version Selection (like Go): Each dependency is called a minimum version; for each library, the highest minimum version is taken. per program, each library exists only in one version; if dependencies require different main versions, this is an error. Two main versions side by side are only available under different names (`acme.json2`).
- [E] Source code is published. The directory reproducibly builds the intermediate representation for the six standard combinations (`debug`/`release` × `windows`/`linux`/`macos`). The compiler loads the appropriate variant; if it has its own symbols or a missing variant, it builds it itself.
- [A] Versions follow Semantic Versioning. `rpp publish` compares the exported interface (exported types with their `public` and `protected` members) to the previous version; removed or modified parts require a new main version, new parts a new minor version. Otherwise, the list shall reject publication.
- [A] Lock file `rpp.lock` with exact version and checksum of each library. A public log of all published checksums makes subsequent exchanges recognizable.
- [A] Published versions are immutable; instead of being deleted, they are marked as withdrawn. Existing lock files continue to build.
- [A] Entry in `rpp.yaml`: `dependencies` with `name` and `version` (minimum version) or `path`; optional `registry` for own or mirrored directory.
- [A] Not in version 1: optional functions of a library (such as `features` in Cargo), multiple versions of the same library in a program, scripts that run when built.
- [E] The standard library is under the reserved owner `rpp` (`rpp.collections`, `rpp.io`); no one else can publish under `rpp`.

## 14. Variables and Constants

- [E] `const` identifies constants.
- [E] Type inference with `var`: The type results from the initialization expression.
- [E] Exactly one variable per declaration; `i32 a, b;` does not exist. Several variables at once are introduced only by Tuple Declaration: `(i32 a, string b) = f();`
- [E] In the `for` head also a Tuple declaration is allowed: `for ((i32 i, i32 j) = (0, n); i < j; i++, j--) { ... }`
- [E] Local `const` variables may be calculated at maturity but not reallocated.
- [E] `const` applies low: No `mut` methods may be invoked on a `const` object.
- [E] `const` fields are set only in the constructor. `static const` Requires a value that is fixed at compile time.
- [E] Parameters do not carry `const`; without `mut` they are only legible anyway.
- [E] Secure initialization: Local variables must be assigned on each path before the first reading; there are no default values. Nullable fields without initializers are `null`; all others are set by the constructor.

## 15. Built-in functions and standard library

- [E] `assert` is tested in both modes and throws at `false` (section 19).
- [E] Built-in functions in version 1: `print(string)` and `assert(bool, string)`, plus `replace` and `take` for fields (section 17, V-3) and `same(a, b)` for the identity of objects (section 8) and `fail(text)` for a deliberate program error (section 22).
- [E] Scope of the standard library: first cut, sufficient for a self-translating compiler (section 24).

## 16. Findings from the Grammar Test

The grammar is available as "RPP.g4" and has been tested.

- [E] Lambdas are at the expression level. The hull reaches to the end of the surrounding expression; as an operand or for a direct call, a lambda must be clamped. `(i32 x) => x + 1` Ambiguous.
- [E] Case marks are literals, constant or variant names and variants with bindings (`case Circle(r):`, qualified `case Shape::Circle(r):`), not any expressions. `Circle(r)` At the same time call and pattern.
- [E] Shift operators consist of two tokens so that `List<List<i32>>` is readable. side effect: `a > > b` with space is accepted.
- [E] To the left of `=` is only a name, field, index access or tuple.

Findings from the test, meanwhile decided:

- [E] Keywords in package names are prohibited (see Section 13).
- [E] The copy is called `copy(a)` (see Section 8).
- [E] `List<Foo>::create()` is permitted (see Section 10; originally with `.`, since 20.11.2026 with `::`).
- [E] All keywords remain reserved, including `shared`, `weak`, `own`, `base` and `cast` The only exception is `main`.

## 17. Determinations from the Ownership Core calculus

In the preparation of Chapter 5 (“R++ Ownership Core Calculus”), the following points were raised and decided on 18.11.2026, where the reasons are in Section 2.

- [E] V-1: `shared` Objects are mutable. The exclusivity is checked for them at runtime; a conflict throws `BorrowError`, in both modes.
- [E] V-2: Accesses never go directly through a `shared`- or `weak`-field; the compiler creates a local auxiliary variable for this. `this.next.name()`.
- [E] V-3: Do not move out of fields, but there are two built-in functions: `replace(mut x.f, neu)` sets the new value and delivers the old one; `take(mut x.f)` is only available for nullable fields, sets the field to `null` and delivers the old value.
- [E] V-4 (as amended on 21.11.2026): Methods may return Borrows which are `this` date of issue `ref V get(K k)`, mutable `ref mut V at(K k) mut` with `return mut …;` At the call point, the Borrow is considered a loan to the recipient until the end of the statement or, with `ref` until the end of the block. Borrows to return parameters remains forbidden. Nuclear calculus R-5.23, R-5.24; tested in the FR++ model.
- [E] V-5: A move in only one `if` branch is allowed; after that the variable is considered moved, released by drop flag.
- [E] V-6: `weak` is created only from `shared`; access to `w.lock()` with result `shared T?`.
- [E] V-7: Sharing order: local variables backwards; for objects first destroyer, then fields backwards, base class last.
- [E] V-8: A reading borrow only protects what can be reached via `own` edges; objects reached via `shared` remain changeable (with runtime testing according to V-1).
- [E] V-9: `shared` values are implicitly copied at assignment and handover (counter + 1); from `own` is `shared` only at explicit `shared` target (variable, field or parameter); from `shared` is never `own`, but there is `copy(s)`.
- [E] V-10: In the constructor, all fields must be set before `this` is used other than for setting fields.
- [E] V-11: An exception that leaves a destructor terminates the program.
- [E] Named local borrows (20.11.2026): `ref Foo f = list[0];` borrows reading, `ref mut Foo g = mut x.child;` changeable; `mut` precedes type and initializer as for parameters and calls; the borrow is valid until the end of the surrounding block, as for local slices. The initializer must be a location (variable, field or index access). `ref` Only exists for local variables, not at fields, parameters, `for` header, ownership form, or with `var`. `ref` is a reserved word.

## 18. Determinations from Chapter 4 (Types)

When Chapter 4 (“R++ Types”) was formulated, minor points had to be defined in addition; they were decided on 20.11.2026.

- [E] V-4.0: `i32 + ui32` has the common type `i64` and is not an error (correction of the original example); error only without common type (`i64 + ui64`).
- [E] V-4.1: `char` has comparisons, no arithmetic; `cast<char>(n)` with invalid scalar value raises in both modes.
- [E] V-4.2: Shift count outside the range: Exception in the debug, modulo bit width in the release; `>>` arithmetic for signed types.
- [E] V-4.3: Bit operators for integers only and `byte`; `bool` has only `!`, `&&`, `||`, `==`, `!=`.
- [E] V-4.4: Strings unchangeable; `+` only between strings; `==` by content, `<` lexicographical by scalar values.
- [E] V-4.5: Equality over the generic mixin `Equatable<T>` with `bool equals(T other)`; primitive types, `string` and enums without data are self-evident `Equatable`.
- [E] V-4.6: `as` requires a real subtype; result is a Borrow `C?` or `shared`-target `shared C?`; no downcast with transfer of ownership.
- [E] V-4.7: Enum variants without value: previous value + 1, the first 0.
- [E] V-4.8: Tuples, arrays and function types do not have `==`; Tuple literals take element types from context.
- [E] V-4.9: No automatic conversion between `T[N]` and `T[]`; array size ≥ 0.
- [E] V-4.10: Conversion to floating point rounds to the next value and never throws.
- [E] V-4.11: `null` needs a nullable context (`var x = null;` is an error); no `T??`, no `void?`.

## 19. Determinations from Chapter 6 (null safety)

Decided on 20.11.2026; details in "R++ Null Safety".

- [E] Full flow analysis for the narrowing: right side of `&&` and `||`, `!`, `? :`, `else` branches, `while` and `for` conditions and any branch that does not end normally (`return`, `throw`, `break`, `continue`).
- [E] `assert(c, text)` is active in both modes and then constricts like a fulfilled condition.
- [E] `x == null` and `x != null` with non-nullable `x` are errors; likewise `?.` and `??` on non-nullable values.
- [E] A generic `T?` coincides with `T` when `T` itself is nullable (`T = Foo?` yields `T? = Foo?`).
- [E] A narrowing ends with the next assignment to the variable, after which the type of the new value applies; after `if` no exit, the intersection of both paths applies.
- [E] Nullable fields are fetched for testing: `own` fields with `ref` (or removed with `take`), `shared` fields by copy. Array elements are never narrowed, but also fetched with `ref`.
- [E] `x?.m()` has the type `R?`; for `void` only as statement; for `null`, the rest of the chain is skipped. `a ?? b` is `T` if `b` is not nullable, otherwise `T?`; `b` is evaluated for `null` only.
- [E] No `??=` in version 1.
- [E] `this`, results of `new`, Literals except `null` and parameters without `?` are never `null`.

## 20. Determinations from Chapter 7 (classes, mixins, type parameters)

Decides on 21.11.2026; details in "R++ Classes and Generics".

- [E] Methods and constructors may be overloaded. Variants may differ not only in return type or only in ownership and borrow markings; a static and an instance method may not be the same.
- [E] Resolution by order of precedence per argument: exactly the same type, then literal and number adjustment, then upcast, then `T` → `T?`. The variant is chosen which is nowhere worse and at least once better; non-generic variants beat generic; otherwise ambiguity errors.
- [E] Additional rule (supplemented when formulating, adopted on 21.11.2026): In the case of equal rank, the more special parameter type wins (which can be automatically converted into the other, e.g. `i32` before `i64`, `Circle` before `Shape`). Without it, `f(x)` would be ambiguous with `x : i8` `f(i32)` and `f(i64)`.
- [E] A mixin conflict is solved by the class itself defining the method as `override`; `M::m()` invoking the version of the mixin `M`, `base.m()` that of the base class.
- [E] `copy` is virtual: `copy(a)` copied according to the actual class. If a class `copy`, each subclass must have its own `copy`; the base part is copied with the `copy` of the base class.
- [E] are members before the first visibility label `private`.
- [E] If a class does not have a constructor, there is a parameterless one if each field has an initializer or is nullable.
- [E] `override` requires exactly the same parameter types, ownership modes, `mut` mode and return type; visibility must not become narrower. Overwritten methods remain virtual; no `sealed` in version 1.
- [E] Mixins are allowed to import other mixins. They have no static methods and no constants; all methods are `public`.
- [E] Generic generation via the constraint `new()` as permitted in C#: `class Pool<T : new()>` `new T()`; the type argument requires a parameterless constructor.
- [E] Virtual calls are allowed in the constructor only after complete initialization and then dynamic; in the destructor also dynamic.

## 21. Determinations from Chapter 8 (Statements and expressions)

Decides on 21.11.2026; details in "R++ Statements and Expressions".

- [E] Assignments and compound assignments are only statements (and in the `for` head); `x = y = 0;` and `if ((x = f()) != null)` do not exist. `++` and `--` may be expressed in terms: `i++` delivers the old value, `++i` the new value.
- [E] evaluation from left to right; receiver before arguments; in assignments first the parts of the target, then the right side.
- [E] `x op= e` means `x = x op e` with a single evaluation of `x`; the result must automatically fit into the type of `x`.
- [E] Methods with return type must end with `return` or `throw` on each path.
- [E] `switch` over integers, `char`, `string` and enums; duplicate marks are errors; without `default` only for complete enums.
- [E] `break` directly in a `case` terminates the `switch` (as in C/Java); `continue` always applies the loop.
- [E] Type of `c ? a : b`: common number type or the type of branch to which the other automatically merges; no search for common base class.
- [E] `(a, b) = (b, a);` First evaluates the right side; exchange works.
- [E] `for (x : c)` runs over arrays, vectors, slices, strings and classes with the library mixin `Iterable<T>`.
- [E] An unused result of an appeal is an error. Deliberate rejection: `discard list.pop();` Methods with `nodiscard` cannot be discarded with `discard` either.
- [E] Unreachable code generates a warning; thus, the specification also knows warnings in addition to errors.
- [E] No jump marks (`break outer;`) and no areas on strings in version 1.
- [E] V-8.1: Local variables must not obscure other local variables and parameters; fields already (added when formulating, adopted on 21.11.2026).
- [E] V-8.2: `nodiscard` belongs to the signature and must be adopted when overwriting and fulfilling a requirement; in `void` methods it is an error (added when formulating, adopted on 21.11.2026).

## 22. Chapter 9 (Exceptions)

Decide on 21.11.2026; details in "R++ Exceptions".

- [E] Two types: Exceptions (triggered with `throw` and catchable with `catch`) and program errors (index, overflow, division by 0, invalid `cast`, `assert`, `BorrowError`, exception from a destructor, `fail`). Program errors are not catchable: destructors and `finally` still run, then the program ends.
- [E] Only `own` objects of subclasses of `rpp.core.Exception` (fields `message`, `cause`, `suppressed`) are thrown.
- [E] A `finally` block must not be left with `return`, `break` or `continue`; a direct `throw` in it is an error.
- [E] shall make a call in the `finally` while an exception is already underway, the new one replaces the old one; the old one becomes `suppressed` The new ones are stored.
- [E] The first fitting `catch` catches; a `catch` behind a `catch` of its upper class produces a warning (unreachable).
- [E] The `catch` variable has the exception; `throw e;` it throws on (move).
- [E] When a constructor is thrown, set fields and a finished base portion are released; the class destroyer itself does not run.
- [E] Exit codes: uncaught exception 1, program error 2; message on the error output.
- [E] Stacktrace in debug mode always, left to the implementation in release mode.
- [E] No `using`/try-with-resources; cleaning up destructors.
- [E] New built-in function `fail(text)`: triggers a program error and never returns (counts as `throw` for narrowing, return and accessibility).
- [E] One `try` needs at least one `catch` or one `finally`.

## 23. Determinations from Chapter 10 (Lambdas)

Decide on 21.11.2026; details in "R++ Lambdas".

- [E] Recorded values of a `own` Lambda are immutable; a call never changes the lambda. `shared`-object that remains changeable via the reference (such as V-8).
- [E] In `Func` parameters bear the markings of method parameters: `Func<Foo, …>` Reading, `Func<mut Foo, …>` modifiable, `Func<own Foo, …>` adopted. `mut` is allowed only there.
- [E] A `own` lambda must not be used `this`; you get values into local variables beforehand.
- [E] A `own` lambda without an expected type gets its `Func` type from parameters and hull: `var f = own (i32 x) => x + 1;` is `Func<i32, i32>`.
- [E] A `own` lambda cannot capture a Borrow variable of an object type; write before `copy(x)`.
- [E] A normal lambda borrows a variable mutable when the body changes it, otherwise reading; the borrows are valid for the duration of the call.
- [E] A `own` lambda is detected when the lambda expression is evaluated; `own` values are shifted into it.
- [E] Function values are `own` objects: Assign moves, `shared` is possible, `copy` is not.
- [E] Return type from the print-out body or as a common type of all `return` values; an expected `Func` type is tested.
- [E] Call with `f(args)`, also via fields and parameters; reads only. A `Func` parameter without `own` may be called and passed on as often as desired, but not saved.
- [E] Lambdas do not provide borrows.

## 24. Determinations from Chapter 13 (Standard Library, First Section)

Decided on 21.11.2026; details in "R++ Standard Library".

- [E] The first section covers exactly what a self-translating compiler written in R++ needs, until then the libraries under `rpp` remain on version 0.x and are allowed to change.
- [E] Modules: `rpp.core` (Exceptions, `Equatable`, `Comparable`, `Hashable`, `Iterable`), `rpp.text` (String methods, `StringBuilder`, character classes), `rpp.fmt` (Numbers ↔ Text), `rpp.collections` (Vector methods, `Map`, `Set`, sorting), `rpp.io` (files, directories, paths, console), `rpp.env` (Ambient variables, platform).
- [E] iteration of own classes via a cursor protocol: `Iterable<T, C>` with `begin`, `done`, `next` and `ref T at(C)`. The cursor is an ordinary value; the loop borrows the collection. Iterator objects do not exist because they would need borrows in fields.
- [E] Primitive types and `string` have built-in methods and static members: `n.toString()`, `s.substring(1, 3)`, `i32::parse(s)`, `i64::MAX`.
- [E] `N::parse(s)` Throws one `FormatException`; `N::tryParse(s)` delivers `N?`.
- [E] V-13.1: Several libraries at `rpp`, shipped with the compiler, without entry in `rpp.yaml`; `rpp.core` is visible without `using`.
- [E] V-13.2: Deterministic iteration: `Map` and `Set` in insertion order, `Dir::list` sorted.
- [E] V-13.3: None `exit`; provides the exit code `main`.
- [E] V-13.4: `print(text)` writes with a break of lines; without a break of lines `Console::write`.
- [E] V-13.5: `get(k)` provides `ref V?`; `at(k)` requires an existing key, otherwise program error.
- [E] V-13.6: A Borrow method may also return via `ref` variables, loop elements and other Borrow calls originating from `this` (not yet shown in the FR++ model).
- [E] V-13.7: `<`, `<=`, `>`, `>=` for types with `Comparable` call `compareTo`.

## 24a. Expansion of the standard library and rpp.random

Decide on 23.11.2026; details in “R++ Standard Library”, sections 9 and 10.

- [E] V-13.9: New packages under `rpp` add to the library without changing the first cut; they remain on 0.x until self-translation. When porting from other languages: no `Optional`/`Result` (`T?`, `try…`, Exceptions do this), borrowing before consuming, determinism, same behavior in both modes (no reliance on the overflow behavior of the release mode). New packages are written in R++ itself where possible and serve as a load test for M6.
- [E] The expansion follows a roadmap in three phases (working status): A next to M6 (vector algorithms, `Deque`, `rpp.math`, `rpp.random`, text and IO expansion), B after self-translation (`rpp.iter`, `rpp.regex` according to RE2-Art, `rpp.json`, `rpp.time`, `rpp.encoding`, `rpp.hash` with SHA-256 for `rpp.lock`), C with the threads (`rpp.sync` with channels per `own`, `rpp.process`, `rpp.net`).
- [E] R-13.23: `rpp.random.Random` is a seedable generator with a fixed algorithm: Seeding by SplitMix64, step by `xoshiro256**`; same seed delivers the same sequence on each platform and in both modes. The arithmetic modulo 264 is calculated without overflow.
- [E] R-13.24: `nextUi64()`, `nextInt(bound)` (uniformly distributed without modulo-bias by discard; `bound ≤ 0` is a `FailError`), `nextFloat()` in `[0, 1)` with 53 bits, `nextBool()` as the uppermost bit.
- [E] R-13.25: `shuffle<T>(mut T[] v)` mixes with Fisher-Yates over `remove`/`insert`; an exchange of two indices is not possible (V-3, overlap of unknown indices).
- [E] V-13.10: Vector algorithms that can be written in R++ come up as static generic methods of the auxiliary class `Vectors`, not as built-in methods `T[]` (built-in methods produce a divergence in M6 until redrawing). Built-in methods remain what R++ cannot efficiently express (`sort`). Shiftable algorithms use `remove`/`insert` and are quadratic; this is sufficient for the reference.
- [E] R-13.26: `Vectors.binarySearch` (Comparable, ascending sorted slice, position or `null`) and `Vectors.equals` (Equatable, elemental comparison; fill the gap that arrays do not have `==`).
- [E] R-13.27: `Vectors.reverse`, `retain` (predicate) and `dedup` (consecutive duplicates) modify `mut T[]` via `remove`/`insert`; work for any element type, including `own` objects.
- [E] R-7.26 also enforced for method calls: so far `rppc` Constraints only checked for class instantiation, not generic methods; a call such as `binarySearch` with a type without `Comparable` was accepted and failed only at runtime. When writing to `Vectors` noticed and in `rppc` (`body.rs`) and in the compiler in R++ (`Checker.rpp`) Resolved; new test `n08_vectors_ohne_comparable` The specification ()R-7.26) already demanded the exam.
- [E] V-13.11: `rpp.math` provides `f64` functions. Rounds and `clamp` are pure R++ source text; `sqrt` is built-in (correctly rounded to IEEE 754, sample `fail("eingebaut")` without M6 divergence).
- [E] R-13.28: `Math.PI`/`Math.E`, `sqrt` (correctly rounded, `x < 0` → NaN), `floor`/`ceil`/`trunc`/`round` (exact integer, `round` at .5 away from zero); NaN/±∞/±0 are retained.
- [E] R-13.29: `Math.clamp(x, lo, hi)` Limited to `[lo, hi]`; `lo > hi` is a `FailError`, NaN remains NaN.
- [E] R-13.30: `rpp.text.Text` (pure R++ source text following the pattern of `Fmt`) supplements the string methods with `lines` (CRLF-capable, not a final blank line element), `removePrefix`/`removeSuffix`, `repeat` (`n < 0` → `FailError`) and `splitLimit` (maximum limit parts, last part is the remainder; `limit < 1` or empty sep → `FailError`).
- [E] R-13.31: `File` receives `appendText`, `copyFile`, `moveFile`, `delete`, `size` (built-in, file system access; `IOException` in case of errors). `copy` is a keyword, therefore `copyFile`/`moveFile`. `copyFile`/`moveFile` replace an existing target. Mtime and buffered reading follow later (Mtime needs `rpp.time`).
- [E] R-13.32: `Deque<T>` (pure R++ source text, vector-based, `Iterable<T, i64>`) with `pushFront`/`pushBack`, `popFront`/`popBack` (empty → `IndexError`), `front`/`back` (`ref T?`), `length`/`isEmpty`/`clear`. Operations at the beginning are linear; a ring buffer would be a later built-in variant.
- [E] R-13.33: `Map.getOrPut(own K key, Func<V> factory)` delivers `ref mut V`; if the key is missing, it is inserted `factory()` (only called then). Built-in so that `own` keys can also be used (a pure R++ version would have to reuse the moved key after `put`).

- [E] V-13.37 (adopted on 26.11.2026): `Math.pow`, `Math.exp`, `Math.log`, `Math.sin`, `Math.cos`, and `Math.tan` as incorporated (`fail("eingebaut")`, Dispatch in `stdlib.rs`); results platform-dependent, not bit-equal; within a platform deterministic, both modes equal.
- [E] V-13.38 (adopted on 26.11.2026): `Vectors.count<T : Equatable<T>>(Slice<T> s, T x)` as a static generic method of auxiliary class `Vectors`; counts the occurrences of `x`, yielding empty slice `0`.
- [E] V-13.39 (adopted 26.11.2026): `Vectors.any<T>(Slice<T> s, Func<T, bool> predicate)`, `Vectors.all<T>(…)` and `Vectors.none<T>(…)` as static generic methods; predicates on the slice, empty slice: `any` → `false`, `all` → `true`, `none` → `true`.
- [E] V-13.40 (adopted on 26.11.2026): `rpp.text.Text` adds `isBlank` (about `trim`), `substringBefore`/`substringAfter` (about `splitLimit` with `limit = 2`) and `capitalize`, `toUpperCase` and `toLowerCase` (about `substring` and the Builtins `toUpper`/`toLower`); pure R++ source text, V-13.10, sample `Fmt`.
- [E] V-13.41 (adopted on 26.11.2026): `rpp.math.Math` supplemented by the transcendents `log10`, `hypot`, `atan2`, `sinh`, `cosh` and `tanh` (built-in, dispatch in `stdlib.rs`, platform-dependent, not bit-like, V-13.11) and the conversions `toDegrees`/`toRadians` (pure R++ source text, arithmetic about `Math::PI`).
- [E] V-13.42 (adopted on 26.11.2026): `rpp.iter.Iter` (pure R++ source text, V-13.10, pattern of `Vectors`) supplemented `rpp.iter` with the lazy operations `map`, `filter`, `take`, `flatMap` and `chain` over `Slice<shared T>` with result `shared T[]` (elements divided instead of shifted, R-5.3); Porte: Rust `Iterator`, Kotlin Sequence, C++20 ranges.

## 25. Determinations from Chapters 1, 11, 12 and 14

Decide on 21/11/2026; details in “R++ Introduction”, “R++ Packages and Projects”, “R++ Execution Model” and “R++ Conformance Tests”.

- [E] `using` binds individual types (`using a.b.C;`) or under other names (`using X = a.b.C;`); no wildcards.
- [E] A file can contain any number of types, its name is free; only the directory must fit the package. Files directly in the source folder belong to the root packet without `package` line.
- [E] Packages of a project may use each other; the dependency graph between projects is cycle-free.
- [E] Name resolution: local names → Members → Type parameters → own package → `using` → `rpp.core` Two of the same names `using` They are a mistake when used.
- [E] Stack overflow and memory deficiency are program errors (exit code 2).
- [E] floating point bitaccurate according to IEEE 754 in both modes, without FMA summary and advanced accuracy.
- [E] Version 1 is simple-threaded; the thread rules remain normative, a thread library comes later.
- [E] The test suite has runtime tests with expected output and expected exit code per mode in addition to accepted and rejected programs.
- [E] The test run checks which rules are not mentioned in any test head and reports the gaps.
- [E] An implementation is compliant if it passes all tests of the suite.
- [E] V-12.1: `args` contains the arguments without the program name (adopted on 21.11.2026).
- [E] V-12.2: At least 10,000 nested method calls are guaranteed (adopted on 21.11.2026).
- [E] floating pointiterals may have an exponent, but only after digits with point: `1.0e10`, `2.5E-3`; `1e10` a syntax error remains so that each floating pointiteral is recognizable at its point (decided on 21.11.2026).

## 26. Rule coverage of conformance tests

On 21.11.2026 when closing the gaps in rule coverage; details in "R++ Conformance Tests".

- [E] Tests that require multiple files or projects are directories with `rpp.yaml` (project tests); in YAML `# FEHLER` marks the error line.
- [E] A rejected test may fail due to a syntax error or directive if the first error is in the marked line.
- [E] Rules that cannot be tested with a test program are listed in Chapter 14 with justification and do not count as a gap.
- [E] After a directive, there may be a line commentary (`#if debug // …`).
- [E] V-4.12 (assumed on 21.11.2026): Enum values are produced without `new`: `Color::Red`, `Shape::Circle(2.0)`.
- [E] V-9.1 (adopted on 21.11.2026): Each type of program error has a fixed name for the message (`IndexError`, `OverflowError`, `DivisionError`, `CastError`, `AssertError`, `BorrowError`, `DestructorError`, `FailError`, `StackOverflowError`, `OutOfMemoryError`). Runtime tests test these names; so far, only three of them were mentioned as examples.
- [E] V-14.1 (assumed on 21.11.2026): Runtime tests can set command line arguments and standard input (`// Argumente: …`, `// Eingabe: "…"`).

## 27. First implementation

Decided on 21.11.2026; details in "R++ Implementation Plan".

- [E] The first implementation (frontend, debug interpreter as reference, later JIT and release mode) is written in Rust. It remains the bootstrap compiler until R++ translates itself.
- [E] Construction and milestones according to “R++ Implementation Plan” (adopted on 21.11.2026 with the start of M1): Cargo-Workspace `rppc/`, Parser by hand (recursive descent and precedence parser), joint interim representation, interpreter with test mode for the invariants from chapter 5, milestones M1 to M6.
- [E] The negative grammar tests and readings are files in `rpp-grammar/tests/neg` and `tests/parse_probes`; the ANTLR test run and the Rust parser read the same files.
- [E] Milestone M1 is completed: The Rust parser recognizes the same language as `RPP.g4` tested with all grammar and conformance tests and a differential test on mutated programs.
- [E] Milestone M2 is completed: Projects, name resolution and type testing; pass all conformance tests except those for M3.
- [E] V-4.13 (adopted on 21.11.2026): Clarification on R-4.12 that for two integers only integer types can be of common type (otherwise would have been `i32`, `ui32` none).
- [E] V-8.3 (adopted on 21.11.2026): A class method meeting a `nodiscard` requirement need not be repeated `nodiscard`; its result remains `nodiscard`.
- [E] Milestone M3, first part: flow analysis for narrowing, safe initialization, moves and borrows, and warnings; pass all 242 conformance tests.
- [E] V-12 (Chapter 5, adopted on 21.11.2026): A branch that does not end normally contributes nothing to the condition after `if`, `switch`, loop or `try`, as in the case of narrowing (R-6.8).
- [E] V-6.1 (Chapter 6, adopted on 21.11.2026): For R-6.3, R-6.16 and R-6.18 variables count the declared type; a narrowing does not make `v?.m()`, `v ?? w` and `v == null` an error.
- [E] V-14.2 (Chapter 14, adopted on 21.11.2026): A message may state reasons besides the violated rule; a rejected test is satisfied if one of them is the expected one.
- [E] Milestone M3 is completed: Flow analysis in `rppc`, pass all conformance tests, and `rppc` assessed translated FR++ programs as the model (5000 random programs without deviation).
- [E] V-13 (Chapter 5, adopted on 21.11.2026): A `own` value at a `weak` target is an error (R-4.41); `weak` arises only from `shared` or `weak`.
- [E] Milestone M4 has begun: debug interpreters `rpp-interp`, `rppc run`; all runtime tests except two that require built-in parts of the library (M5) pass in both modes.
- [E] V-14 (Chapter 5, adopted on 22.11.2026): Reading of V-7 in case of inheritance – first all destructors (class to base), then all fields in reverse order; also when cleaning up for a throwing constructor.
- [E] Implementation plan (accepted on 22.11.2026): The interim representation comes only with JIT or release mode; until then, the interpreter executes the tested AST.
- [E] Milestone M4 is completed: Debug interpreter with test mode for the invariants from Chapter 5; all runtime tests with `print` and `rpp.core` pass in both modes; execution the same as in the FR++ model except for one explained case.
- [E] Milestone M5, first part: built-in parts of the standard library in the interpreter; all 245 conformance tests pass in both modes.
- [E] V-15 (Chapter 5, adopted on 22.11.2026): `own`-Object of a class without a destructor can be `(K k, V v) = e;` cut down into its fields (separated)R-5.25); Moves from individual fields remain prohibited (V-3).
- [E] Milestone M5 is completed: built-in standard library, type arguments at runtime, hash index; all 255 conformance tests pass in both modes.
- [E] V-14.3 (Chapter 14, adopted on 22.11.2026): Runtime tests run in a fresh, empty working directory that they may describe.
- [E] Milestone M6 has begun: The compiler in R++ is created in `rpp-compiler/`, step by step and compared against `rppc`; the first part is the Lexer.
- [E] M6: The parser in R++ builds a uniform syntax tree (type, text, position, children) instead of one class per node type; it is compared via a common text form.
- [E] M6: The loading of projects is transferred to R++ and equivalent to `rppc`; incorrect forms in `rpp.yaml` (no list, no text) are errors after R-11.13.
- [E] V-13.8 (Chapter 13, adopted on 22.11.2026): `File::exists(path)` and `Path::absolute(p)` in `rpp.io`.
- [E] M6: The declaration phase is transferred to R++; types are there immutable, divided values (`shared Ty`).
- [E] M6: The test of the hulls is carried over to R++; the inspector owns the program and borrows the syntax trees.
- [E] Keyword `mut` (adopted on 22.11.2026): variability is marked `mut` as in Rust instead of `&`, which was at the same time bitwise and constraints. `mut` It precedes the type (`mut Foo f`, `Func<mut Foo, void>`, `ref mut Foo g`, `ref mut V at(K k)`), before the expression (`rename(mut foo)`, `= mut x`, `return mut …;`, `for (mut Foo f : mut list)`) and after the parameter list of modifying methods (`void clear() mut`). `mut` is a reserved word. `&` means only bitwise And and connects constraints (`T : A & B`).
- [E] Milestone M7 (adopted on 24.11.2026): The speed of the translated code is created by removing the C code generator from M6; it becomes the release mode after R-12.1, instead of its own intermediate representation or LLVM. Each step keeps the fixed point of self-translation and all comparisons with `rppc`.
