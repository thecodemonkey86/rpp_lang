# R++ Grammar and Test Run

[Deutsch](README.de.md)

The executable ANTLR4 grammar for R++ and its test programs.

## Contents

| Path | Purpose |
|---|---|
| `RPP.g4` | Grammar in ANTLR4 format (normative) |
| `tools/preprocess.py` | Resolves `#if`, `#else`, and `#endif` before parsing |
| `tools/run_tests.py` | Test run with the ANTLR parser |
| `tools/difftest.py` | Checks that ANTLR and the Rust parser agree on mutated programs |
| `tests/pos/*.rpp` | Example programs that must be accepted |
| `tests/neg/*.rpp` | Programs that must be rejected (first line `// Name: …`) |
| `tests/parse_probes/*.rpp` | The first statement must match the grammar rule named in `// Lesart: …` |
| `../conformance/**/*.rpp` | Syntax checks for the conformance suite, including cases rejected during semantic analysis |
| `gen/` | Python parser produced from `RPP.g4` |
| [Test report](test-report.txt) · [Deutsch](test-report.de.txt) | Historical test output |
| `examples/hello/rpp.yaml` | Example program with a dependency |
| `examples/mathlib/rpp.yaml` | Example library |

## Run the tests

Prerequisite: Python 3 and the package `antlr4-python3-runtime` (version 4.13.2).

```
pip install antlr4-python3-runtime==4.13.2
python3 tools/run_tests.py
```

To check the same cases with the Rust parser, run `cargo run -- syntaxtest ..` from `rppc/`.

## Regenerate the parser

After editing `RPP.g4`, regenerate the files in `gen/` with Node.js.

```
npm install antlr-ng
npx antlr-ng -Dlanguage=Python3 -o gen RPP.g4
```

You can also use the official Java tool.

```
java -jar antlr-4.13.2-complete.jar -Dlanguage=Python3 -o gen RPP.g4
```

The checked-in parser was generated with antlr-ng 1.0.10. The grammar has yet to be verified with the Java tool.
