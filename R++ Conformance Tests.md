# R++ Conformance Tests (Chapter 14)

[Deutsch](R%2B%2B%20Conformance%20Tests.de.md)

As of 22.11.2026, Draft 0.4. This document fills in Chapter 14 of the R++ Specification Outline, which specifies how the Conformance test suite is structured, how a test program indicates its expected result, when an implementation is compliant and how coverage of the rules is checked.

The decisions for this chapter appear in Sections 25 and 26 of the design decisions, including V-14.2 and V-14.3. Normative rule numbers follow the form `R-14.n`. [Design decisions](R%2B%2B%20Design%20Decisions.md)

## 0. Clarifications adopted during the rule coverage review

| No | Proposal |
|---|---|
| V-14.1 | Runtime tests can set command line arguments (`// Argumente: …`) and default input (`// Eingabe: "…"`) (R-14.5). |
| V-14.3 | (assumed on 22.11.2026, from Milestone M5) Each runtime test runs per mode in a fresh, empty working directory, where it is allowed to create, read and delete files and directories; relative paths refer to them. After that, the directory is deleted. Outside of it, a test does not write anything. `rpp.io` (R-13.19, R-13.20) can be checked without prepared files, and tests do not interfere with each other. |
| V-14.2 | (assumed on 21.11.2026, from Milestone M3) An error message may specify several rules: the violated rule and the rules that justify why it is violated. `x.run()` after `x = null;` Injured R-6.2, justified by R-6.11; indicates the message `[R-6.2, R-6.11]`. A rejected test is met if one of the above rules is the expected one (R-14.7). without this clarification, the main rule would have to change depending on the cause, although the violation is the same. |

## 1. Test suite layout

**R-14.1 (Storage).** The test suite is located in the directory `conformance/`, per chapter in a subdirectory `chNN_topic/` with up to three types of tests:

| Directory | Contents |
|---|---|
| `accepted/` | Programs that need to be translated, if necessary with the exact warnings marked |
| `rejected/` | Programs that have to be rejected with exactly one error of a particular rule in a particular line |
| `runtime/` | Programs with `main`, which are executed and must deliver a fixed output and a fixed exit code |

A test can be a single file `name.rpp` or a **project test** under `name/` with an `rpp.yaml` manifest. Use a project test when the case requires multiple files or a manifest, especially in Chapter 11. Place test headers at the top of `rpp.yaml` as YAML comments such as `# Regel: …`. Any library referred to by `path` in a subdirectory belongs to the same test.

**R-14.2 (Names).** File names begin with `a` (accepted), `n` (rejected, “negative”) or `l` (run time), followed by a two-digit number and a short description: `n05_return_in_finally.rpp`.

## 2. Headers and markers

**R-14.3 (Accepted programs).** Start with `// Konformitätstest Kapitel N – muss übersetzt werden.` and optionally follow it with `// Regel: R-x.y, …`. Specify expected warnings in the header (`// Erwartet: genau eine Warnung, …`) and end each line that should produce a warning with `// WARNUNG`. Reserve `// FEHLER` for rejected programs.

**R-14.4 (Rejected programs).** Include the following header lines.
- `// Regel: R-x.y …` – the first rule mentioned is the one whose error is expected, the others explain the connection;
- `// Erwartet: genau ein Fehler, in der mit FEHLER markierten Zeile (Begründung).`

Exactly one line is marked at the end `// FEHLER`, in YAML files of a project test `# FEHLER`. otherwise, the program does not contain an error; without the marked line it would be correct or at most incomplete. The expected error may be a syntax error or a directive error (for example, for `??=` or `#define`); then the first reported error must be in the marked line.

**R-14.5 (Runtime tests).** Include `// Regel: …` and one or two expectation lines.

```
// Erwartet: Ausgabe "…"; Exit-Code n; Fehlerausgabe enthält "…"
// Erwartet (debug): …
// Erwartet (release): …
```

Run each runtime test in a fresh, empty working directory for each mode. The test may write there. Remove the directory after the test (V-14.3).

Optionally, set two more headers to start the program (V-14.1):

```
// Argumente: eins zwei
// Eingabe: "a\nb\n"
```

`Argumente` are words separated by spaces, which `main` is given in `args`; `Eingabe` is the full text of the standard input in the spelling of a string literal; if they are missing, there are no arguments and an empty input.

Without mode, the expectation applies to both modes. `Ausgabe` is the full text of the standard output; lines are separated by `\n`, the last line break is missing. `Fehlerausgabe enthält` only checks if the text occurs, otherwise messages may differ per implementation. Missing `Ausgabe`, the standard output is not checked.

**R-14.6 (Rule coverage).** Refer to each rule `R-x.y` in at least one test header. Where applicable, include an accepted and a rejected case. The test runner compares rule definitions with the references in test headers and lists any gaps without failing the run. It excludes Chapters 1 and 14 because they concern the specification and test suite themselves. Section 5 explains the remaining rules that cannot be tested with a single program.

## 3. Implementation conformance

**R-14.7 (Conforming implementation).** An implementation conforms if it produces the expected result for each test in every mode it supports.
- accepted programs translate them, with exactly the marked warnings and no more;
- reject rejected programs with an error of the said rule in the marked line (the rule may stand as a justification next to the violated rule, V-14.2);
- Runtime tests run it in debug and release modes, delivering output, exit code and error output as expected.

An implementation that offers only one mode is compliant with this mode; the rule number of an error specifies the implementation in its message; the wording is free.

## 4. Tools and status

**R-14.8 (Test runs).** These tools test the test suite:

| Tools | Tested |
|---|---|
| `rpp-grammar/tools/run_tests.py` | that all test programs are syntactically valid, including all files of a project test; that an expected syntax error is in the marked row; the markings `FEHLER` and the headers of the runtime tests; the rule coverage (R-14.6) |
| `fr-model/run_tests.py`, `fr-model/fuzz.py` | the rules of Chapter 5 on the core language FR++, including execution and random testing |
| `fr-model/compare.py` | that `rppc` accepts or rejects FR++ programs, translated according to Section 7 of Chapter 5, as the model (handwritten and random programs) |
| `rppc test` (first implementation, from milestone M2) | translate or reject according to R-14.7: accepted and runtime tests without errors and with warnings exactly in the `WARNUNG` lines; rejected only with errors in the marked line, including one of the expected rule (V-14.2). The interpreter performs runtime tests in both modes and compares output, exit code and error output (from M4). |
| `rppcc test` (R++ compiler in R++, from milestone M8) | same as `rppc test` with the same judgment per test; runtime tests are translated into native programs in both modes (C code generator and C compiler) and executed in a fresh, empty working directory. Thus, the implementation is also tested for the release mode according to R-14.7. |

Status 26.11.2026 (M9): `rppc test` and `rppcc test` pass all 332 tests; new are 13 tests for the registry in the file system (chapter 11, section 5a) and `l50_sha256` (R-13.46). Before that, status 26.11.2026 (M8): `rppc test` and `rppcc test` pass all 318 tests, `rppcc test` under Windows (MSVC) and Linux (gcc) with native programs in both modes; new are the runtime tests for processes, the program path and `Dir::removeAll` (R-13.43 to R-13.45). Checks the semantics of all chapters `rppc test`; as of 23.11.2026: all 293 tests pass (translate or reject, runtime tests performed in both modes), including new tests for `rpp.random` (R-13.23 to R-13.25), the vector algorithms (R-13.26, R-13.27), `rpp.math` (R-13.28, R-13.29), the text aids (R-13.30), the file operations (R-13.31) and `Deque` and `getOrPut` (R-13.32, R-13.33).

**As of 21.11.2026:**

| Chapter | Accepted | Rejected | Runtime | Rules covered |
|---|---|---|---|---|
| 4 Types | 8 | 27 | 2 | 47 of 47 |
| 5 Ownership (R++ and FR++) | 7 + 11 | 26 + 25 | 3 | 25 of 25 |
| 6 Null safety | 5 | 17 | 2 | 23 of 23 |
| 7 Classes and Generics | 6 | 30 | 4 | 28 of 28 |
| 8 statements | 5 | 22 | 4 | 26 out of 28 (+ 2 without test) |
| 9 Exceptions | 4 | 10 | 6 | 19 of 19 |
| 10 Lambdas | 4 | 16 | 1 | 18 of 18 |
| 11 packages and projects | 4 | 13 | 1 | 21 out of 25 (+ 4 without test) |
| 12 Execution model | – | – | 10 | 8 of 12 (+ 4 without a dedicated test) |
| 13 Standard Library | 5 | 7 | 9 | 19 out of 20 (+ 1 without test) |
| **Total** | | | | **234 of 245**, for 11 reasons without test |

Project tests count as one test. Each rule in chapters 4 to 13 is mentioned in at least one test head or justified in section 5; all tests are syntax tested and also semantically (translate or reject) with the first implementation; the outputs of the runtime tests are checked from milestone M4.

## 5. Rules without a dedicated test

These rules cannot be checked with a single test program (R-14.6). The test run reads the table and does not count it as a gap.

| Rules | Justification |
|---|---|
| R-8.11 | Reference to Chapters 4, 6, 7 and 10; verified by their tests |
| R-8.28 | Reference to Chapter 9; tested by its tests |
| R-11.31 | `publish` and `withdraw` are commands, not programs; checked by `.github/scripts/check.py registry` (chapter 11, section 8) |
| R-11.22 | the interim representation is not observable (R-12.4) |
| R-12.1 | Definition |
| R-12.2 | covered by R-14.7: each test applies to both modes |
| R-12.3 | concerns the development of implementation, not a program |
| R-12.12 | summary of Chapter 16; covered by its tests |
| R-13.2 | Concerns the versioning of the library, not a program |
| R-15.16 | Boundary of the guarantees for C code; covered by comparing both modes in all runtime tests of chapter 15 |
| R-15.17 | covered by R-14.7: each runtime test of chapter 15 applies to both modes |
| R-16.21 | repeats R-12.10 for every thread; a stack overflow is not tested (R-12.10) |
| R-16.23 | covered by R-14.7: each runtime test of chapter 16 applies to both modes |
