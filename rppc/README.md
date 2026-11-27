# rppc — the Rust reference implementation

[Deutsch](README.de.md)

The `rppc` workspace contains the R++ parser, semantic checker, interpreter, and standard library. Milestones M1–M5 covered syntax, names and types, flow analysis checked against FR++, the interpreter and its invariant checks, and the standard library with runtime type arguments. [Implementation plan](../R%2B%2B%20Implementation%20Plan.md)

## Contents

| Path | Purpose |
|---|---|
| `crates/rpp-syntax/src/preprocess.rs` | conditional compilation (`#if` ... `#endif`), such as `rpp-grammar/tools/preprocess.py` |
| `crates/rpp-syntax/src/lexer.rs` | Lexer under the Lexer Rules of `RPP.g4` |
| `crates/rpp-syntax/src/ast.rs` | Abstract syntax tree |
| `crates/rpp-syntax/src/parser.rs` | Recursive descent, expression precedence, and backtracking where ANTLR uses lookahead |
| `crates/rpp-syntax/src/lib.rs` | Parser entry point `rpp_syntax::parse`, expression statement checks (R-8.12), and precedence tests |
| `crates/rpp-sema/src/` | Project loading (`project.rs`, `yaml.rs`), declarations and class rules (`defs.rs`), types (`ty.rs`), body checking (`body.rs`), and flow state, places, and loans (`flow.rs`) |
| `crates/rpp-sema/src/rt.rs` | Semantic checker data consumed by the interpreter through `Program::rt` |
| `crates/rpp-interp/src/` | Debug interpreters: Values and ownership modes (`value.rs`), execution, release, exceptions, locks (`lib.rs`) |
| `std/rpp/…` | Standard library as R++ source text (chapter 13); built-in parts execute the methods with `fail("eingebaut")` in the interpreter (`crates/rpp-interp/src/stdlib.rs`), `rpp.random`, `rpp.collections.Vectors`, `rpp.collections.Deque`, `rpp.text.Text` and the rounding part of `rpp.math` are pure R++ source text |
| `crates/rppc/src/conftest.rs` | Semantic Conformance test run `rppc test` |
| `crates/rppc/src/main.rs` | Command line |
| `crates/rppc/src/syntaxtest.rs` | Syntax test run with the same cases as `rpp-grammar/tools/run_tests.py` |
| [Syntax test report](test-report.txt) · [Deutsch](test-report.de.txt) | Historical syntax test output |
| [Conformance test report](test-run.txt) · [Deutsch](test-run.de.txt) | Historical semantic test output |

## Building and testing

Prerequisite: Rust (tested with 1.83).

```
cargo build
cargo test
cargo run -- syntaxtest ..
```

`syntaxtest` reads from the project directory (`..`):

- `rpp-grammar/tests/pos/*.rpp` – must be accepted, partly with several sets of symbols
- `rpp-grammar/tests/neg/*.rpp` – Must be rejected
- `rpp-grammar/tests/parse_probes/*.rpp` – the first statement must be read as the specified grammar rule
- `conformance/**` – must be syntactically valid; a rejected test must fail only in the line marked with `FEHLER`

Parse or check individual files:

```
cargo run -- parse --symbols debug,linux FILE.rpp
cargo run -- check PATH
```

Run the conformance suite. `-v` prints the result of each test.

```
cargo run -- test ..
```

Runtime tests are executed in both modes and in the test mode (output, exit code, error output). Each runtime test runs in a fresh, empty working directory (V-14.3).

Use a source file or project directory for `PATH`. In a `run` command, arguments after the second `--` go to the R++ program. `Console::readLine()` reads from standard input, including from a pipe or redirected file.

```
cargo run -- run PATH -- one two
cargo run -- run --release FILE.rpp
cargo run -- run --check-invariants FILE.rpp   # Check invariants I-1, I-2, I-4, and I-5 from Chapter 5
cargo run -- tokens FILE.rpp                   # One token per line for comparison against rpp-compiler
cargo run -- pre debug,linux FILE.rpp         # Apply conditional compilation
cargo run -- ast debug,linux FILE.rpp         # Print the syntax tree
cargo run -- project std PATH                 # List projects, files, and diagnostics. Place --release before std.
cargo run -- defs std PATH                    # Print declaration tables and declaration diagnostics
cargo run -- diagnose std PATH                # Print all diagnostics, including warnings and rule references
```

A message identifies the violated rule and, where it helps, the rules that justify the violation:

```
n03_assignment_invalidates_narrowing.rpp:26:8: Error [R-6.2, R-6.11] z has nullable type Node? and has not been checked here
```

## Equivalence with grammar

The Python test run (`rpp-grammar/tools/run_tests.py`) checks the same cases with the ANTLR parser generated from `RPP.g4`, and the differential test compares both parsers on randomly mutated programs:

```
cd ..
py rpp-grammar/tools/difftest.py 3000 2
```

Status 21.11.2026: 9 600 mutants with three seeds, no difference in acceptance or rejection, the first line of error matches in all rejected cases.

## Comparison with the FR++ model

`fr-model/compare.py` translates FR++ programs into R++ and checks whether `rppc` accepts or rejects them in the same way as the model (needs `target/debug/rppc`):

```
cd ../fr-model
py -X utf8 compare.py 5000
```

Status 21.11.2026: no deviation for all translatable handwritten programs and 4466 translatable from 5000 random programs (`fr-model/comparison-report.txt`).
