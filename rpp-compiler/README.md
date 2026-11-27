# rpp-compiler — the self-hosting R++ compiler

[Deutsch](README.de.md)

Milestone M6 developed the compiler in R++ and compared it against the Rust implementation until it could compile itself. [Implementation plan](../R%2B%2B%20Implementation%20Plan.md)

As of 22 November 2026, the compiler covered lexical analysis, preprocessing, parsing, project loading, declarations, and body checking with flow analysis. It also offered a C code generator through the `c` command.

## Contents

| Path | Purpose |
|---|---|
| `rpp.yaml` | Manifest for the `rppcc` executable |
| `src/compiler/lexer/` | Token species, tokens and lexer equivalent to `rppc/crates/rpp-syntax/src/lexer.rs` |
| `src/compiler/pre/` | conditional compilation (R-11.23 to R-11.25), equivalent to `preprocess.rs` |
| `src/compiler/syntax/Node.rpp` | Node of the syntax tree (kind, text, position, children) and its text form |
| `src/compiler/syntax/Parser.rpp` | Parser (recursive descent, precedence parser, reset via an exception), equivalent to `parser.rs`, with the checking of the expression statements (R-8.12) |
| `src/compiler/syntax/Source.rpp` | read a source file (preprocessor, lexer, parser, R-8.12); the first error is as `SyntaxError` |
| `src/compiler/project/Yaml.rpp` | the YAML part of R-11.13 equivalent to `yaml.rs` |
| `src/compiler/project/Project.rpp` | Projects, source folders, local dependencies, standard library (R-11.13, R-11.15 to R-11.17), equivalent to `project.rs` |
| `src/compiler/sema/Ty.rpp` | Immutable semantic types represented as `shared Ty`, equivalent to `ty.rs` |
| `src/compiler/sema/Defs.rpp` | Declaration tables: types, fields, methods, constructors, variants |
| `src/compiler/sema/Program.rpp` | Declaration phase: collect types, resolve names, member tables, constraints, file and class level rules; equivalent to `Program::build` in `defs.rs` |
| `src/compiler/sema/Flow.rpp` | Flow state for type narrowing, initialization, moves, places, and loans, equivalent to `flow.rs` |
| `src/compiler/sema/Checker.rpp` | Checks statements, expressions, calls, lambdas, and built-ins. Tracks flow and records execution data. Corresponds to `body.rs`. |
| `src/compiler/sema/Rt.rpp` | Execution data per node (such as `rt.rs`): types, target types, moves, call targets, lambda captures, base constructors, declaration types |
| `src/compiler/gen/CGen.rpp` | C generator that follows the evaluation steps of the `rpp-interp` interpreter |
| `runtime/rpp_rt.h`, `runtime/rpp_rt.c` | Runtime for the generated code (C17, MSVC and gcc): values with ownership mode such as `value.rs`, release, exceptions and program errors, numbers of both modes, strings, vectors, built-in parts of the standard library |
| `src/main.rpp` | Commands `tokens`, `pre`, `ast`, `project`, `defs`, `diagnose`, `c` |
| `compare.py` | Comparison with `rppc` |
| `compare_c.py`, `tests_c/` | Comparison of translated programs with `rppc run` |

The syntax tree uses a uniform node structure. The Rust implementation builds the corresponding text representation in `rppc/crates/rpp-syntax/src/dump.rs`. Each line gives a node's kind, text, and position.

## Run the inspection commands

```
cd ../rppc
cargo run -- run ../rpp-compiler -- tokens FILE.rpp
cargo run -- run ../rpp-compiler -- pre debug,linux FILE.rpp
cargo run -- run ../rpp-compiler -- ast debug,linux FILE.rpp
cargo run -- run ../rpp-compiler -- project [--release] STD PATH
cargo run -- run ../rpp-compiler -- defs STD PATH
cargo run -- run ../rpp-compiler -- diagnose [--timings] STD PATH
cargo run -- run ../rpp-compiler -- c [--release] STD PATH OUTPUT.c
cargo run -- tokens FILE.rpp          # The same output from rppc
cargo run -- pre debug,linux FILE.rpp
cargo run -- ast debug,linux FILE.rpp
cargo run -- project [--release] std PFAD
cargo run -- defs std PFAD
cargo run -- diagnose std PFAD
```

## Comparison

`rppcc diagnose --timings STD PATH` outputs the duration of project loading, declaration checking and body checking in three lines on the error output; the values are whole milliseconds; sorting and output of the diagnoses are outside the measurement. Without `--timings` only the regular diagnoses appear.

`rppcc project --help` and `rppcc tokens --help` show the respective help on the standard output (exit code 0); in the case of missing or excess arguments and unknown options, message and call appear on the error output (exit code 2). With `rppcc tokens -- -beispiel.rpp` file names with leading minus can also be transferred.

`project` displays main and source files relative to the directory of their respective project. File positions in messages from `project`, `defs` and `diagnose` refer to the input project, for individual files to their directory. For files outside this base, the absolute path appears.

```
py -X utf8 compare.py                # Run all stages. -v prints each case; tokens|pre|ast|project|lock|defs|diagnose selects stages; --fr N adds N random FR++ programs.
```

Entries are all `.rpp` files of the project (conformance tests, grammar tests including negative tests and readings, standard library, `rpp-compiler` itself) and border cases per part; `pre` and `ast` run with the sets of symbols `debug,linux` and `release,windows`. `project` loads all project directories and individual files of the conformance tests, `rpp-compiler` itself and 39 border cases for `rpp.yaml` in debug and release mode; the output lists projects, files with declared and expected package and type names as well as the messages. `defs` runs on the same inputs, the grammar tests and 17 limit cases for declaration rules; the output lists all types with their members and signatures and the declaration phase messages. `diagnose` compares all messages of the entire check via the same inputs without `rpp-compiler` itself (too slowly in the interpreter) and via the FR++ programs translated to R++.

As of 22 November 2026, the compiler covered lexical analysis, preprocessing, parsing, project loading, declarations, and body checking with flow analysis. It also offered a C code generator through the `c` command.

| Command | Matches | Mismatches | Control run |
|---|---|---|---|
| `tokens` | 521 | 0 | without `??` in the punctuation list: 18 deviations |
| `pre` | 1056 | 0 | – |
| `ast` (tree or first error with position) | 1036 | 0 | `+`/`-` stronger than `*`: 10 deviations |
| `project` | 592 | 0 | `name:app` Accepted without spaces: 1 deviation |
| `defs` | 501 | 0 | `protected` with rank of `public`: 1 deviation |
| `diagnose` | 782 | 0 | R-6.9 instead of R-6.8 after `if`: 1 deviation |

When building the parser, an error in the interpreter was noticed and fixed: A `own` argument that came via `??`, `?:` or brackets was borrowed instead of moved (new runtime test `ch05_ownership/runtime/l02_own_argument_expression.rpp`).

## C code generator

`rppcc c [--release] STD PATH OUTPUT.c` checks the program including the standard library and generates a C file; it is translated together with `runtime/rpp_rt.c`:

```
cl /std:c17 /O2 /I runtime OUTPUT.c runtime/rpp_rt.c /link /STACK:16777216
gcc -std=c17 -O2 -I runtime OUTPUT.c runtime/rpp_rt.c -lm -ldl
```

The code generator follows the debug interpreter of `rppc`, the reference to R-12.3: each expression becomes C-code that performs the same steps in the same order (`eval`, `eval_store`, `transfer`, `temp`, `convert`, `args`). A value (`rv`) bears its kind in the interpreter as `Value` and the ownership mode in objects at the reference; release, destructors (V-7), locks (V-1), unwinding with `finally`, program errors, stacktrace and exit codes (R-12.9) behave as there. Errors run as status through the invocations; each statement and scope has a cleanup mark. Generics are not duplicated; type arguments stand on the object as in the interpreter (R-7.26). The mode (debug or release) is selected when generating.

The information which the interpreter reads from `Program::rt` is collected `Checker.rpp` in the same places as `body.rs` (`Rt.rpp`); the nodes of the syntax tree are numbered for this (`Node.id`).

Comparison: `py -X utf8 compare_c.py [--fr N] [--only-fr] [--gcc] [--exe RPPCC] [--cflags "…"] [PATTERN]` creates, translates (MSVC; with `--gcc` gcc) and starts each runtime test of the conformance tests and each program in `tests_c/` in both modes in a fresh working directory and compares output, error output and exit code with `rppc run`; with `--fr N` also the FR++ programs translated to R++. `--only-fr` limits the comparison to FR++ programs.

Unicode tables (`runtime/rpp_unicode.h`, `rpp_unicode_ign.h`) are created with `runtime/gen_unicode.rs` from the standard library of Rust and with `gen_unicode_ignorable.py`; thus `char` and strings behave as in the interpreter. Texts of operating system errors in `IOException` come from the C library (not specified after V-13.12). Paths and command line arguments are UTF-8; under Windows, file accesses go through the `wchar_t` versions.

With `--exe` a translated `rppcc` generates the C code instead of the interpreter; `--cflags "-DRPP_NATCHECK"` translates so that each native read access of the generator (M7) checks the nature of the value.

Native base values (M7): Pure expressions above `i8`–`i64`, `char` and `bool` without invocations are evaluated directly by the generator in C as `int64_t`; the rapid case is in `rpp_rt.h` (`rn_…`), all other cases go through the rv paths as in the interpreter. Local variables and parameters of these types are C variables (C)`int64_t x…`, except when it controls a lambda; or `take`/`replace` It needs a tuple assignment as a place. Small blocks manage the runtime themselves (free lists per size class), `xmalloc`/`xfree`).

Runtime environment variables: `RPP_NOFREE=1` does not release memory (to limit sharing errors); `RPP_PROFILE=FILE` writes the command counter and up to seven callers as addresses relative to the module each millisecond, one sample per row (Windows; resolve with the linker’s map file, `/MAP`); `RPP_MAINTHREAD=1` runs without its own thread with a large stack.

Speed (26.11.2026, translated `rppcc.exe`, MSVC `/O2`): `diagnose rpp-compiler` 0,40 s, self-compilation 0,62 s (25.11.2026: 1,4 s and 1,7 s; 22.11.2026: 9,2 s and 10,4 s); `rppc diagnose` (release) 0,47 s. The individual steps are set out in the implementation plan, Section 6.

## Build, run, and test native programs (M8)

`rppcc` builds native programs themselves; Rust is not needed for this:

```
rppcc build [--release] [-o OUTPUT] PATH      # Check, generate C, and invoke the C compiler
rppcc run [--release] PATH [-- ARGS…]          # Build in a temporary directory and run
rppcc test [-v] [--check-only] [ROOT]           # Run the conformance suite
```

The parts are in `src/compiler/driver/`: `Toolchain.rpp` search the root (the directory with `rppc/std` and `rpp-compiler/runtime`: `RPP_HOME`, otherwise up from the current `rppcc` directory) and call the C compiler (`RPP_CC`, otherwise `cl` under Windows and `cc` otherwise; for `cl` apply the switches of MSVC, otherwise those of gcc and clang). `build` without `-o` places the program in the current directory, named after the file or the project directory. `--release` selects the R++ semantics of the release mode and translates the C with `/O2` or `-O2` otherwise with `/O1` or `-O1`. `run` the program starts with `Process::call`; it inherits the console, and its exit code becomes that of `rppcc`.

`ConformanceTest.rpp` follows `rppc/crates/rppc/src/conftest.rs`: Accept or reject the compiler checks in the same process (debug mode), with the same rules for error lines, expected rule and warnings. Runtime tests are generated in both modes, bound to the runtime once translated and executed in a fresh, empty working directory (V-14.3); output, exit code and error output are compared. The output has the same form as that of `rppc test`. A whole run takes a good 80 s under Windows (MSVC), a good 2 minutes under Linux (gcc, WSL).

The standard library gained `rpp.process` (`Process::run`, `Process::call`), `Env::executablePath`, and `Dir::removeAll` to support this workflow (R-13.43 through R-13.45).

## Dependencies from a registry (M9)

Dependencies with `version` come from a registry in the file system (chapter 11, section 5a): `REGISTRY/owner.name/VERSION/` is a library project, and `VERSION.withdrawn` next to it marks it as withdrawn. The program's `rpp.yaml` names the registry with `registry`; otherwise `RPP_REGISTRY` applies.

```
rppcc lock [--stdout] PROJECT                  # select and write rpp.lock (like rppc lock)
rppcc publish [--registry DIR] PROJECT         # check the library and its version, copy it into the registry
rppcc withdraw [--registry DIR] NAME VERSION   # mark a version as withdrawn
```

The parts: `src/compiler/project/Registry.rpp` (versions, checksums, form of `rpp.lock`; counterpart `rppc/crates/rpp-sema/src/registry.rs`) and `Workspace.resolve` in `Project.rpp` (Minimal Version Selection, checking against `rpp.lock`, loading the selected versions; counterpart `project.rs`). The checksums are computed by `rpp.hash.Sha256` from the standard library (R-13.46), written in R++. `publish` compares the exported interface with the previous version in the form of `defs` (exported types, their `public` and `protected` parts, without positions); removed or changed lines require a new major version, new lines a new minor version.

`compare.py` checks resolution in the stages `project` (projects with versions, selected libraries with checksums, diagnostics; plus 38 edge cases for the registry and `rpp.lock`) and `lock` (text of `lock --stdout`). `.github/scripts/check.py registry` runs `publish`, `withdraw` and `lock` once end to end.

## C interop (M10)

`extern("lib") class` and `extern("lib") handle` (chapter 15) are checked in `Program.rpp` and `Checker.rpp` like in `rppc` (`defs.rs`, `body.rs`); the project entries `native`, `nativePaths` and `allowNative` are in `Project.rpp`, the line `native: true` of `rpp.lock` in `Registry.rpp`.

`CGen.rpp` generates one wrapper per `extern` method. It converts the arguments (strings as UTF-8 with a terminating NUL, `byte[]` as pointer and length, handles as their C pointer), flushes the program's output (R-15.11), calls the C function and converts the result back. A handle object carries its pointer; its destructor calls the release function once.

- Debug mode: the program looks up every function through a pointer that `rt_native_load` fills before `main` from the libraries (search: `nativePaths` of the declaring project, then of the program, then the system). A missing library or symbol ends the program with `NativeError` and exit code 2.
- Release mode: the C file declares a prototype per symbol and keeps all of them in the table `rpp_native_symbols`, so the linker reports a missing symbol even if the program never calls it (R-15.12). `build` links the libraries: MSVC with `/LIBPATH` and `NAME.lib`, gcc and clang with `-L`, `-l` and `-Wl,-rpath`. Under Windows, the DLL is copied next to the program.

Outside Windows the runtime uses `dlopen`, so every program links `-ldl`. C functions must not use names starting with `rt_` or `rpp_`, which belong to the runtime. `rppcc test` compiles the test library `conformance/ch15_c_interop/native/rpptest.c` like `rppc test` does (`Toolchain.sharedLib`).

## Bootstrap without Rust

`bootstrap/rppcc.c` is the C code of the compiler itself, generated with `rppcc c --release rppc/std rpp-compiler bootstrap/rppcc.c` (start point, checked in without line conversion).

```
sh rpp-compiler/bootstrap/bootstrap.sh [--check-seed]       # GCC or Clang ($CC, defaults to cc)
rpp-compiler\bootstrap\bootstrap.cmd [--check-seed]          # MSVC (looked up with vswhere as needed)
```

Stage 0 compiles the seed and uses it to generate the compiler from source (`gen1.c`). Stage 1 compiles `gen1.c` and generates the compiler again (`gen2.c`). The build reaches a fixed point when the two C files match byte for byte. The CI option `--check-seed` also checks that the checked-in seed matches. After changing the compiler or standard library, refresh the seed from the repository root.

```
rpp-compiler/build/rppcc c --release rppc/std rpp-compiler rpp-compiler/bootstrap/rppcc.c
```

The result is `rpp-compiler/build/rppcc` (`rppcc.exe`); the directory `build/` is not checked in; the starting point is the same on all platforms: on Windows (MSVC) and Linux (gcc), the compiler generates the same C code from the same source code.
