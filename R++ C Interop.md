# R++ C Interop (Chapter 15)

[Deutsch](R%2B%2B%20C%20Interop.de.md)

Status: 27.11.2026, draft 0.1. This document fills Chapter 15 of the "R++ Specification Outline". It specifies how an R++ program calls functions of a C library: the declaration with `extern`, which types cross the boundary and how, handles for objects managed by C, loading the library, and the safety boundary that determines which projects may bring C code and which guarantees still hold then.

The fundamentals are decided (see "R++ Design Decisions", Section 12, milestone M10 in the Implementation Plan). The proposals in Section 0 arose while writing out the rules and were accepted on 27.11.2026; the rules that depend on them name them. Every normative rule carries a number `R-15.n`. The conformance tests are in `conformance/ch15_c_interop/`.

## 0. Amendments from writing out the rules (adopted on 27.11.2026)

| No. | Proposal |
|---|---|
| V-15.1 | A handle type is written as a type declaration of its own next to the `extern` class: `extern("demo") handle Counter(demo_counter_free);`. `handle` is a keyword only directly after `extern(…)` and remains an ordinary name elsewhere. |
| V-15.2 | The name of a method in an `extern` class is also the name of the C symbol; there is no renaming. Methods of an `extern` class therefore cannot be overloaded. |
| V-15.3 | A parameter `byte[]` or `mut byte[]` becomes **two** parameters in C: a pointer to the first element and the length as `size_t`. |
| V-15.4 | A `string` returned by C still belongs to C: R++ copies it immediately and never frees the pointer. Strings the caller must free are modeled with handles. |
| V-15.5 | All C libraries of a program are loaded and all symbols resolved before `main`. If anything is missing, the program does not run: release mode reports it when building, debug mode at start as a program error (exit code 2). This becomes a new permitted difference in R-12.5. |
| V-15.6 | Before every C call, R++'s standard output and standard error are flushed, so that output from R++ and C appears in the order in which it is produced (R-12.4). |
| V-15.7 | New entry `nativePaths` in `rpp.yaml`: folders relative to the project directory that are searched first for C libraries when building and loading. |
| V-15.8 | A library "brings C code" if it is `native: true` or depends on a library that brings C code. Every edge to such a library needs `allowNative: true`. The program thus decides, through its direct dependencies, on all C code below them, and every library in between has allowed its own C dependencies. |
| V-15.9 | New kind of program error `NativeError` (R-9.2) for errors at the boundary: an argument that cannot be converted to C, a result that cannot be converted to R++, a missing library or a missing symbol at start. |

## 1. Declaration

**R-15.1 (extern class).** An `extern` class declares functions of a C library:

```
externClassDecl : 'export'? 'extern' '(' STRING ')' 'class' IDENT '{' externMember* '}' ;
externMember    : accessLabel | externMethod ;
externMethod    : 'static' returnType IDENT '(' paramList? ')' ';' ;
```

The string in parentheses is the name of the library (R-15.4). Like every type declaration, the class lives in a package and is used like a class with static methods: `Zlib::crc32(0, data)`. `extern` is a reserved word.

**R-15.2 (Content).** An `extern` class contains only static methods without a body. Fields, constructors, a destructor, `copy`, instance methods, methods with a body, constants, base classes, mixin imports and type parameters are errors. An `extern` class cannot be instantiated, extended or used as the type of a variable. Visibility labels and `export` apply as for other classes (R-7.2, Chapter 11); methods before the first label are therefore `private`.

**R-15.3 (Symbol names).** The name of a method is the name of the C symbol (V-15.2). Two methods with the same name in one `extern` class are an error. If two `extern` classes of a program declare the same symbol of the same library, both declarations must have the same signature; otherwise the second one is an error.

**R-15.4 (Library name).** The name consists of letters, digits, `_`, `-` and `.`, is not empty and contains no path separators. Per platform it becomes a file name: `lib<name>.so` on Linux, `lib<name>.dylib` on macOS, `<name>.dll` on Windows (`<name>.lib` for linking). The folders from `nativePaths` (V-15.7) are searched first, in their order, then the places the system provides for libraries.

**R-15.5 (Handle type).** A handle type stands for an object managed by C (V-15.1):

```
externHandleDecl : 'export'? 'extern' '(' STRING ')' 'handle' IDENT '(' IDENT ')' ';' ;
```

The name in parentheses is a C function of the same library with the signature `void f(void*)` that releases the object. In C a handle is an opaque pointer (`void*`). A handle type has no members; R++ code can only hold, borrow, pass and release it.

```
extern("demo") handle Counter(demo_counter_free);

extern("demo") class Demo {
public:
    static own Counter? demo_counter_new(i64 start);
    static i64 demo_counter_next(mut Counter c);
}
```

## 2. Types across the boundary

**R-15.6 (Permitted types).** Parameters and return types of methods of an `extern` class map to C as follows:

| R++ | as parameter | as return type |
|---|---|---|
| `i8`, `i16`, `i32`, `i64` | `int8_t` … `int64_t` | same |
| `ui8`, `ui16`, `ui32`, `ui64` | `uint8_t` … `uint64_t` | same |
| `f32`, `f64` | `float`, `double` | same |
| `bool` | `bool` (C99) | same |
| `byte` | `uint8_t` | same |
| `string` | `const char*`, copied (R-15.7) | `const char*`, copied (R-15.8) |
| `string?` | – | like `string`; `NULL` becomes `null` |
| `byte[]` | `const uint8_t*`, `size_t` (R-15.9) | – |
| `mut byte[]` | `uint8_t*`, `size_t` (R-15.9) | – |
| handle `H` (borrowed) | `void*` (R-15.10) | – |
| `mut H` | `void*` (R-15.10) | – |
| `own H` | `void*`, ownership passes to C (R-15.10) | `void*`, ownership passes to R++ |
| `own H?` | – | `void*`; `NULL` becomes `null` |
| `void` | – | `void` |

Other types are an error at the declaration, among them `char`, classes, enums, tuples, function types, arrays of other element types, nullable primitive values, `shared` and `weak`.

**R-15.7 (string as parameter).** R++ passes a copy of the string in UTF-8 with a terminating null byte. The copy is valid only for the duration of the call and is released afterwards. If the string contains the character U+0000, that is a program error (`NativeError`, V-15.9) before C is called.

**R-15.8 (string as return value).** R++ reads the returned pointer up to the first null byte and copies the content into a new string (V-15.4). Invalid UTF-8 is a program error (`NativeError`). `NULL` yields `null` for `string?` and is a program error (`NativeError`) for `string`.

**R-15.9 (byte[]).** A parameter `byte[]` is borrowed for reading for the duration of the call, `mut byte[]` mutably (Chapter 5); C receives a pointer and the number of elements (V-15.3). Through `mut byte[]`, C may change the elements but not the length. With length 0 the pointer may be anything, including `NULL`.

**R-15.10 (Handles).** A handle type exists only as an `own` value; `shared H`, `weak H` and `copy` of a handle are errors. A parameter `H` borrows the handle for reading, `mut H` mutably, both only for the duration of the call. A parameter `own H` moves the handle to C: R++ no longer releases it. A return value `own H` belongs to R++; `NULL` is a program error (`NativeError`) there, and becomes `null` for `own H?`. When R++ releases a handle (Chapter 5, R-12.11), it calls the release function from R-15.5 exactly once.

## 3. Calls

**R-15.11 (Sequence).** A call of a method of an `extern` class proceeds as follows:
1. The arguments are evaluated as for every call (R-8.2).
2. They are converted according to R-15.6 to R-15.10; if that fails, the program ends with `NativeError` and C is not called.
3. Standard output and standard error are flushed (V-15.6).
4. The C function is called.
5. The result is converted (R-15.8, R-15.10); then all borrows of the call end, and the copies from R-15.7 are released.

A C function throws no exceptions; it reports errors through its return value, which R++ code checks and translates into an exception where needed.

**R-15.12 (Loading).** All libraries named by `extern` classes and handle types of a program are loaded before `main`, and every declared symbol is resolved (V-15.5), even if the program never calls it. If a library or symbol is missing, release mode reports it when building; debug mode ends the program before `main` with `NativeError` and exit code 2.

## 4. Safety boundary

**R-15.13 (native).** Only a project with `native: true` in its `rpp.yaml` may declare `extern` classes and handle types; otherwise every such declaration is an error. This applies to programs and libraries alike.

**R-15.14 (Permission).** A library brings C code if it is `native: true` or depends on a library that brings C code (V-15.8). Every dependency on such a library needs `allowNative: true` in the `rpp.yaml` of the depending project, local and from the registry alike:

```yaml
dependencies:
  - name: acme.zlib
    version: "1.2"
    allowNative: true
```

A missing permission is an error at this requirement, naming the library that contains the C code. `allowNative: true` on a dependency without C code is a warning.

**R-15.15 (Visibility).** The output of `project` shows for every library whether it brings C code. In `rpp.lock` (R-11.30), every entry of a library with `native: true` carries the line `native: true`; since the checksum covers `rpp.yaml` (R-11.29), a marking added later is detected as well.

**R-15.16 (Guarantees).** The checks of Chapters 4 to 6 apply unchanged to the R++ code of a program, including at calls of `extern` methods. R-12.6 (no undefined behavior) holds for a program with C code only as long as every called C function keeps to its declaration:
- it has the signature that R-15.6 derives from the declaration;
- it reads and writes only memory it received, and only within the bounds of R-15.9;
- it keeps no pointer to passed strings or arrays beyond the call;
- it frees no memory owned by R++, and no borrowed handle;
- it calls no R++ code and starts no threads that touch R++ values after the call.

If C code violates one of these conditions, the specification does not define the behavior of the program. Checking these conditions is not the compiler's task; that is why every place where they must hold has to be explicitly permitted according to R-15.13 and R-15.14.

## 5. Building and running

**R-15.17 (Both modes).** C calls behave the same in debug and release mode (R-12.5 with the addition from V-15.5). How an implementation connects the library is up to it; the first implementation loads it at run time in the `rppc` interpreter and calls via `libffi`, while `rppcc` emits prototypes and direct calls and links the library when building. How the built program finds the library at run time (search path, copy next to the program) is up to the implementation, as long as R-15.4 holds.

## 6. Example

`rpp.yaml` of a library that binds zlib:

```yaml
name: acme.zlib
version: 1.0.0
type: library
sources: [src]
native: true
```

`src/acme/zlib/Zlib.rpp`:

```
package acme.zlib;

extern("z") class ZlibC {
public:
    static ui32 crc32(ui32 crc, byte[] data);
}

export class Crc32 {
public:
    static ui32 of(byte[] data) {
        return ZlibC::crc32(0, data);
    }
}
```

A program that uses it gives permission (R-15.14):

```yaml
name: app
version: 0.1.0
type: executable
main: src/main.rpp
sources: [src]
dependencies:
  - name: acme.zlib
    version: "1.0"
    allowNative: true
```

## 7. Changes to other chapters

With this chapter, the following places have changed (entered on 27.11.2026). `RPP.g4` and the parsers of `rppc` and `rppcc` followed with the implementation in M10 (27.11.2026).

| Place | Change |
|---|---|
| Outline, Chapter 2 | `extern` in the list of reserved words |
| `RPP.g4` | `externClassDecl` and `externHandleDecl` as type declarations |
| R-9.2 | new kind `NativeError` (V-15.9) |
| R-11.13 | entries `native` and `nativePaths` (V-15.7) |
| R-11.15 | `allowNative` on dependencies |
| R-11.30 | line `native: true` in `rpp.lock` |
| R-12.5 | missing library: when building or at start (V-15.5) |
| R-12.6 | reference to the boundary from R-15.16 |

## 8. Conformance tests

The test programs are in `conformance/ch15_c_interop/`. The tests bring a small C library `native/rpptest.c`, which the test run compiles with the platform's C compiler before the tests; the test projects name its folder in `nativePaths`.

| Rules | Accepted or runtime | Rejected |
|---|---|---|
| R-15.1, R-15.2 | `l01_call_integers` | `n01_extern_with_body`, `n02_extern_field`, `n03_extern_instance` |
| R-15.3 | – | `n04_overloaded_symbol`, `n05_conflicting_signatures` |
| R-15.4 | – | `n06_library_name_with_path` |
| R-15.5, R-15.10 | `l02_handle_release`, `l03_handle_to_c` | `n07_shared_handle`, `n08_handle_copy` |
| R-15.6 | `l04_floats_and_bool` | `n09_class_parameter`, `n10_char_parameter` |
| R-15.7, R-15.8 | `l05_strings`, `l06_null_string` (runtime: `NativeError`) | – |
| R-15.9 | `l07_byte_arrays` | `n11_byte_array_return` |
| R-15.11 | `l08_output_order` | – |
| R-15.12 | `l09_missing_symbol` (runtime: exit code 2 in debug mode, build error in release mode) | – |
| R-15.13 | – | `n12_extern_without_native` |
| R-15.14, R-15.15 | `l10_allowed_dependency` | `n13_not_allowed`, `n14_not_allowed_indirect` |

R-15.16 and R-15.17 cannot be checked by individual programs; they are covered by comparing both modes in all runtime tests of this chapter.
