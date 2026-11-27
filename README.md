# R++

R++ is a statically typed, object-oriented programming language with C-family syntax. It uses ownership and borrowing to manage object lifetimes without lifetime annotations, and distinguishes nullable types such as `string?` from non-nullable types such as `string`.

Use **`rppc`**, the Rust reference implementation, to check and run R++ programs. Use **`rppcc`**, the self-hosting compiler written in R++, to generate C and build native executables. The language specification is a draft, and the native backend is under active development.

## Run your first program

Install Rust and Cargo. The Rust workspace has no third-party crate dependencies. Run the commands below from the repository root.

```sh
git clone https://github.com/thecodemonkey86/rpp_lang.git
cd rpp_lang
cargo build --manifest-path rppc/Cargo.toml
```

Create `hello/src/main.rpp` as a UTF-8 file.

```cpp
i32 main(string[] args) {
    string? name = args.length > 0 ? args[0] : null;
    print("Hello, " + (name ?? "world") + "!");
    return 0;
}
```

`rppc run` starts immediately. `Console::readLine()` reads the next line from the terminal, a pipe, or a redirected file and returns `null` at EOF.

```sh
cargo run --manifest-path rppc/Cargo.toml -- check hello/src/main.rpp
cargo run --manifest-path rppc/Cargo.toml -- run hello/src/main.rpp -- Ada
```

The second command prints `Hello, Ada!`. Omit `-- Ada` to print `Hello, world!`. The first `--` separates Cargo options from the `rppc` command. The second passes arguments to the R++ program.

### Read R++ code

- An executable starts at `i32 main(string[] args)`. Its return value is the process exit code. `print` writes a line to standard output.
- `T?` allows `null`. An `if (value != null)` check can narrow a nullable value within that branch. Use `value?.member` for optional access and `value ?? fallback` to supply a default.
- `own` denotes exclusive ownership, `shared` denotes shared ownership, and `weak` is a non-owning reference to a shared object. Use `ref` and `ref mut` for read-only and mutable borrows. Passing an object to an `own` parameter moves it. [Ownership rules](R%2B%2B%20Ownership%20Core%20Calculus.md)
- `T[]` is a growable vector. `using rpp.io.Console;` imports a type, and `Console::write("text")` calls a static method. The bundled standard library needs no dependency entries. [Standard library](R%2B%2B%20Standard%20Library.md)

## Create a project

When your program spans multiple files, create `hello/rpp.yaml` next to `hello/src/`.

```yaml
name: hello
version: 0.1.0
type: executable
main: src/main.rpp
sources:
  - src
```

Pass the project directory to the tool.

```sh
cargo run --manifest-path rppc/Cargo.toml -- check hello
cargo run --manifest-path rppc/Cargo.toml -- run hello -- Ada
```

Paths in the manifest are relative to the project directory. A file under `src/demo/util/` starts with `package demo.util;`. Files directly under `src/` belong to the root package and omit the package declaration.

A reusable library uses `type: library` and omits `main`. Its package names start with the library name, and other projects can use types marked `export`. A local dependency is a `dependencies` entry with `name` and `path`. [Local library example](conformance/ch11_projects/accepted/a04_library), [package and project specification](R%2B%2B%20Packages%20and%20Projects.md)

### Use versioned libraries from a registry

A dependency with `version` comes from a registry. Until the central registry service exists, a registry is a directory: version `1.4.0` of `acme.json` lives in `REGISTRY/acme.json/1.4.0/` as a library project. Name the directory with `registry`, relative to the project, or set `RPP_REGISTRY`.

```yaml
registry: ../registry
dependencies:
  - name: acme.json
    version: "1.4"      # at least 1.4.0, same major version
```

Each library is used in one version per program: the highest minimum version that any dependency asks for (minimal version selection). Versions marked as withdrawn are skipped. `lock` writes `rpp.lock` with the exact versions and SHA-256 checksums; with a lock file, builds use the locked versions and report a library whose files have changed.

```sh
rpp-compiler/build/rppcc lock hello                                  # or: rppc lock hello
rpp-compiler/build/rppcc publish --registry registry acme-json       # copy a library into the registry
rpp-compiler/build/rppcc withdraw --registry registry acme.json 1.4.0
```

`publish` checks the library, refuses to overwrite a published version, and compares the exported interface with the previous version: removed or changed parts need a new major version, new parts a new minor version. [Registry rules](R%2B%2B%20Packages%20and%20Projects.md#5a-the-registry-in-the-file-system), [example with a lock file](conformance/ch11_projects/runtime/l02_registry)

## Build a native executable

`rppcc` builds native executables through C. You do not need Rust for this: the repository contains the compiler's own generated C code as a seed in [`rpp-compiler/bootstrap/rppcc.c`](rpp-compiler/bootstrap/). The bootstrap script compiles the seed with a C17 toolchain, lets the result compile `rppcc` from source twice, and checks that both generations produce the same C code.

**GCC or Clang on Linux**

```sh
sh rpp-compiler/bootstrap/bootstrap.sh
```

**MSVC on Windows.** The script finds Visual Studio's C++ build tools if `cl` is not already on `PATH`.

```bat
rpp-compiler\bootstrap\bootstrap.cmd
```

Both scripts write `rpp-compiler/build/rppcc` (`rppcc.exe` on Windows). The binary finds the standard library and the C runtime in the checkout above it, so keep it there or set `RPP_HOME` to the repository root. Then build or run a program:

```sh
rpp-compiler/build/rppcc run hello -- Ada
rpp-compiler/build/rppcc build --release -o hello-app hello
./hello-app Ada
```

`rppcc` calls `cc` on Linux and `cl` on Windows. On Windows, run it from a Developer Command Prompt so that `cl` is on `PATH`. Set `RPP_CC` to choose another C compiler, such as `gcc` or `clang`. `--release` selects R++ release semantics and optimizes the C code; without it, the program has R++ debug semantics. Use `rppc run --release` to try release semantics in the interpreter.

To generate the C file yourself, use `rppcc c [--release] rppc/std hello hello.c` and compile it with the runtime in `rpp-compiler/runtime/rpp_rt.c`. [Compiler, runtime, and bootstrap details](rpp-compiler/README.md)

## Tool commands

The forms below use `rppc` as shorthand for `cargo run --manifest-path rppc/Cargo.toml --` from the repository root. You can also run the binary under `rppc/target/debug/` directly. Keep the checkout available because the binary uses the bundled standard library from the source tree.

| Command | Purpose |
| --- | --- |
| `rppc check PATH` | Check a source file or project without running it. Use this for an exit status that reflects compilation errors. |
| `rppc run [--release] [--check-invariants] PATH [-- ARGS…]` | Run a source file or project in the interpreter. `--check-invariants` enables extra ownership invariant checks. |
| `rppc parse [--symbols LIST] FILE` | Check syntax only. `LIST` is a comma-separated set such as `debug,linux`. |
| `rppc tokens FILE` | Dump lexer tokens. |
| `rppc pre SYMBOLS FILE` / `rppc ast SYMBOLS FILE` | Show preprocessed source or the syntax tree for the given symbols. |
| `rppc project [--release] STD PATH` / `rppc defs [--release] STD PATH` | Inspect project loading or declaration tables. |
| `rppc diagnose STD PATH` | Print diagnostics in the format used for comparison with `rppcc`. |
| `rppc lock [--stdout] PROJECT` | Resolve the registry dependencies and write `rpp.lock` (or print it). |

The self-hosting compiler `rppcc` (built by the bootstrap script) accepts the inspection commands above with the same arguments and adds these:

| Command | Purpose |
| --- | --- |
| `rppcc build [--release] [-o OUTPUT] PATH` | Check a program, generate C, and compile it to a native executable. |
| `rppcc run [--release] PATH [-- ARGS…]` | Build into a temporary directory and run the executable with the console attached. |
| `rppcc test [-v] [--check-only] [ROOT]` | Run the conformance suite like `rppc test`; runtime cases are built natively and run in debug and release mode. `--check-only` skips them. |
| `rppcc c [--release] STD PATH OUTPUT.c` | Generate C without compiling it. |
| `rppcc publish [--registry DIR] PROJECT` | Publish a library to a registry directory after checking it and its version number. |
| `rppcc withdraw [--registry DIR] NAME VERSION` | Mark a published version as withdrawn. |

Use `rppc --help` or `rppcc --help` to list the commands. With `rppcc diagnose --timings STD PATH`, the compiler also reports how long project loading and each checking phase took. Existing scripts can still use the legacy German spellings.

`PATH` is a `.rpp` file or a directory containing `rpp.yaml`. `STD` is `rppc/std` when running from the repository root. Compiler diagnostics are in English and generally include a source location and a specification rule such as `R-6.2`. [Reference CLI details](rppc/README.md)

## Work on the compiler

Run the Rust unit tests, syntax suite, and language conformance suite from the repository root.

```sh
cargo test --workspace --manifest-path rppc/Cargo.toml
cargo run --manifest-path rppc/Cargo.toml -- syntaxtest .
cargo run --manifest-path rppc/Cargo.toml -- test .
```

The conformance suite checks accepted and rejected programs, expected warnings, and runtime results. Runtime cases exercise the interpreter in both language modes and with ownership invariant checks enabled. `rpp-compiler/build/rppcc test` runs the same suite with native executables.

After changing the compiler or the standard library, refresh the bootstrap seed so that it matches the source again:

```sh
rpp-compiler/build/rppcc c --release rppc/std rpp-compiler rpp-compiler/bootstrap/rppcc.c
```

The [CI workflow](.github/workflows/ci.yml) runs on Linux and Windows for pull requests and pushes to `master`. It also checks the English command-line output and compares native executables built with GCC or MSVC against the interpreter. A separate job uses no Rust: it bootstraps `rppcc` from the C seed, checks the fixpoint and that the seed is current, and runs `rppcc test`. The same job checks `publish`, `withdraw` and `rpp.lock` with `python .github/scripts/check.py registry`. After a debug build, use `python .github/scripts/check.py cli` and `python .github/scripts/check.py native` to run those checks locally. Run the native check from a Developer Command Prompt on Windows.

The separate [Compiler bootstrap workflow](.github/workflows/bootstrap.yml) is run manually from the Actions tab. It builds two native `rppcc` generations and verifies that the second and third C output files are byte-for-byte identical. To run it locally, first build `rppc` with `cargo build --locked --release --manifest-path rppc/Cargo.toml`, then run `python .github/scripts/check.py bootstrap` with the same C compiler prerequisites.

| Where to look | What you will find |
| --- | --- |
| [`rppc/`](rppc/) | Rust reference implementation and [standard library source](rppc/std/rpp/). |
| [`rpp-compiler/`](rpp-compiler/) | Self-hosting compiler, C runtime, and Python scripts that compare diagnostics and native execution against the reference implementation. |
| [`rpp-grammar/`](rpp-grammar/README.md) | Normative ANTLR grammar, parser tests, and differential testing against the Rust parser. |
| [`conformance/`](conformance/) | Small R++ examples organized by specification chapter. Start with the `accepted` and `runtime` directories. |
| [`fr-model/`](fr-model/README.md) | Executable model of the ownership rules, with its own tests and comparison tools. |

## Documentation

See the [language specification](R%2B%2B%20Specification%20Outline.md), the [conformance test rules](R%2B%2B%20Conformance%20Tests.md), and the [implementation plan](R%2B%2B%20Implementation%20Plan.md). Each translated document links to its German edition.

Die deutschen Sprachreferenzen sind in der [Spezifikationsübersicht](R%2B%2B%20Specification%20Outline.de.md) verlinkt.

## License

[MIT License](LICENSE)
