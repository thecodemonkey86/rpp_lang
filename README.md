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
- `own` denotes exclusive ownership, `shared` denotes shared ownership, and `weak` is a non-owning reference to a shared object. Use `ref` and `ref mut` for read-only and mutable borrows. Passing an object to an `own` parameter moves it. [Ownership rules](R%2B%2B%20Ownership-Kernkalk%C3%BCl.md)
- `T[]` is a growable vector. `using rpp.io.Console;` imports a type, and `Console::write("text")` calls a static method. The bundled standard library needs no dependency entries. [Standard library](R%2B%2B%20Standardbibliothek.md)

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

A reusable library uses `type: library` and omits `main`. Its package names start with the library name, and other projects can use types marked `export`. The current tools load local dependencies through `dependencies` entries with `name` and `path`. [Local library example](konformitaet/kap11_projekte/akzeptiert/a04_bibliothek), [package and project specification](R%2B%2B%20Pakete%20und%20Projekte.md)

## Build a native executable

Run the R++ compiler through `rppc` to generate C. The arguments to `c` are the standard library directory, a source file or project directory, and the output file.

```sh
cargo run --manifest-path rppc/Cargo.toml -- run rpp-compiler -- c --release rppc/std hello hello.c
```

Compile the generated file and the bundled C runtime with a C17 toolchain.

**GCC on Linux**

```sh
gcc -std=c17 -D_POSIX_C_SOURCE=200809L -O2 -pthread -I rpp-compiler/runtime hello.c rpp-compiler/runtime/rpp_rt.c -lm -o hello-app
./hello-app Ada
```

**MSVC on Windows**, from a Visual Studio Developer Command Prompt

```bat
cl /nologo /std:c17 /O2 /I rpp-compiler\runtime hello.c rpp-compiler\runtime\rpp_rt.c /Fe:hello.exe /link /STACK:16777216
hello.exe Ada
```

The `--release` flag after `c` selects R++ release semantics. Omit it to generate C with R++ debug semantics. For interpreter runs, use `rppc run --release` to exercise release semantics. An already built `rppcc` binary accepts the same `c --release rppc/std hello hello.c` arguments directly. [Compiler and bootstrap details](rpp-compiler/README.md)

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

Use `rppc --help` or `rppcc --help` to list the commands. With `rppcc diagnose --timings STD PATH`, the compiler also reports how long project loading and each checking phase took. Existing scripts can still use the legacy German spellings.

`PATH` is a `.rpp` file or a directory containing `rpp.yaml`. `STD` is `rppc/std` when running from the repository root. Compiler diagnostics are in English and generally include a source location and a specification rule such as `R-6.2`. [Reference CLI details](rppc/README.md)

## Work on the compiler

Run the Rust unit tests, syntax suite, and language conformance suite from the repository root.

```sh
cargo test --workspace --manifest-path rppc/Cargo.toml
cargo run --manifest-path rppc/Cargo.toml -- syntaxtest .
cargo run --manifest-path rppc/Cargo.toml -- test .
```

The conformance suite checks accepted and rejected programs, expected warnings, and runtime results. Runtime cases exercise the interpreter in both language modes and with ownership invariant checks enabled.

The [CI workflow](.github/workflows/ci.yml) runs on Linux and Windows for pull requests and pushes to `master`. It also checks the English command-line output and compares native executables built with GCC or MSVC against the interpreter. After a debug build, use `python .github/scripts/check.py cli` and `python .github/scripts/check.py native` to run those checks locally. Run the native check from a Developer Command Prompt on Windows.

The separate [Compiler bootstrap workflow](.github/workflows/bootstrap.yml) is run manually from the Actions tab. It builds two native `rppcc` generations and verifies that the second and third C output files are byte-for-byte identical. To run it locally, first build `rppc` with `cargo build --locked --release --manifest-path rppc/Cargo.toml`, then run `python .github/scripts/check.py bootstrap` with the same C compiler prerequisites.

| Where to look | What you will find |
| --- | --- |
| [`rppc/`](rppc/) | Rust reference implementation and [standard library source](rppc/std/rpp/). |
| [`rpp-compiler/`](rpp-compiler/) | Self-hosting compiler, C runtime, and Python scripts that compare diagnostics and native execution against the reference implementation. |
| [`rpp-grammatik/`](rpp-grammatik/README.md) | Normative ANTLR grammar, parser tests, and differential testing against the Rust parser. |
| [`konformitaet/`](konformitaet/) | Small R++ examples organized by specification chapter. Start with the `akzeptiert` (accepted) and `laufzeit` (runtime) directories. |
| [`fr-modell/`](fr-modell/README.md) | Executable model of the ownership rules, with its own tests and comparison tools. |

Most detailed documentation is currently in German. See the [specification index](R%2B%2B%20Spezifikationsger%C3%BCst.md), the [conformance test rules](R%2B%2B%20Konformit%C3%A4tstests.md), and the [implementation plan](R%2B%2B%20Implementierungsplan.md).

## License

[MIT License](LICENSE)
