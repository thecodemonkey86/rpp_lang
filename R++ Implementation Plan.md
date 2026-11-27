# R++ Implementation Plan

[Deutsch](R%2B%2B%20Implementation%20Plan.de.md)

Status: 26.11.2026, Draft 0.17. This document describes the first implementation of R++: host language, structure, testing strategy and milestones. It is not part of the specification; chapters 1 to 14 and grammar `RPP.g4` remain relevant.

Host language, structure and milestones have been decided (Design Decisions, Section 27). Milestones M1 to M9 have been completed (Section 6); M7, the speed of the translated code via the expansion of the C code generator, reached its target value on 26.11.2026. M8 made `rppcc` the independent compiler for both modes (decided, implemented and completed with the first green CI run on 26.11.2026). M9 brings dependencies with versions via a local directory (decided and implemented locally on 26.11.2026, completed with the green CI run on 27.11.2026). M10 brings C interop (decided on 27.11.2026).

## 1. Role of the first implementation

The first implementation has three tasks:

1. **Reference.** Its debug interpreter is the reference implementation (R-12.3). legibility and proximity to the specification precede speed.
2. **Conformance testing.** Only with her will the 242 conformance tests be tested semantically, not only syntactically.
3. **Bootstrap.** It translates the later R++ compiler until it translates itself, so the debug mode must be fast enough to run a compiler.

## 2. Language: Rust

| Point of view | Why Rust |
|---|---|
| Security requirement | A compiler for a memory-secure language should itself be memory-secure. |
| Compiler construction | `enum` and `match` fit to AST, types and intermediate representation; exhaustion testing finds forgotten cases. |
| Numbers | `wrapping_*` `checked_*` and `saturating_*` map overflow and `cast` of both modes directly (R-4.9, R-4.10, R-4.14). `f32`/`f64` are IEEE 754, Rust does not combine to FMA without explicit statements (R-12.7). |
| Text text | `char` is a Unicode scalar value, `String` is UTF-8, as in R++ (R-4.4, R-4.18). |
| JIT and Release | Cranelift for a later JIT in debug mode, LLVM (about `inkwell`) for release mode, both usable from Rust. |
| Duration | No garbage collector competing with the ownership model of R++. |

Rust does not have an official ANTLR target, so the parser is written by hand (section 3.1). `RPP.g4` remains normative; the Python test run continues to check the grammar itself.

Rust 1.83 is installed. For work, this is enough; a newer stable version can follow later with `rustup update`.

## 3. Construction

A cargo workspace in the directory `rppc/`:

| Crate | Task | Chapter |
|---|---|---|
| `rpp-syntax` | Preprocessor, Lexer, Parser, AST with source positions | 2, 3, 11 (Directives) |
| `rpp-project` | `rpp.yaml` `rpp.lock`, Packages, Dependencies, Name Resolution | 11 |
| `rpp-sema` | Type checking, overloading, generics, lambdas, zero flow analysis, ownership and borrows, warnings | 4–8, 10 |
| `rpp-ir` | Joint interim representation of both modes (R-12.2) | 5, 9, 12 |
| `rpp-interp` | Debug interpreter over the intermediate representation | 9, 12 |
| `rpp-std` | Built-in parts of the standard library (output, files, environment); the rest is R++ source text | 13 |
| `rppc` | Command line: `rppc check`, `rppc run`, `rppc test` | – |

Coming later `rpp-jit` (Cranelift) and `rpp-llvm` Stranger crates only where they save a lot of work; the restricted YAML part R-11.13 Reading a little parser of your own.

### 3.1 Parsers

Recursive descent for declarations and statements, precedence parser (pratt) for expressions according to the table in scaffold 3.2. The verified readings from scaffold 3.3 (`a < b > c;` is a declaration, `(a).run();` an expression, etc.) are decided by limited foresight with reset, as ANTLR does.

The test files in `rpp-grammar/tests/pos`, `tests/neg` and `tests/parse_probes` apply to both parsers; both test runs read the same files; where multiple alternatives fail, the parser reports the error that lies furthest in the source code.

### 3.2 Semantic testing

The tests run on a resolution version of the AST (name bound, types known). The ownership test follows the core calculus from Chapter 5: Availability environments, loan sets, junction merging and loop fixtures, as `fr-model/frpp/checker.py`. The zero test (chapter 6) shares the flow analysis with it.

Each error calls file, line and rule number (R-1.5, R-14.7).

### 3.3 Intermediate representation and interpreter

Changed on 22.11.2026 (section 6, M4): The debug interpreter first executes the tested AST with the details of the test (`Program::rt`); the intermediate representation is created with the JIT or the release mode and is tested against this interpreter.

The interim representation is a control flow graph with explicit statements for everything that specifies the specification in terms of runtime behavior: release and drop flags (V-5, V-7), locks for `shared` objects (V-1), unwinding and `finally` (chapter 9), checks for index, overflow and `cast` specifying in which mode they apply (R-12.5).

The interpreter executes these statements directly. In a **check mode**, he additionally checks the invariants I-1 to I-5 from chapter 5, such as the FR++ model, so that an error in the ownership check becomes visible as an internal error instead of a wrong behavior.

## 4. Audit strategy

| Test test | Whatwith |
|---|---|
| Parser equivalent to grammar | positive and negative grammar tests, readings (section 3.1) |
| Conformance | `rppc test conformance/` Reads the headers R-14.3 to R-14.5: errors with rule and line, warnings, output, exit code, error output, arguments and input |
| Ownership | the FR++ tests and the random programs from `fr-model/fuzz.py`, translates to R++; both examiners must judge equally |
| Duration | Interpreter test mode for all tests |

## 5. Milestones

| No | Content | Finished if: |
|---|---|---|
| M1 | Preprocessor, Lexer, Parser, AST | All grammar tests and conformance tests are read as by the ANTLR parser |
| M2 | Projects and packages, name resolution, type verification without ownership and zero verification | the rejected tests of chapters 4, 7, 8, 10, 11 and 13 are rejected with the correct rule, all accepted translated |
| M3 | Zero flow analysis, ownership and borrows, alerts | also the rejected tests of chapters 5, 6 and 9; FR++ tests and random programs match |
| M4 | Intermediate representation, interpreters, exceptions and program errors | pass all run-time tests with `print` and `rpp.core` |
| M5 | Standard library for the compiler: `rpp.text`, `rpp.collections`, `rpp.io`, `rpp.env`, `rpp.fmt` | All conformance tests pass in debug mode |
| M6 | Start of the R++ compiler in R++ | the R++ compiler translates itself (fixed point) and agrees with `rppc` in all comparisons (set on 23.11.2026) |
| M7 | Speed of the translated code: extension of the C code generator (decided on 24.11.2026) | translated `rppcc` at least as fast as `rppc` at `diagnose rpp-compiler` (set on 26.11.2026); reached on 26.11.2026 (0.40 s against 0.47 s) |
| M8 | `rppcc` as a stand-alone compiler: `build`, `run` and `test`, release mode via the C code generator, bootstrap without rust (decided on 26.11.2026) | `rppcc test ..` Passes all conformance tests in debug and release mode (R-14.7); the bootstrap from the checked-in C starting point reaches the fixed point without rust, under Windows (MSVC) and Linux (gcc) in the CI; all comparisons with `rppc` remain without deviation. Fulfilled on 26.11.2026: local (Windows with MSVC, Linux with gcc in WSL) and in the first CI run of the job `self-hosted` (Ubuntu 24.04 and Windows 2025) |
| M9 | Dependencies with versions according to Design Decisions 13b via a local directory in the file system: `rpp.hash` (SHA-256), Minimal Version Selection, `rpp.lock`, `rppcc publish` (decided on 26.11.2026, met locally on 26.11.2026) | new conformance tests in Chapter 11 cover the rules of 13b and pass with `rppc test` and `rppcc test`; `rpp.lock` is reproducible and detects any change to a locked library; `compare.py` shows no deviation for resolution, the fixed point holds. Met on 27.11.2026: locally (Windows with MSVC, Linux with gcc in WSL) and in the CI run of the job `self-hosted` (Ubuntu 24.04 and Windows 2025) |
| M10 | C interop according to Design Decisions 12: `extern` classes, values and handles across the boundary, permission per project (`native`, `allowNative`), execution in `rppc` (loaded dynamically) and `rppcc` (linked directly) (decided on 27.11.2026) | a chapter of its own in the specification with rules and safety model; new conformance tests with a C test library in the repository pass with `rppc test` and `rppcc test` in both modes; `compare.py` shows no deviation, the fixed point holds; green CI run on Ubuntu and Windows |

The release mode is created from the C code generator (decided on 24.11.2026); since M8 checks `rppcc test` the Conformance also for it (R-14.7).

## 6. Status

**M1 completed (21.11.2026).** `rppc/crates/rpp-syntax` contains preprocessor, Lexer, AST and parser, without foreign crates.

| Test test | Result |
|---|---|
| `cargo run -- syntaxtest ..` | 18 positive files (22 sets of symbols), 127 negative tests, 80 readings, 242 conformance tests: 0 errors |
| `cargo test` | Unit tests for preprocessor, lexer and operator priority |
| `rpp-grammar/tools/difftest.py` | 9 600 mutated programs (seeds 2 to 4): no difference to the ANTLR parser in acceptance or rejection; first error line is the same in all rejected cases |

**M2 completed (21.11.2026).** New are `rppc/crates/rpp-sema` and the standard library as source text in `rppc/std/rpp/…`.The library contains the signatures from Chapter 13; its hulls are placeholders until M4/M5.

| Particle | Content |
|---|---|
| `project.rs`, `yaml.rs` | `rpp.yaml` Source folders, local dependencies, cycles, library names (R-11.2, R-11.13, R-11.15 to R-11.18) |
| `defs.rs` | Collect types, resolve names (R-11.6 to R-11.12), `using`, `main`, `export`, member tables, class and mixin rules (chapter 7), enums (R-4.25, R-4.26), constraints (R-7.25, R-7.26) |
| `body.rs` | Statements and expressions: Number rules and literals (chapter 4), overload (R-7.17), generic methods (R-4.44), lambdas (chapter 10), built-in methods (chapter 13), return (R-8.26), switch (R-8.24) |

`rppc test ..` (Conformance tests):

| Result | Number |
|---|---|
| Passed: all accepted and runtime tests translate, the rejected of chapters 4, 7, 8, 10, 11 and 13 are rejected in the marked line with the expected rule | 192 |
| Failed | 0 |
| open to M3: rejected tests on ownership and borrows (chapter 5), type narrowing (chapter 6), safe initialization (R-4.46) and the ownership rules for lambdas and exceptions | 50 |

Until M3, the test treats nullable values as non-zero (no narrowing) and does not check ownership modes.

When writing M2, errors in tests were noticed and corrected:
- two tests used the class `Error` instead of `Exception`;
- one test used the non-existent type `List`;
- two tests from Chapter 7 defined their own mixins `Equatable` and `Comparable` instead of those from `rpp.core`;
- in three Chapter 6 tests, the field `Node?[] slots` remained unsettled, and a Chapter 5 test called `copy` for a class without `copy`;
- in n16 (chapter 4), a local variable concealed a parameter;
- the `using` tests n01 and n02 (chapter 11) referred to non-existent packages and are now project tests with these packages;
- n19 (chapter 7) tested the same as n28 and now tests a mixin constraint;
- the order of the rules in three test heads now matches the reported rule.

**M3, first part completed (21.11.2026).** The hull test (`body.rs`) now carries a flow state with (`flow.rs`) There is no separate pass; the condition is carried while the types are tested.

| Particle | Content |
|---|---|
| Flow status | per digit: narrowed variables (R-6.5), possibly unassigned variables (R-4.46), possibly shifted variables (R-5.3), plus the rule that causes a loss, copied to branches, then combined; paths that do not end normally contribute nothing (V-12) |
| Loops | Fixed point on the loop head (R-6.9, R-5.16): lead without messages until the stand is no longer changed, then a run with messages; `break` and `continue` bring their messages with them |
| try | `catch` and `finally` see the union of all the estates of the `try` bloc (R-6.12, R-9.17) |
| Loans | Places (variable with field path, `[]` for elements) and Loans `(κ, p)` as in Chapter 5, Section 3.2, with duration call (recipients and arguments, R-5.9, R-5.10), statement (slice, borrow return, R-5.24) or block (`ref`, loop, local slice, R-5.17); shared roots always overlap |
| Moves | Use as a value for an owner (section 7 of chapter 5): initializer, assignment, `return`, `own`-argument, `throw`, tuple and array elements, enum data, `own`-lambda; never moved from fields, elements and borrows (V-3) |
| Lambdas | Acquisitions are collected; a normal lambda lends them at the point where it is transferred (R-10.7), a `own` lambda shifts `own` values within itself (R-10.8) and must not capture any borrows (R-10.9) or change what is recorded (R-10.11). |
| null safety | Facts `W(c)`, `F(c)` (R-6.6) for `if`, `?:`, `&&`, `\|\|`, loops and `assert`; dereference, arithmetic and comparisons require narrowed values (R-6.2, R-6.23) |
| Warnings | unreachable code (R-8.27), `catch` behind the `catch` of a upper class (R-9.7) |

A message identifies the violated rule and the rules that give rise to the violation, such as `[R-6.2, R-6.11]` for access to `x = null;` (V-14.2).

`rppc test ..`: pass all 242 conformance tests (translate or reject, warnings in the marked lines); the category “open” no longer exists.

Three clarifications have emerged during the implementation, adopted on 21.11.2026: V-12 (Chapter 5, branches without normal end), V-6.1 (Chapter 6, declared type for `?.`, `??` and `== null`) and V-14.2 (Chapter 14, several rules in one notification). In addition, tests that violated Chapter 5 have been corrected:
- in 13 rejected Chapter 6 tests, the field `Node?[] slots` remained unset (R-7.7);
- `a03_fields.rpp` (chapter 6) called `take(&n.child)` while the Borrow `ref Node? c = n.child` still existed; the Borrow now stands in its own block (R-5.17);
- `a04_lambdas_generics.rpp` (chapter 6) moved in `T? get() { return value; }` from a field; the method now supplies `ref T?` (V-3, V-4);
- `a06_generics.rpp` (chapter 7) shifted in `Util::first` from one element; the generic method is now `Util::count` (V-3);
- `n04_used_after_rethrow.rpp` (Chapter 9) was correct after V-12 because the branch does not end normally with `throw e;`; the test now checks that the catch variable is moved to `Exception kept = e;` (R-9.8);
- `n05_loan_conflict.rpp` (Chapter 10) additionally contained a type error (`i64` on `i32`).

Open in M3: comparison with the FR++ model. The handwritten FR++ programs (`fr-model/tests/`) and random programs from `fr-model/frpp/gen.py` are to be translated into R++ according to Section 7 of Chapter 5 and accepted or rejected by `rppc` as well as by the model.

**M3 completed (21.11.2026).** Comparison with the FR++ model (`fr-model/compare.py`, chapter 5, section 10.5) translates FR++ programs into R++ and compares the judgment. result: all 32 translatable handwritten programs and 4466 translatable out of 5000 random programs judged equal, no deviation; the counter-test with disconnected model rules finds deviations for each rule. Six gaps in `rppc` were found and fixed (`&` on argument, replace behind `Foo&`, move to `shared`/`weak` parameters, Borrow return to a `shared` field, move to a Borrow return, order in `x.f = g(x)`). The 242 conformance tests continue.

V-13 (Chapter 5) has been decided: A `own` value at a `weak` target is an error (R-4.41); plus the test `ch05_ownership/rejected/n21_own_to_weak.rpp`.

**M4, first part (22.11.2026).** New is the Crate `rppc/crates/rpp-interp` (debug interpreter) and the command `rppc run [--release] DATEI|PROJEKT [-- ARGUMENTE]`.

| Particle | Content |
|---|---|
| Test details (`rpp-sema/src/rt.rs`) | per expression (key: file and number of the expression, new in the AST): static type, target type when storing whether the value goes to an owner (move or counted copy), target of each call (selected overload, constructor, built-in function, enum value), captures of own lambdas, base constructors. The standard library is also checked for this. |
| Mode mode | The workspace loads in debug or release mode (symbol `debug` or `release`, R-11.24). |
| Values (`rpp-interp/src/value.rs`) | Possessional objects at reference (`own`, `shared` with counter, `weak`, Borrow), vectors, tuples, enums, function values; variables as cells capturing normal lambdas |
| Release | at the block end in reverse order, at assignment (old value), for temporary values at the end of the statement; destructors according to V-7 in the reading V-14; shared with reference counter, weak over `alive` |
| Construction | Base part, field initializer, body; virtual calls go to the class of the object already in the constructor; throws a constructor, is cleaned up after R-9.13 |
| Exceptions | Unwinding with release and `finally`; program errors (`IndexError`, `OverflowError`, `DivisionError`, `CastError`, `AssertError`, `FailError`, `DestructorError`, `StackOverflowError`, `BorrowError`) also clean up but are not caught (R-9.14); stacktrace in debug mode |
| Numbers | Types exact, overflow, push and `cast` to R-12.5 per mode, division by zero in both modes a program error |
| Locking | V-1 as in the FR++ model: locations with shared root variable; receivers and borrowed arguments for the duration of the call, `ref` and loops to the block end |
| End date | Exit code from `main` after release of the variables, 1 for an exception (`describe()` on the error output), 2 for a program error (R-12.9); at least 10 000 nested calls (limit 12 000) |

`rppc test ..` now runs the runtime tests in both modes: 241 passed, 0 failed, 2 open. The two open tests from chapter 13 (`l01_text`, `l03_set_reihenfolge`) need built-in parts of the library (`StringBuilder`, `Set`), which according to the plan belong to M5.

First state of comparison with the FR++ model (`fr-model/compare.py --run`): Out of 1000 random programs, 226 are accepted and executed by both. 212 deliver the same output including the destructors, as well as all 10 handwritten programs. There are 14 differences open (Chapter 5, Section 10.5 and `fr-model/comparison-report.txt`): 13 endless destructor cascades where the model ends with `BorrowError` and `rppc` with `StackOverflowError`, and an unresolved blocking conflict (seed772).

Change to plan (adopted on 22.11.2026): Section 3.3 provided for an interim representation as a control flow graph executed by the interpreter; instead, the first part of M4 executes the tested AST together with the particulars of the test; release, lock, unwinding and tests per mode are expressly in the interpreter. The interim representation is only created with the JIT or the release mode when it is used for code generation; until then the AST interpreter is the reference (R-12.3).

Still open after the first part: the test mode for the invariants, `new T()` in generic hulls and iteration over `Iterable` classes (with M5).

**M4 completed (22.11.2026).**

| Particle | Content |
|---|---|
| Test mode | `rppc run --check-invariants`; `rppc test` performs all runtime tests with it. After each top level statement of `main`, the interpreter checks I-1 (unique ownership, reference counter), I-2 and I-4 (no references to shared objects) and I-5 (living locked objects), at the end leaks in programs without `shared`. A breach is an internal error with exit code 3rd cross-check: An intentionally built-in error (old value not released when assigned) is reported. |
| Program error when cleaning up | If a second one arises when cleaning up after a program error, the first one remains the reported cause, the second one is attached and no further destructors run (R-9.14: the program ends immediately). |
| Locking | A borrowed object argument with shared root is blocked for invocation and bound, not read; primitive arguments are read (as in the FR++ model). |

Comparison with the FR++ model, with test mode: all 10 handwritten programs and 466 of 467 random programs executed (from 2000) run the same; no invariant is injured. The remaining difference (seed1070) is the model: it does not clean up after a program error, R-9.14 requires this (`fr-model/comparison-report.txt`).

`rppc test ..`: 241 passed, 0 failed, 2 open to M5 (`StringBuilder`, `Set`).

Further open and intended for M5: `new T()` in generic hulls and `for` above `Iterable` classes.

Next is M5: the built-in parts of the standard library (`rpp.text`, `rpp.collections`, `rpp.io`, `rpp.env`, `rpp.fmt`).

**M5, first part (22.11.2026).** The classes of the standard library are further than R++ source text in `rppc/std/rpp/…`; methods with the hull `fail("eingebaut")` is carried out by the interpreter himself (`rpp-interp/src/stdlib.rs`):

| Package | Built-in |
|---|---|
| `rpp.collections` | `Map` (Entries as `Entry` objects in insert order, `put` replaced and retains position, `at` with missing key is a program error), `Set`; passes over `begin`/`done`/`next`/`at` |
| `rpp.text` | `StringBuilder` |
| `rpp.fmt` | `padLeft` `padRight`, `hex`, `fixed` (correctly rounded) |
| `rpp.io` | `Console` `File` (`open` tosses `IOException`, `readAllText` tosses `FormatException` in the case of invalid UTF-8), `Dir` (`list` sorted), `Path` (separator `/`) |
| `rpp.env` | `Env::get`, `Env::os` |
| `rpp.core` | `hash` `equals`, `compareTo`, `less` for primitive values and strings (`hash` from strings: FNV-1a over UTF-8) |

To this end: `for` on classes with: `Iterable<T, C>` (R-13.10When an object is shared, the built-in contents are now also released (the test mode had a leak in the `Map` found.

New run-time tests: `ch13_standard_library/runtime/l05_map_set.rpp` and `l06_fmt_and_missing_key.rpp`. `rppc test ..`: pass all 245 conformance tests, the run-time tests in both modes and in test mode. This achieves the goal of M5 (pass all conformance tests).

Still to be seen:
- `new T()` in generic hulls: the interpreter does not yet carry type arguments at runtime (R-7.26 requires this for the interpreter);
- File and directory tests: You need a specified test directory (R-11.19, R-11.20);
- `Map` and `Set` search linearly; for the later compiler it needs a hash index;
- V-15 (Chapter 5): Today’s rules do not allow `Map.remove` to be written in R++, so `Map` and `Set` are built-in instead of source text.

**V-15 Implemented (22.11.2026).** dismantling of one of the `own`-object by R-5.25 in `rppc` (Examination and interpreter) and in the FR++ model (parser, examiner, interpreter, translation to R++). `a07_destructure.rpp`, `l01_destructure.rpp`, `n22` up to `n26`; in the model `a11_destructure.fr`, `n25_destructure_with_destructor.fr`. `rppc test ..`: pass all 252 conformance tests, allowing `Map` and `Set` to be written as R++ source text; they remain installed for now.

**M5 completed (22.11.2026).**

| Particle | Content |
|---|---|
| Typical arguments at maturity (R-7.26) | An object carries the type arguments of its class; methods, constructors, field initializers and lambdas derive from this the occupation of their type parameters via `this` and also via base classes. `new T()` thus generates the correct class. Type arguments of generic methods are not yet included (`new T()` in a generic method is an internal error). |
| Hash index | `Map` and `Set` find keys via an index hash value → positions; `hash()` They call their own classes. `remove` The index is rebuilt. |
| Working list of run-time tests | `rppc test` starts each runtime test per mode in a fresh, empty directory and then deletes it (V-14.3 in chapter 14, adopted on 22.11.2026). |

New runtime tests: `ch07_classes/runtime/l04_new_t.rpp`, `ch13_standard_library/runtime/l07_map_custom_keys.rpp` (2000 entries), `l08_files.rpp` (`File`, `Dir`, `Path`, `IOException`). `rppc test ..`: pass all 255 conformance tests, the runtime tests in both modes and in test mode.

Next, following the plan M6: the start of the R++ compiler in R++.

**M6 started (22.11.2026): the Lexer in R++.** The project `rpp-compiler/` (name `rppcc`) contains a lexer in R++(`src/compiler/lexer/`), equivalent to the Rust lexer. `rppc run ../rpp-compiler -- tokens DATEI` and `rppc tokens DATEI` both issue one line per token; `rpp-compiler/vergleich_lexer.py` compares it for all 503 `.rpp` files of the project (including the lexer itself) and 14 border cases: 517 equal, 0 different. Counter-test: An intentionally removed punctuation mark leads to 18 deviations.

The lexer was accepted and executed without any change to the test or interpreter, but changed only that `for` the characters were once decomposed over a string (previously quadratic runtime).

Further in M6: preprocessor (conditional compilation), then the parser with AST; is compared with `rppc` (acceptance, rejection, first error line, readings).

**M6: Preprocessor and parser in R++ (22.11.2026).** `rpp-compiler/` now contains, in addition to the Lexer, the preprocessor (`src/compiler/pre/`, with its own exception class for errors in directives and an enum with data for the conditions) and the parser (`src/compiler/syntax/`, about 1300 lines R++). The syntax tree has a uniform structure (art, text, position, children); its text form is generated on the Rust side `rpp-syntax/src/dump.rs` from the AST of `rppc` Compare with `rpp-compiler/compare.py`:

| Particle | Commander | equals | different types |
|---|---|---|---|
| Lexa | `tokens DATEI` | 521 | 0 |
| Preprocessor | `pre SYMBOLE DATEI` | 1056 | 0 |
| Parser with test R-8.12 | `ast SYMBOLE DATEI` Tree or first error with position | 1034 | 0 |

Entries are all `.rpp` files of the project, including negative grammar tests and readings, and borderline cases; preprocessor and parser with two sets of symbols.

The parser initially ran only for 964 out of 1032 cases: the interpreter did not postpone a `own` argument when it came across `??`, `?:` or brackets, leaving a reference to an already released node. The test had treated the printout correctly as a move, fixed, and passed the run-time test `ch05_ownership/runtime/l02_own_argument_expression.rpp`. `rppc test ..`: 256.

Further in M6: the name resolution and type checking in R++ (as `rpp-sema`), compared over the messages of the conformance tests.

**M6: Load projects in R++ (22.11.2026).** First stage of name resolution: `rpp-compiler/src/compiler/project/` reads `rpp.yaml` (own reader for the YAML part R-11.13, as `yaml.rs`) and loads projects, source folders, local dependencies and the standard library as `project.rs`, with the messages to R-11.13 and R-11.15 to R-11.17. Source files reads `compiler/syntax/Source.rpp` (preprocessor, lexer, parser, R-8.12) with the first error as `SyntaxError`. Compared is the output of `projekt [--release] STD PFAD` (projects, files with declared and expected package and type names, messages): 592 equal, 0 different. Entries are all project directories and individual files of the conformance tests, `rpp-compiler` itself and 39 border cases for `rpp.yaml`, each in debug and release mode.

Modified:

- `rppc` Reports false forms in `rpp.yaml` Now as an error ()R-11.13: `sources`, `dependencies` and the symbols are lists, `type` which is not a text, a `path`, which is not a text. `[x]` read as a project directory and `main.rpp` Double-charged.
- Paths that do not exist are normalized `rppc` textually (`.` and `..`); the standard library is loaded with canonical path.
- New in the standard library (V-13.8, adopted): `File::exists` and `Path::absolute`; runtime test `ch13_standard_library/runtime/l09_paths.rpp`. `rppc test ..`: passed 257.

Next in M6: Collect declarations and resolve names (like `defs.rs`), compared over the messages.

**M6: Declaration phase in R++ (22.11.2026).** `rpp-compiler/src/compiler/sema/` contains the type representation ()`Ty.rpp`, such as `ty.rs` Types are unchangeable and are considered `shared Ty` The declaration tables (divided)`Defs.rpp`) and `Program.rpp` with all phases of `Program::build` Of an extract `defs.rs`: Collect types ()R-11.4Resolve type names ()R-11.7, R-11.8, R-11.10, R-11.12, R-7.23, R-4.37, R-4.45, R-10.13), base classes, imports and heredity cycles (R-7.12, R-7.20fields, methods, constructors and enum variants (R-4.25, R-4.26, R-7.27), constraints (R-7.25, R-7.26), files (R-11.2, R-11.14, R-11.18) and classes (R-7.1, R-7.6, R-7.10, R-7.11, R-7.13, R-7.14, R-7.16, R-7.18, R-7.20, R-7.21, R-8.15, R-11.11).

`defs STD PFAD` (`rppc/crates/rppc/src/defsdump.rs`): all types with type parameters, constraints, base, imports, fields, method and constructor signatures and variants, plus the messages of this phase in the order of `rppc`. Entries are the projects and individual files of the conformance tests, `rpp-compiler` themselves, the grammar tests and 17 borderline cases: 501 equal, 0 different. `protected` at the rank of `public` deviates from a borderline case.

Noticed and fixed: The interpreter moved an enum without data and a tuple from basic values when they went to an owner ()`Access acc = access;` The test treats both as a copy ()`movable`). The source was then left as moved, and a later reading broke with an internal error. New runtime test `ch05_ownership/runtime/l03_copy_enum_and_tuple.rpp`; `rppc test ..`: 258 passed.

Further in M6: the body checking (`body.rs`, `flow.rs`): expressions, statements, null safety, ownership and flow analysis, compared over the reports of conformance tests.

**M6: Test of hulls in R++ (22.11.2026).** `rpp-compiler/src/compiler/sema/Checker.rpp` (approximately 3300 lines) transfers `body.rs`: statements and expressions with types, overload resolution and derivation of type arguments, lambdas, built-in methods and functions, plus flow analysis `flow.rs` (`Flow.rpp`: narrowing, safe initialization, moves, loans, fixed point on loops, try/catch/finally). Thus, `rpp-compiler` R++ checks programs completely as `rppc check`. the data for execution (`rt.rs`) are not transferred; they come with the code generator.

For the examiner, the examiner owns the program and lends the syntax trees; `Program` keeps for each file project, package and `using` entries. Quantities and assignments of the flow analysis are sorted lists; integer literals above `i64::MAX` are present as sign and `ui64` amount (literals above `ui64::MAX` are considered as invalid, unlike in `rppc`).

Comparison is made `pruefe STD PFAD` (all messages with warnings and justification rules, sorted as `rppc check`). Entries are the projects and individual files of the conformance tests, the grammar tests, the borderline cases of the declaration phase and the FR++ programs translated to R++ (handwritten and 300 random programs): 782 equal, 0 different. Counter-test: with R-6.9 instead of R-6.8 as justification after a `if`, a Conformance test differs. `rpp-compiler` also check both sides equally (no message); in the interpreter this takes a good 20 minutes, mainly because of the fixed point on nested loops.

Further in M6: the data for the execution and a code generator (intermediate representation or C), then the self-translation.

**M6: Code generator after C and self-translation (22.11.2026).** Decided (22.11.2026): The code generator generates C (C17, translatable with MSVC and gcc), not first its own intermediate representation.

| Particle | Content |
|---|---|
| Execution details | `Checker.rpp` collects them in the same places as `body.rs` (`Rt.rpp`, as `rt.rs`): types, target types, moves, call targets, basic constructors, types of declarations; plus the collections of all Lambdas. The nodes are numbered in the workspace (`Node.id`); `withStd` also checks the standard library. |
| Code generator (`compiler/gen/CGen.rpp`) | implements the interpretation of the debug interpreter site by site in C (`eval`, `eval_store`, `transfer`, `temp`, `convert`, `args`, all statements); errors run as statuses with clean-up marks per statement and scope; virtual calls via distributors per method; generics without duplication, type arguments on the object (R-7.26) including the view of a base class |
| Duration (`runtime/rpp_rt.c`) | Values with nature and ownership such as `value.rs`; release with counters for `shared`, `weak` and strings; numbers of both modes with 128-bit intermediate values such as `i128`; program errors, stack trace, exit codes (R-12.9); 12 000 calls on a separate stack; built-in parts of the standard library |
| Commander | `rppcc c [--release] STD PFAD AUSGABE.c` |

The behavior is compared to `rppc run` (`rpp-compiler/compare_c.py`: generate, translate with MSVC, execute in the fresh working directory; output, error output and exit code):

| Entries | equals | different types |
|---|---|---|
| Durability tests of the conformance tests, both modes | 84 | 0 |
| `rpp-compiler/tests_c` (Lambdas, Enums with data, `finally`, Tupel, Slices, `shared`/`weak` ...), both modes | 2 | 0 |
| FR++ programs (handwritten and random programs), both modes | 138 (69 Programs) | 0 |

**Self-translation.** The interpreted `rpp-compiler` generates C code for itself (217,783 lines, 20 minutes in the interpreter); MSVC translates it into just over a minute `rppcc.exe`.
- generates for itself the same C-code, byte by byte (fixed point; 3 minutes 50);
- agrees in all parts of `compare.py --exe` with `rppc`: `tokens` 539, `pre` 1092, `ast` 1070, `project` 596, `defs` 507, `diagnose` 789 is equal, 0 different; `diagnose` now inclusive `rpp-compiler` itself (3 minutes 45 instead of about 20 minutes in the interpreter).

When comparing, you noticed and resolved: A view of a `weak` reference was not counted during storage (runtime, memory errors in FR++ programs).

**Speed (22.11.2026).** Measured with a sample profiler in runtime (`RPP_PROFILE=FILE`, Windows; addresses resolved via the linker's map file).

| Cause | Restructuring | `diagnose rpp-compiler` | Self-Translation |
|---|---|---|---|
| Starting point | – | 225 s | 230 s |
| small runtime functions as real calls between two translation units | fast paths `static inline` in the header (secures, intermediate values, release of basic values, integer calculation and comparison, index, `length`); common number type as table | (with 2.) 28 s | 101 s |
| Flow analysis in `Flow.rpp`: sorted lists linearly searched, copies element by element | binary search, Schnitt/Vereinigung/Differenz by merging, `copy(…)` for copy; results unchanged | | |
| chatter code for basic values | Generator omits `temp`, `promote`, `own_str` and double number conversions at basic values; blocking test (V-1) only if the root variable `shared` or a type parameter can be | 9,2 s | 10,4 s |

For comparison: `rppc diagnose` (Rust) takes 0.6 seconds, the interpreter about 20 minutes. The fixed point of the self-translation holds after each step (two laps, because the new generator runs only in the second lap).

**Further open points (22.11.2026).**

| Point | Implementation |
|---|---|
| `new T()` with the type parameter of a generic method | The test holds the derived type arguments to the selected call (`MethodInfo.targs`, in `body.rs` and `Checker.rpp`). The interpreter places them within the scope of the method as seen from the caller and takes them into lambdas; the code generator passes them to the method via `rt_set_mtargs` and to lambdas via `RFun.mt`. New runtime test `ch07_classes/runtime/l05_new_t_method.rpp` (also via another generic call, an instance method and a lambda); `rppc test ..`: 259 passed. Generic methods cannot be overwritten (R-7.13), virtual calls still retain the type arguments. |
| Unicode for `char` and Strings | The C-runtime takes its tables from Rust's standard library (`runtime/gen_unicode.rs` created `rpp_unicode.h`: letters, uppercase and lowercase letters, blank space, images up to three characters) and Case_Ignorable from the Unicode categories (`gen_unicode_ignorable.py`) for the final sigma. Additional test `rpp-compiler/tests_c/t02_unicode.rpp`. |
| Text of `IOException` | V-13.12 (Chapter 13, adopted on 23.11.2026): The text names the path and describes the error; the exact wording, especially the part of the operating system, is not fixed. |

Open: The translated code is still about 15 times slower than `rppc` (values with type in each expression, frame and field per call, name comparison for built-in methods); the release mode via its own intermediate representation or LLVM remains open according to the plan.

**M6 completed (23.11.2026).** Criterion (determined on 23.11.2026): The R++ compiler translates itself (fixed point) and agrees with `rppc` in all comparisons. Status: Fixed point after two rounds; `compare.py --exe` 540/1094/1072/598/508/790 Equal (tokens, pre, ast, project, defs, diagnose), 0 different; `compare_c.py --fr 200` 228 Equal, 0 different; `rppc test ..` 259 passed.

Decided on 23.11.2026: V-13.12 (text from `IOException`) is accepted. `Map` and `Set` remain installed; as R++ source text only when the generated code is faster. The speed of the translated code becomes milestone M7; whether the C code generator becomes the release mode after R-12.1 (instead of its own intermediate representation or LLVM, Section 3, is yet to be discussed.

**Expansion of the standard library: rpp.random (23.11.2026).** First package of expansion (V-13.9; Standard Library, Sections 9 and 10, R-13.23 to R-13.25). `rppc/std/rpp/random/Random.rpp` is pure R++ source text without built-in parts: xoshiro256** with SplitMix64 seeding, plus `nextUi64`, `nextInt`, `nextFloat`, `nextBool` and the generic `shuffle<T>(mut T[] v)`.

| Particle | Content |
|---|---|
| Arithmetic modulo 264 | Addition and multiplication in 32-bit halves, without overflow ()R-4.14); both modes run the same.R-4.16). |
| `nextInt` | uniformly distributed without modulobias by discarding below a threshold; `bound ≤ 0` is a `FailError` |
| `shuffle` | Fisher-Yates about `remove`/`insert`: From elements is not moved out ()V-3), and `swap` on two indices is always considered to be overlapping (Chapter 5, Section 7). `own`Objects. |

The expected outputs of the run-time tests are calculated with an independent reference implementation (python) and recorded in the test heads: `a06_random`, `l20_random_folge` (fixed sequences for seeds 42, 0 and 7, reproducibility, `nextInt`, `nextFloat`, `nextBool`), `l21_random_shuffle` (basic values, `own` objects, strings, empty vector; Seed 2026), `l22_random_bound` (`FailError` at `nextInt(0)`).

`rppc test ..`: passes all 272 conformance tests, the runtime tests in both modes and in test mode. Counter-test with the compiler in R++: `diagnose` reports for `Random.rpp` and the new tests on both sides the same (no) reports. The modifiable vector parameter `mut T[]` with `remove`/`insert` by the test was thus used for the first time in a library package.

**Strengthening: vector algorithms and a test fix (23.11.2026).** `rppc/std/rpp/collections/Vectors.rpp` (pure R++ source text, V-13.10) supplemented `rpp.collections` with static generic methods: `binarySearch` (Comparable), `equals` (Equatable, elementary comparison – arrays have no `==`, V-4.8), `reverse`, `retain` (predicate) and `dedup` (successive duplicates). The modifiable methods work over `remove`/`insert` (V-3; `swap` on two indices is always considered overlapping) and move the elements; they also work for `own` objects.

| Particle | Content |
|---|---|
| Auxiliary class instead of installed | Written as R++ class, not as built-in methods on `T[]`: both compilers treat them the same, no divergence in M6V-13.10Built-in methods remain what R++ cannot efficiently express ()`sort`). |
| Test fix R-7.26 | When writing, Constraints of generic **methods** were not checked on the call (only class constraints about `check_pending`). `binarySearch` with a type without `Comparable` was accepted and failed only at runtime (`InternalError: Methode compareTo ohne Rumpf`). In `body.rs` (after the method choice, the derived type arguments against the constraints tested) and in `Checker.rpp` fixed. R-7.26 already required the test; the fix makes the implementation spec-compliant. |

New tests: `a07_vectors`, `l23_vectors_suche`, `l24_vectors_mutieren`, `n08_vectors_ohne_comparable` (R-7.26). `rppc test ..`: pass all 276 conformance tests, run-time tests in both modes and in test mode; `cargo test` without error. Counter-test with the compiler in R++: `diagnose` reports for `Vectors.rpp` and the four new tests on both sides the same (the negative test is rejected as over with R-7.26).

**Expansion: rpp.math (23.11.2026).** `rppc/std/rpp/math/Math.rpp` adds `f64` functions to the library (V-13.11; Standard Library, Section 12, R-13.28, R-13.29): the constants `PI` and `E`, `sqrt`, `floor`, `ceil`, `trunc`, `round` and `clamp`.

| Particle | Content |
|---|---|
| Rounds and clamp | pure R++ source text. `floor`/`ceil`/`trunc`/`round` round exactly integers over `cast<i64>` (direction zero, R-4.10) with adjustment; `round` away from zero at .5. NaN (`x != x`), ±0 and \|x\| ≥ 252 (integers, also ±∞) remain unchanged – as a result, both modes run the same and the sign of zero is retained. `clamp` Requires `lo <= hi` (otherwise `FailError`). |
| sqrt | Built-in (`fail("eingebaut")`, Dispatch in `stdlib.rs` over `rpp.math.Math`), correctly rounded to IEEE 754 as `Fmt::fixed`; `x < 0` gives NaN. As an R++ class with built-in body, both compilers treat the signature the same – no M6 divergence (V-13.11). |

The target outputs are calculated using an independent Python reference (e.g. `sqrt(2)` to 10 digits, `PI`/`E` to 5 digits), new tests: `a08_math`, `l25_math_runden` (rounds, clamp, NaN/∞), `l26_math_sqrt` (squares, correctly rounded `sqrt(2)`, NaN/∞, constants), `l27_math_clamp_fehler` (`FailError` at `lo > hi`). `rppc test ..`: pass all 280 conformance tests, the runtime tests in both modes and in test mode; `cargo test` without error. Counter-test with the compiler in R++: `diagnose` reports for `Math.rpp` and the four new tests on both sides the same.

**Extension: Text helps (23.11.2026).** `rppc/std/rpp/text/Text.rpp` supplemented `rpp.text` with static methods on `string` (Standard Library, Section 13, R-13.30): `lines`, `removePrefix`, `removeSuffix`, `repeat` and `splitLimit`. The package is **pure R++ source text without built-in parts** and without any change to the rust code – it relies only on existing string methods (R-13.13), `StringBuilder` and `string::join` (V-13.9, V-13.10, pattern `Fmt`).

| Particle | Content |
|---|---|
| `lines` | above `split("\n")`, `removeSuffix(…, "\r")` for CRLF; a final `\n` does not produce an additional empty line, `""` gives an empty vector |
| `removePrefix`/`removeSuffix` | `startsWith`/`endsWith` plus `substring`; otherwise unchanged |
| `repeat` | `StringBuilder`; `n < 0` is a `FailError` |
| `splitLimit` | `split` and `string::join` for the undivided remainder in the last part; `limit < 1` or empty separator is a `FailError` |

New tests: `a09_text`, `l28_text_lines`, `l29_text_hilfen`, `l30_text_split_limit`, `l31_text_repeat_fehler`, `l32_text_split_limit_fehler`. String results are encapsulated in the tests in `[…]` so that empty strings are clearly testable. `rppc test ..`: passes all 286 conformance tests, the runtime tests in both modes and in test mode. Counter-test with the compiler in R++: `diagnose` reports for `Text.rpp` and the six new tests on both sides the same.

**Extension: File operations (23.11.2026).** `File` (in `rppc/std/rpp/io/Io.rpp`) receives five more static methods (Standard Library, Section 14, R-13.31): `appendText`, `copyFile`, `moveFile`, `delete`, `size`. They are built-in (`fail("eingebaut")`, Dispatch in `stdlib.rs`) because they need file system access – as already `File`/`Dir`. As an R++ class, both compilers treat the signatures equally (no M6 divergence, V-13.10).

| Particle | Content |
|---|---|
| `appendText` | attaches UTF-8, creates a missing file (`OpenOptions` append+create) |
| `copyFile` | `std::fs::copy` provides the byte number; replaces an existing target |
| `moveFile` | `std::fs::rename`; if this fails because the target exists (Windows), it is removed and renamed – so both platforms replace an existing target equally |
| `delete` / `size` | `remove_file` or `metadata().len()`; `IOException` if the file is missing |

`copy` is a keyword (chapter 7), therefore `copyFile`/`moveFile`. new tests: `a10_io`, `l33_io_datei_ops` (all operations in the fresh working directory, V-14.3), `l34_io_fehler` (`IOException` if file missing). `rppc test ..`: pass all 289 conformance tests, the runtime tests in both modes and in test mode; `cargo test` without error. Counter-test with the compiler in R++: `diagnose` reports for `Io.rpp` and the three new tests on both sides the same. Stay open buffered reading and Mtime (the latter needs `rpp.time`, phase B).

**Improvement: Elemental vector algorithms (25.11.2026).** `rppc/std/rpp/collections/Vectors.rpp` (pure R++ source text, V-13.10) supplemented `rpp.collections` with the elementary algorithms: `count<T : Equatable<T>>(Slice<T> s, T x)` (number of occurrences, R-13.38) and `any<T>(Slice<T> s, Func<T, bool> predicate)`, `all<T>(…)` and `none<T>(…)` (predicates, R-13.39). Porte: C++ `<algorithm>` (`count`, `any_of`, `all_of`, `none_of`), Rust `iter().filter(..).count()` and `Iterator` respectively, Python `list.count`.

| Particle | Content |
|---|---|
| `count` | Count over the slice; empty slice gives `0`; `Equatable` necessary (R-13.38, R-7.26). |
| `any`/`all`/`none` | Predicate about the slice; `any` ends on the first hit, `all` In the first counterexample, Empty Slice: `any` → `false`, `all` → `true`, `none` → `true`. |

New tests: `a07_vectors` (extended to `count`, `any`, `all`, `none`), `l35_vectors_algorithmen`, `n09_vectors_ohne_Equatable` (R-7.26, Constraint-Check as for `binarySearch` without `Comparable`). `rppc test ..`: pass all conformance tests, runtime tests in both modes and in test mode; `cargo test` without errors. The lazy view `zip`/`windowed`/`chunked` (Phase A, Roadmap) remains open.

**Expansion: rpp.math – Transcendent (26.11.2026).** `rppc/std/rpp/math/Math.rpp` (pure R++ source text, V-13.9; sample `sqrt`, V-13.11) supplemented `rpp.math` with the transcendents `pow`, `exp`, `log`, `sin`, `cos` and `tan` as incorporated (`fail("eingebaut")`, Dispatch in `stdlib.rs` about `rpp.math.Math`). Results are platform-dependent and not bit-equal; within a platform deterministic, both modes are the same.

| Particle | Content |
|---|---|
| `pow(x, y)` | `x.powf(y)` |
| `exp(x)` | `x.exp()` |
| `log(x)` | `x.ln()` |
| `sin(x)`, `cos(x)`, `tan(x)` | Native Sin/Cos/Tan |

New tests: `a08_math` (extended by `pow`, `exp`, `log`, `sin`, `cos`, `tan`), `l36_math_transzendenten` (values such as `exp(0) = 1`, `log(1) = 0`, `sin`/`cos`/`tan(0)`, `pow(2, 10) = 1024`). `rppc test ..`: pass all conformance tests, runtime tests in both modes and in test mode; `cargo test` without error.

**Expansion: rpp.math – Transcendent Supplements (26.11.2026).** `rppc/std/rpp/math/Math.rpp` supplemented `rpp.math` with the transcendents `log10`, `hypot`, `atan2`, `sinh`, `cosh` and `tanh` (built in, dispatch in `stdlib.rs`, as in `pow`/`exp`/`log`/`sin`/`cos`/`tan`, R-13.41) and the conversions `toDegrees`/`toRadians` (pure R++ source text, arithmetic about `Math::PI`).

| Particle | Content |
|---|---|
| `log10(x)` | `x.log10()` |
| `hypot(x, y)` | `x.hypot(y)` |
| `atan2(y, x)` | `y.atan2(x)` |
| `sinh(x)`, `cosh(x)`, `tanh(x)` | Native Hyperbolic Functions |

New tests: `a08_math` (extended to `log10`, `hypot`, `atan2`, `toDegrees`, `toRadians`, `sinh`, `cosh`, `tanh`), `l46_math_transzendenten_ergaenzungen` (values such as `log10(1000) = 3`, `hypot(3, 4) = 5`, `atan2(1, 0) = π/2`, `toRadians(180) = π`, `toDegrees(π) = 180`, `sinh`/`cosh`/`tanh(0)`).

**Extension: Text helps (26.11.2026).** `rppc/std/rpp/text/Text.rpp` (pure R++ source text, V-13.10) supplemented `rpp.text` with the text aids: `isBlank` (about the Builtin `trim`), `substringBefore`/`substringAfter` (about `splitLimit` with `limit = 2`) and `capitalize`, `toUpperCase` and `toLowerCase` (about `substring` and the Builtins `toUpper`/`toLower`). Porte: C#/.NET `IsNullOrWhiteSpace`, Java/Kotlin `substringBefore`/`substringAfter`, Kotlin/Rust `capitalize`, `toUpperCase`/`toLowerCase`.

| Methodology | Content |
|---|---|
| `isBlank(s)` | `true` if s is empty or only whitespace; above `trim`, empty result → `true`. |
| `substringBefore(s, delimiter)` | Part before the first occurrence; not found → s unchanged. |
| `substringAfter(s, delimiter)` | Part after the first occurrence; not found → empty. |
| `capitalize(s)` | First letter large, rest small; `capitalize("")` → `""`. |
| `toUpperCase(s)` / `toLowerCase(s)` | Total s in Groß-/Kleinschreibung. |

New tests: `a09_text` (extended by `isBlank`, `substringBefore`, `substringAfter`, `toUpperCase`, `toLowerCase`, `capitalize`), `l45_text_hilfen` (runtime). `rppc test ..`: pass all conformance tests, run-time tests in both modes and in test mode; `cargo test` without error.

**Expansion: rpp.iter – Lazy Iterators (26.11.2026).** `rppc/std/rpp/iter/Iter.rpp` (pure R++ source text), V-13.10, sample of `Vectors`) Complemented `rpp.iter` The Lazy Operations: `map`, `filter` and `take` over `Slice<T>` Porte: Rust `Iterator` Kotlin Sequence, C++20 ranges.

| Methodology | Content |
|---|---|
| `map(source, f)` | new element per source element below `f`; order and length remain. |
| `filter(source, p)` | keeps the elements for which `p` is true. |
| `take(source, n)` | The first `n` elements. |

New tests: `a12_iter` `l47_iter_lazy` (map, filter, take, flatMap, chain). `rppc test ..`: pass all conformance tests, runtime tests in both modes and in test mode; `cargo test` without error.

Addendum (26.11.2026): The first version (`T[] map<T>(Slice<T> …)`) moved with `result.push(source[i])` elements from a generic slice; both compilers report this (five times R-5.3). `rppc test` It does not notice, because hulls of the standard library are only checked when the whole program and library is checked (`rppcc c`); the self-translation therefore broke off. Now the elements are `shared T` (`Slice<shared T>`, `shared T[]`, R-13.42); `l47_iter_lazy` provides in `flatMap` `shared string[]`.

A hole in R-4.31 was striking, in both compilers: when viewed as a slice and between arrays, only the element type without ownership form was compared (`unformed`).A `Box[]` went through as `Slice<shared Box>`; the divided references read from it survived the vector, and the program then read a shared object, even in check mode without message. Now the shape must match, except for element types that are not shifted (`same_element` in `body.rs`, `sameElement` in `Checker.rpp`; conversion, arrays and order of priority of overloads). New test `ch04_types/rejected/n28_slice_besitzform` (counter-test: the old state does not report anything).

**merge of library expansion and C-run time added (23.11.2026).** The expansion of the standard library (top, branch `temp_merge`) is merged with M6; use the new parts `mut` instead of `&`. V-13.9 was doubled, the rule to the text of `IOException` is now called V-13.12. The C runtime (`rpp_rt.c`) now knows all new built-in parts such as `stdlib.rs`:

| Particle | Reaction in C |
|---|---|
| File operations, `Dir.entries`, `Path.normalizedText`/`relativeText` | as `stdlib.rs`; paths purely lexical as `lexical_path`; type of entry about `lstat` or the attributes under Windows |
| `SaveFile` | temporary file next to the target (exclusively created), `commit` renames and replaces an existing target, `cancel` and the destructor delete it |
| `StringView`, `Math.sqrt`, `ElapsedTimer.monotonicNanos` | Viewings as objects with `source`, `byteStart`, `byteEnd`; UTF-8 signs |
| `Map.getOrPut`, `Set.appendEntry`, `partitionPoint` | `appendEntry` divides `own` objects between both sets as the holders in the interpreter |
| Unicode Paths on Windows | File accesses and command line arguments about the `wchar_t` versions (UTF-8 ↔ UTF-16) |

In the code generator: `for` above `Iterable` takes the distributor, like the interpreter, in a method without hull (`this` in a mixin, for example `Iterable.any`), the class of value; the distributor distinguishes overcharges according to `mut` (`Map.at(i64)` next to `Map.at(K) mut`).

When creating C (checking with the standard library hulls), two errors were noticed in the new library code, which `rppc` does not report because it does not check the standard library hulls: `Random.mix` assigned its parameter (R-8.19), `Deque.front`/`back` returned `null` or a borrow that does not originate from `this` (R-5.23). Both are fixed; the expenses remain the same.

Status: `rppc test ..` 291 passed; `compare_c.py --fr 200` 278 equal, 0 different; fixed point of self-translation (round 1 to 3 bytes equal for byte, a good 10 s per round); `compare.py --exe` 581/1176/1154/670/540/822 equal (tokens, pre, ast, project, defs, diagnosis), 0 different.

**M7 started: Expansion of the C code generator (24.11.2026).** Decided (24.11.2026): The release mode according to R-12.1 is created by removing the C code generator from M6, not via its own intermediate representation or LLVM. The generator remains tested against the interpreter (`compare_c.py`), the fixed point of the self-translation must keep up after each step. A target value for M7 has yet to be set.

Starting point (as measured in M6, MSVC `/O2`): `diagnose rpp-compiler` 9.6 s, self-translation 10.2 s; `rppc diagnose` (Rust, Release) 0.54 s. The profile showed 32% in a single binary search (`IdSet.lowerBound` in `Flow.rpp`) and 11% in `rt_deep_copy`.

| Step | Implementation | `diagnose rpp-compiler` | Self-Translation |
|---|---|---|---|
| Starting point | – | 9,6 s | 10,2 s |
| Native baseline | Pure expressions about `i8`–`i64`, `char` and `bool` (literals, local variables, fields and field chains without lock, index, `length`, operators; no calls) are evaluated by the generator directly in C as `int64_t` (`CGen.rpp`, section "Native base values"). Steps that can fail (overflow, division, index) stand as their own statements in the order of the interpreter; their fast fall is inline in `rpp_rt.h` (`rn_…`), everything else goes over the previous rv paths and behaves the same (messages, release mode). Conditions, assignments, interconnection assignments, `++`/`--`, `return` and native value declarations do not need a mark for intermediate values; nor do blocks and branches. | 5,4 s | 6,0 s |
| Copy of vectors | `rt_deep_copy` Copies basic values, enums without data and strings without call per element (838 000 copies with 89 million elements per element `diagnose`) | 4,5 s | |
| Objectives and variables | `Checker.rpp` keeps the type of local base value targets (native assignment); base value variables are not released at the end of the range; `return;`, `break`, `continue` without mark; slow cases of `rn_…` as separate functions | 3,9 s | 4,4 s |
| Base Value Variables | Local variables and parameters of type `i8`–`i64`, `char`, `bool` are C variables (`int64_t x…`) instead of values with a type in the field `V`.Except for names that capture a lambda that need `take`/`replace` as a location or are the target of a tuple assignment (`scanNoNat`); other writing paths (assignment of a non-native value, composite assignment, `++`/`--` as an expression) write the C variable directly. Read as value comes `rn_box`. | 3,8 s | 4,4 s |
| Memory managers | `xmalloc`/`xrealloc`/`xfree` in `rpp_rt.c`: blocks up to 1024 bytes from free lists per size class (16 bytes), fed from pieces of 1 MiB; larger via malloc. | 3,6 s | 4,1 s |
| Less appeals | Virtual calls go directly to the method instead of through the distributor if the receiver is statically an object of a class and no class overwrites the method (the same check as when the distributor was created; in the compiler, 3255 out of 3279 distributor calls are omitted). `rt_enter`/`rt_leave` (staple for the Stacktrace) and `rt_is_prim` are inline in the header. | 3,3 s | 3,8 s |
| Intermediate values individually | Intermediate values are individual C variables `t0, t1, …` instead of a field `T`, so that the C compiler can keep those that have not been submitted in registers; arguments, elements and decompositions get their own fields `A0, A1, …`, which are reused like the intermediate values per statement (the stacking frame does not grow). Counter-test: The variables (`V`) as individual C variables were also about 1% slower and discarded. | 3,1 s | 3,6 s |
| Transfer according to static knowledge | `rt_temp` not applicable if the previous line has assigned a value without ownership to the intermediate value (Sight `rv_view`, Literal, Enum without data, `null`, base value); this applies to approximately 15 000 out of 19 000 digits in the compiler. The gain is far greater than the proportion of `rt_temp` in the profile (about 1%) because the functions become smaller and MSVC `rt_temp` does not embed in large functions. Cross-check: Moving a `string` variable at its last use (liveliness backwards via the statements) did nothing, because parameters borrowed arrive and the counter increases when storing anyway; the effort lies in memory access to the string, not case differentiation. Rejected. | 2,55 s | 2,95 s |
| promote and own_str according to static knowledge | `rt_own_str` not applicable after `rt_promote` and `rt_transfer_var` (the borrowed strings and `weakb` already convert); `rt_promote` and `rt_own_str` not applicable immediately after the call of a generated, non-native method and after a constructor, because these always provide possessing values (each return goes over `store`, a second promotion would be ineffective). According to `rt_index` and built-in methods, they remain, where views can come back. 1700 out of 3300 `rt_promote` and 3200 out of 4600 `rt_own_str` are eliminated in the compiler. | 2,30 s | 2,66 s |
| Built-in methods according to number | The generator uses a number for built-in methods (`push`, `contains`, `substring`, ...) (`rt_bm` with `BM_…`, list `BM_LIST` in `rpp_rt.h` and `builtinMethodId` in `CGen.rpp`) instead of the name as a string; `rt_builtin_method` with name remains for older generated code. In `rt_native` stand Map/Set, `StringBuilder` and `StringView` front (own functions `native_collection`, `native_string_builder`); before that, every call of `Map.get` or `StringBuilder.append` passed by about 45 `strcmp` with full names. Newly measured on the same computer against the previous state (2.26 s / 2.65 s). | 2,24 s | 2,54 s |
| `Flow.join` Linear | Program work of the compiler, not the generator: `join` builds `lost` and `moved` in one run over the sorted lists (`IdMap::unite`, `IdMap::uniform`) instead of as a copy plus `put` per entry (binary search and `insert` per key). The result is the same as in `flow.rs`; moves that happened only in one way get their rule already when merging, measured against the previous stand in the same run (2.25 s / 2.57 s). | 2,01 s | 2,33 s |
| Fewer copies of river levels | Program work of the compiler: `noteTryState` copied after each statement the whole river level, even without open `try`; `join` reads it only, the copy is omitted. `endProbe` takes stand and loans from the secured `ProbeState` (`replace`), instead of copying them a second time. According to the profile, 9.5% went into `Flow.dup` before and a large part of the release went into these copies, measured against the previous state in the same run (2.02 s / 2.33 s). | 1,86 s | 2,17 s |
| `push` without distributor | The generator generates for `push` with an argument `rt_bm_push` (inline in `rpp_rt.h`): If the receiver is a vector, the element is directly appended (without `rt_bm`, without `vec_method`), otherwise the call to `rt_bm` will go as before. In the compiler 397 digits. Measured against the previous position in the same run (1.85 s / 2.18 s). | 1,80 s | 2,12 s |
| Sample of local names | Program work of the compiler: `argShape` checks each argument in advance as a sample (as `arg_shape` in `body.rs`: Secure, check, reset river level and loans). For a local name that does not capture a lambda, the sample yields `localType` and leaves nothing that it does not reset itself; `argShape` takes the type there directly. The sample to be omitted altogether if there is only one version without type parameter would not be equivalent: if an argument does not fit, `invoke` takes another path (without R-7.4, R-7.27, access, R-5.10), the messages differ from `rppc`. Discarded. Measured against the previous stand in the same run (1.77 s / 2.08 s). After that, 8.5% remain in `probeExpr`, of which 3.4% alone back up and reset (`beginProbe`, `endProbe`). | 1,73 s | 2,04 s |
| Strings inline | Only the runtime (`rpp_rt.h`), the generated code remains the same byte for byte: `rt_equal` compares two strings inline (same pointer or length and `memcmp`) instead of over `rt_equal_slow`; `rt_lit` delivers an already created literal inline (`rt_lit_new` creates it on first use); `rt_promote` upgrades a borrowed string inline; `rt_drop` downgrades a string inline and calls `rt_drop_slow` only for release. The profit is greater than the shares in the profile (`rt_equal_slow` a good 2%, `rt_promote_slow` 5%), because the calls are otherwise not embedded in large functions measured against the previous state in the same run (1.73 s / 2.04 s). | 1,53 s | 1,82 s |
| River Levels with Copy When Writing | The four sets of a `Flow` (`nonnull`, `lost`, `unassigned`, `moved`) are `shared` and are never altered; `Flow.dup` divides them instead of copying them; each change passes through methods of `Flow` (`addNonnull`, `removeNonnull`, `addUnassigned`, `removeUnassigned`, `putLost`, `takeLost`, `putMoved`, `takeMoved`) which replace the set with an altered copy, and only if something changes. Since `shared` objects are changeable according to V-1, the type checker does not check this; all write accesses in the checker are switched to these methods (`assigned`, Moves, `relabel`, capture by Lambdas). Measured against the previous stand in the same run (1.55 s / 1.84 s). | 1,39 s | 1,68 s |
| Lexa | Program work of the compiler: `punctLength` decides on the first character instead of a loop with 42 strings; `take` collects the text in one `StringBuilder` instead of one concatenation per character. Newly measured on 26.11.2026 (starting point on this day 1.30 s). | 1,24 s | |
| Objects inline | Only the runtime (`rpp_rt.h`): The frequent cases for objects are inline, as before for strings: `rt_drop` (Borrow does not release anything, `shared` only counts down), `rt_promote`, `rt_transfer_var`, `rt_to_shared`, `rt_check_lock`. Previously, 22 million calls of `rt_drop_slow` and 10 million of `rt_to_shared` each `diagnose`. | 1,15 s | |
| Memory per object | An object and its fields are in a block ()`rt_obj_new`; `vec_reserve`, `obj_free` and `clear` Take note of this because `Map`, `Set` and readers attach entries. an empty vector ()`[]`) gets its element memory only at the first element. | 1,12 s | |
| Values in whole words | A value (`rv`, 24 bytes) was written field by field and copied immediately afterwards as a 16-byte block; this prevents the store forwarding of the processor (around 12 bars per copy).`rv_mk` writes the first 16 bytes in a piece (SSE2) and type and ownership form as one word (`meta`); all constructors and `rv_view`, `rt_promote`, `rt_transfer_var` go over it. An invocation of a small method fell in the microbenchmark from 17 ns to 8 ns. control run: all helpers with `__forceinline` was slower (code size); gcc 15 and clang 22 provide from the same C code 1.43 s and 1.03 s, respectively, MSVC remains the measurement basis. | 0,96 s | |
| Parser aids | Program work of the compiler: `isP`, `isK`, `eatP`, `eatK` directly access the current token (`pos` is always in `toks`), instead of over `isPAt` and `idx`; `failure` builds the text only if the location is the furthest to date. | 0,90 s | |
| Loops: first run on suspicion | Program work of the compiler (`looped`): Out of 170,000 printout tests each `diagnose`, 131,000 ran mutely, as a sample or as a run of a loop in front of the fixed point. The first run of a loop now runs with messages and specifications for execution; when it reaches the fixed point, it is exactly the final run and stops. Otherwise it will be withdrawn (`rollbackSpec`: river level and loans as after a sample, reports shortened, information about a journal with the old values). | 0,80 s | |
| Samples of arguments | Program work of the compiler (`probeArg`): An argument is tested for the resolution of the overloads as a sample and then really; in the case of nested calls, this is repeated per level. The sample of a node is noted with the state at which it ran (the same four sets of the flow level as objects, the same visible variables); in the same state, the repetition is omitted. | 0,76 s | |
| Fixed point about the variables before the loop | Program work of the compiler (`looped`, `Flow.sameBelow`): 102 loops of the compiler always ran all six samples, because variables standing out of the hull (shifted or not yet assigned) in the river level, which get a new number in each run; `sameFacts` therefore never saw two equal stands. Fixed point is also now, if the positions for all variables from before the loop are completely identical, including the reasons in `lost` and `moved`. Then each further run for these variables will again give the same state, and the entries of the torso variables will not affect anything at the loop head; the final run is the same as after all samples. The body test dropped from 542 ms to 225 ms. | 0,46 s | |
| Names, Literals, Release | `Program.lookupOwn` finds a simple type name over `byFull` instead of one run over all types (as before only if several projects declare the same full name); `localById` searches binary. Duration: Literals are program-wide unique (`rt_lit_new`, `RT_INTERNED`), two different literals are compared `rt_equal` without `memcmp`; the lexer gives reserved words and punctuation marks the text as a literal, so that the parser recognizes them on the pointer. `firstBad` counts with a number instead of one tuple per node; `destruct` releases the fields without call per level and field. | 0,44 s | |
| Parser without hopeless attempts | Program work of the compiler (`primary`): Each name in an expression was first attempted as `staticRef` and almost always failed with an exception (22 000 each `diagnose`).If neither `::` nor `<` follows the qualified name, the attempt fails right there with "Expected '::'"; this passage is recorded (`noteFailure`) without making the attempt. The error messages remain the same (`compare.py ast` with the negative tests). | 0,41 s | |
| `StringBuilder` | Only the duration: `appendChar` and `append` with a string attached without intermediate buffer (the lexer attaches each character of a token). | 0,40 s | 0,62 s |

Generator testing: With `RPP_NATCHECK` (C macro), each native read access checks that the value has the expected type; thus, the translated compiler itself and `compare_c.py --cflags -DRPP_NATCHECK` run. `compare_c.py` can use `--exe` to create the translated compiler instead of the interpreter.

Stand (by basic value variables and memory administrators): fixed point of self-translation (round 2 and 3 bytes for byte equal, also with `RPP_NATCHECK`); `compare_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 290 equal, 0 different; `compare.py --exe` 591/1196/1174/690/550/832 equal (tokens, pre, ast, project, defs, diagnose), 0 different; the runtime translates also with gcc.

Remaining profile (according to the memory administrator): about 66% in the generated code, divided into many small methods (frame per call, arguments as field of values, intermediate values with type), about 30% runtime, of which about 15% release and handover (`rt_promote_slow`, `rt_drop_slow`, `destruct`, `rt_temp`, `rt_transfer_var`, `share`). After delivery to static knowledge (profile with callers, `RPP_PROFILE`): The most expensive runtime portions are program work of the compiler, for example `rt_deep_copy` from `IdMap.dup` (copy of `string[]`), counter of the strings in `IdMap.put` and `destruct` when releasing the river levels. Status (according to the built-in methods according to number, 25.11.2026): fixed point of self-translation (round 2 and 3 bytes for byte equal); `compare_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 296 equal, 0 different; `compare.py --exe` 596/1206/1184/696/553/835 equal (tokens, pre, ast, project, defs, diagnosis), 0 different. The comparisons need a freshly built `rppc` (the messages have been in English since 25.11.2026; an older `rppc.exe` gives hundreds of apparent deviations).

Direct calls to built-in methods yielded fewer than expected (about 1% for `diagnose`, 4% for self-translation); the name comparison was not a big post. Next candidate: `rt_temp` by calculations whose result is not a string statically; before a new profile (`RPP_PROFILE`).

Profile (25.11.2026, `diagnose rpp-compiler`, 1272 samples per 1 ms, with callers): `Flow.join` 23 % inclusive, therein `IdMap.put` 15 % (binary search, `insert` and handover of strings: `rt_transfer_var`/`share`/`rt_promote_slow` from `IdMap.put` alone 4 %) and `IdMap.dup` 9 % (`rt_deep_copy` from `string[]`, 7 % own time). Release ()`destruct`, `rt_drop_slow`) 14%, spread over many posts (`probeExpr`, `noteTryState`, `endProbe`, …). `rt_native` and `rt_bm` The self-translation shows the same picture.`Flow.join` 21 %). `join` builds the merger of `lost` and `moved` as a copy plus `put` per entry, i.e. with binary search and insertion per key instead of one pass through two sorted lists (fixed, see `Flow.join` linear in the table).

Status (according to `Flow.join` linear, 25.11.2026): Fixed point of self-translation; `diagnose rpp-compiler` gives the same output as before; `compare_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 296 equal, 0 different; `compare.py --exe` 596/1206/1184/696/553/835 equal, 0 different.

**Proposed target value reached (26.11.2026).** `diagnose rpp-compiler`: translated `rppcc` 0,40 s, `rppc diagnose` (Rust, Release, same day status) 0,47 s; the middle of seven runs on the same computer, MSVC `/O2`. The phases of `rppcc` (`--timings`): Loading 148 ms, declarations 6 ms, fuselages 206 ms; `rppc` takes 54 ms for loading. The proposal from the table of milestones is thus fulfilled.

Where the gain on that day comes from (1.30 s to 0.40 s): 0.28 s from runtime and display of values (steps “objects inline” to “values in whole words”), 0.15 s from Lexer and Parser, 0.44 s from the examiner of the compiler (loops and samples), the rest from the small steps. The work on the examiner is program work that does not have `rppc`: `body.rs` continues to check loops and arguments several times. The same three changes (`looped` on suspicion, `probeArg`, `sameBelow`) could be transferred after `body.rs` and `flow.rs`; `rppc` would then be faster than `rppcc` again, because the generated code per statement is further more expensive than that of Rust (loading: 148 ms against 54 ms).

Status (26.11.2026): Fixed point of self-translation (the compiler generated by the previous status and the one generated by it itself give the same C code for byte); `diagnose rpp-compiler` gives the same output as `rppc`; `compare.py --exe` 609/1232/1210/720/565/847 equal (tokens, pre, ast, project, defs, diagnose), 0 different; `compare_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 316 equal, 0 different; the runtime also translates with gcc.

In addition, the C-runtime now knows the transcendent functions from `rpp.math` (`pow`, `exp`, `log`, `log10`, `sin`, `cos`, `tan`, `sinh`, `cosh`, `tanh`, `hypot`, `atan2`), which were missing since the extension of the standard library; `l36_math_transzendenten` and `l46_math_transzendenten_ergaenzungen` ended in the translated program with “is not built-in”.

Remaining time: loading 37 %, torso test 51 %, the remainder start, output and tidy up at the end of `pruefe`. Of the printout tests, 11 000 are still silent at 40 000 real ones (previously 131 000 at 39 000). Next candidates: token types as a number instead of text in the parser; the attempts `localVarDecl` before each expression statement and `fieldDecl` before each method (one exception each); tuples as a return value without memory request (`capturingFrames`, `callInner`, `literalInfo`: about 130,000 tuples each `diagnose`); `methodsOf` and `fieldOf` without passing through all members and upper types at each access.

**M7 completed (26.11.2026).** Criterion (determined on 26.11.2026): The translated `rppcc` is at least as fast as `rppc` at `diagnose rpp-compiler` (rust, release, status of the same day; middle of seven runs on the same computer) and gives the same output; the fixed point of the self-translation and all comparisons without deviation remain. Stand: 0,40 s to 0,47 s; fixed point, `compare.py --exe` and `compare_c.py --fr 200` The above-mentioned candidates remain open as further optimizations, but are not part of M7.

**M8 decided: `rppcc` as an independent compiler (26.11.2026).** After M7, the translated `rppcc` is faster than `rppc`, but still hangs on Rust: programs translate and start only `compare_c.py`, checks the conformance tests `rppc test`, and the first `rppcc` emerges from `rppc`. For the release mode according to R-14.7 no implementation is yet compliant. M8 closes these gaps:

| Particle | Content |
|---|---|
| `build` and `run` | `rppcc build [--release] PATH` generates C and translates it with `runtime/rpp_rt.c` (MSVC or gcc) to a native program; `rppcc run [--release] PATH -- ARGS` builds and executes. |
| `test` | `rppcc test PATH` Runs the conformance tests as `rppc test` (R-14.8): accept or reject with rule and line, alerts in the marked lines, runtime tests over `build` in both modes with output, exit code and error output. |
| Bootstrap | The compiler's C code, which is created by `rppcc` is the starting point in the repository, plus a building script for Windows and Linux; it is then created `rppcc`, which then translates itself. |
| CI | Bootstrap and `rppcc test` on Windows (MSVC) and Linux (gcc). |

`rppc` remains the reference (R-12.3); `compare.py --exe` and `compare_c.py` continue and must remain unchanged. deferred: faster code for user agents in release mode (such as the differences allowed R-12.5), a JIT for debug mode and the open candidates from M7.

**M8 implemented (26.11.2026).** What came next:

| Particle | Implementation |
|---|---|
| standard library | `rpp.process` with `Process::run` (text inputs and outputs, exit code) and `Process::call` (inherits the console), `Env::executablePath`, `Dir::removeAll` (R-13.43 to R-13.45, standard library section 17). Built-in `stdlib.rs` and `rpp_rt.c`; in C, the streams of `run` run over temporary files so that large outputs without a read thread do not jam. New runtime tests `l48_prozess` and `l49_verzeichnis_entfernen`. |
| `build`, `run` | `rpp-compiler/src/compiler/driver/Toolchain.rpp`: Root over `RPP_HOME` or up from the program path, C compiler over `RPP_CC` (other `cl` or `cc`), switches for MSVC and gcc/clang (also MinGW). |
| `test` | `ConformanceTest.rpp` a transfer of `conftest.rs`: same judgments; runtime tests natively in both modes, the runtime translated once per run. Counter-test with four intentionally false tests (false rule, missing warning mark, wrong output, wrong exit code): same four failures with the same reasons as `rppc test`. |
| Bootstrap | `rpp-compiler/bootstrap/rppcc.c` (C code of the compiler in release mode, 4.9 MB, checked in without line conversion), `bootstrap.sh` and `bootstrap.cmd`: Level 0 from the starting point, then two self-translations; `--check-seed` additionally requires that the starting point is current. |
| CI | new job `self-hosted` in `ci.yml` (Ubuntu and Windows): Bootstrap with `--check-seed`, thereafter `rppcc test`; he neither calls `cargo` nor `rppc`. |

Status (26.11.2026):

- `rppcc test`: Passed 318, 0 failed, 0 open, under Windows with MSVC (79 s) and under Linux with gcc 13.3 in WSL/Ubuntu 24.04 (2 min 10 s). `rppc test ..`: also passed 318.
- Bootstrap: Fixed point on Windows (`bootstrap.cmd`, 77 s) and Linux (`bootstrap.sh`, 1 min 48 s), each with `--check-seed`.
- `compare.py --exe` 614/1242/1220/724/567/849 equal (tokens, pre, ast, project, defs, diagnosis), 0 different; `compare_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 320 equal, 0 different.
- `rppcc run` and `build` tried under Windows (MSVC and MinGW-gcc) and Linux: arguments, exit code and program errors arrive as in `rppc run`.

**M8 completed (26.11.2026).** The first CI run after the push was green: In the job `self-hosted`, the bootstrap reached the fixed point with `--check-seed` Ubuntu 24.04 (gcc) and Windows 2025 (MSVC), the starting point was current, and `rppcc test` passed on both systems 318 of 318 tests (almost 5 minutes for the whole job). The jobs `Tests` also remained green. Not part of M8: macOS (the code for `executablePath` is available, but not translated), concurrent processes and faster code for user agents in release mode.

**M9 decided: Dependencies with versions (26.11.2026).** Programs only connect libraries via `path`. The rules for the central directory (Design Decisions, Section 13b) have been decided since 21.11.2026, but not implemented. M9 implements them without a network service, with a directory in the file system that can be checked in the conformance tests:

| Particle | Content |
|---|---|
| `rpp.hash` | SHA-256 in R++ (standard library roadmap, phase B), used for the checksums in `rpp.lock`. |
| List of lists | Libraries as source code under `besitzer.name/version/` in a folder, selected over `registry` in `rpp.yaml` or an environment variable. |
| Dissolution | `dependencies` with `name` and `version` after Minimal Version Selection; error in various main versions, unknown library or missing version. |
| `rpp.lock` | Exact versions and SHA-256 checksums, checked when building; a modified library is an error; retracted versions continue to build. |
| `rppcc publish` | Compares the exported interface with the previous version, enforces Semantic Versioning and places the library in the local directory. |
| Both compilers | `rppc` and `rppcc` dissolve immediately; `compare.py` receives a part for dissolution. |

Deferred: the directory as a network service with public log of checksums and prebuilt variants, the C connection (candidate for M10, first needs a specification with security model), threads with `rpp.sync` and faster code for user agents in release mode.

**M9 implemented (26.11.2026).** What was added:

| Part | Implementation |
|---|---|
| Specification | Chapter 11, section 5a "The registry in the file system" with R-11.26 to R-11.31: layout `owner.name/VERSION/` with the marker `VERSION.withdrawn`, version form `MAJOR.MINOR.PATCH` (a minimum version may be shorter), selection, checksum, form and checking of `rpp.lock`, no `path` dependencies in the registry. The registry is named by `registry` in `rpp.yaml` (R-11.13), otherwise by `RPP_REGISTRY`. Standard library section 18: `rpp.hash.Sha256` (R-13.46). |
| `rpp.hash` | `Sha256` in R++ (`update`, `updateText`, `finishHex`, `hex`, `hexText`), checked with the FIPS 180-4 test vectors in the runtime test `l50_sha256`. |
| Selection | Minimal Version Selection as a breadth-first search: each requirement points to the smallest version that is not withdrawn, at or above the minimum version, with the same major version; per library the highest version reached wins. Different major versions, unknown libraries, missing versions and cycles in the registry are reported at the requirement. |
| Checksum | SHA-256 over `rpp.yaml` and all `.rpp` files of a version (line endings as LF, sorted by path); stored in `rpp.lock` as `sha256:…`. |
| `rpp.lock` | `rppc lock` and `rppcc lock` write the file, sorted by name and reproducible. Builds use the locked versions, including withdrawn ones; a missing or outdated entry is an error at the requirement, an unneeded entry or a wrong checksum an error in `rpp.lock`. |
| `rppcc publish`, `withdraw` | `publish` checks type, name, version and dependencies, compiles and checks the library, compares the exported interface (lines in the form of `defs`, without positions and private parts) with the next lower version, and requires a new major version for removed or changed lines and a new minor version for new ones. Published versions are never overwritten. `withdraw` creates the marker. |
| Both compilers | `registry.rs` and `project.rs` in `rppc`, `Registry.rpp` and `Project.rpp` in `rppcc`, with the same structure. The output of `project` shows versions and selected libraries with checksums. |
| Tests | 3 new runtime tests (`l02_registry`, `l03_withdrawn`, `l04_locked_withdrawn`) and 10 new rejected tests (`n14` to `n23`) in chapter 11; `compare.py` with a new `lock` stage and 38 edge cases for the registry and `rpp.lock`; `check.py registry` runs through `publish`, `withdraw` and `lock` and is a step in the CI job `self-hosted`. |

Status (26.11.2026, locally on Windows with MSVC):

- `rppc test` and `rppcc test`: 332 passed each, 0 failed, 0 open (chapter 11: 31, chapter 13: 75). `cargo test` and `syntaxtest` green. Chapter 11 covers 29 of 31 rules (2 exempt), chapter 13 covers 43 of 44.
- `rpp.lock`: written twice, byte-for-byte identical, and identical for `rppc lock` and `rppcc lock`; a changed file in a locked library stops the build (`check.py registry`, test `n17_checksum`).
- `compare.py --exe` 659/1332/1310/828/106/581/863 equal (tokens, pre, ast, project, lock, defs, diagnose), 0 different; `compare_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 328 equal, 0 different. `check.py cli` passed.
- Bootstrap with `--check-seed`: fixed point reached, the regenerated seed is current.
- Linux (WSL/Ubuntu 24.04, gcc 13.3): bootstrap with `--check-seed` reaches the fixed point with the same seed, `rppcc test` 332 passed, `check.py registry` passed.

**M9 completed (27.11.2026).** The CI run after the push was green: In the job `self-hosted`, the bootstrap reached the fixed point with `--check-seed` on Ubuntu 24.04 (gcc) and Windows 2025 (MSVC), the starting point was current, `rppcc test` passed 332 of 332 tests on both systems (Chapter 11: 31), and `check.py registry` passed (`publish`, `withdraw`, `lock` and checksums). The jobs `Tests` also remained green. Not part of M9: the registry as a network service, signatures and a public log of checksums.

**Open M7 candidates implemented (27.11.2026).** The four further optimizations named at the end of M7 are done in `rppcc`:

| Candidate | Implementation |
|---|---|
| Token kinds as a number | New enum `Sym` (keywords and punctuation, `Syms::texts` for the text); the lexer gives each such token its `Sym`, the parser compares one enum value instead of kind and text (`isP`, `eatK`, `binaryOp`, `startsExpression` and others). Simple types as a table indexed by `Sym`. The message of the farthest failure is built only in `error()`. |
| Attempts without exceptions | Before an expression statement (and in a `for` initializer), `localVarDecl` is attempted only where it can succeed (a name followed by a name, `<`, `?` or `[` after the qualified name, or `const`, `var`, `own`, `shared`, `weak`, a simple type, `(`); otherwise the same failure is recorded directly. In a class, field and method share type and name; `fieldDecl` is no longer attempted before each method. The reported syntax errors are unchanged (`compare.py` stage `ast`). |
| Tuples as return values | `callInner`, `methodCall`, `staticCall` and `invoke` return the type and leave the facts in a field; `capturingFrames` became `captureFrom` (the end is always `frames.length`); `literalKind` gives the kind of a literal without a tuple. |
| `methodsOf`, `fieldOf` | Index per type (`MemberIndex`, built after `resolveMembers`): first method or field per name, the next method with the same name, the names of the type and all supertypes, the supertypes of a type without type arguments. A name that is not there ends the lookup immediately. |

Status (27.11.2026): `diagnose rppc/std rpp-compiler` 0.415 s against 0.44 s before (median of 15 runs, same computer; `rppc` 0.47 s), same output. Fixed point and seed (`--check-seed`) current; `rppcc test` 332 passed, 0 failed; `compare.py --exe` 659/1332/1310/828/106/581/863 equal, 0 different; `compare_c.py --exe … --cflags -DRPP_NATCHECK --fr 200` 328 equal, 0 different. According to the profile, the parser helpers (`isP`, `eatP`, `eatK`) remain the largest part of the parser; their cost is now the call itself (frame per call).

**M10 decided: C interop (27.11.2026).** Since M9, R++ libraries can be used with versions, but C libraries cannot; what the standard library needs from the system (`sqrt`, `Process`, files) is built into both compilers. M10 implements the decisions from Design Decisions Section 12:

| Part | Content |
|---|---|
| Specification | a chapter of its own, "C Interop", with rules: form of the `extern` class, permitted types and their mapping to C, ownership and borrowing across the boundary, handles and their release, errors when loading and calling, the safety model and its boundary with R-12.6; plus `extern` in `RPP.g4` and in the list of reserved words. |
| Projects | `native: true` in `rpp.yaml`, `allowNative: true` on dependencies (including indirect ones), shown in `project` and `rpp.lock`; the check is the same in both compilers. |
| `rppc` | checking of `extern` classes; the interpreter loads the library with `libloading` and calls via `libffi`. |
| `rppcc` | the same checks; the C generator emits prototypes and direct calls, `build` links the library (gcc: `-l`, MSVC: `.lib`). |
| Tests | a small C test library in the repository that the test run compiles first; accepted, rejected and runtime tests for every rule; `compare.py` covers the new parts. |

M10 is complete when the chapter is written, the new conformance tests pass with `rppc test` and `rppcc test` in both modes, `compare.py` shows no deviation, the fixed point holds and the CI run on Ubuntu and Windows is green.

Deferred: callbacks (R++ lambdas as C function pointers), structs by value, pointers in R++, variadic functions, C source code in registry libraries and moving built-in parts of the standard library to `extern` classes.
