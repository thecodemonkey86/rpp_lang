# FR++ — executable ownership core calculus

[Deutsch](README.de.md)

The FR++ model checks types and interprets the core language defined in Chapter 5. It implements R-5.1 through R-5.22 and the evaluation rules from Section 5. The random tests exercise Theorems 5.1–5.3. [Ownership core calculus](../R%2B%2B%20Ownership%20Core%20Calculus.md)

Run the following commands from this directory with Python 3.10 or later. The model uses only the Python standard library.

```
py -X utf8 run_tests.py                          # Handwritten test programs
py -X utf8 fuzz.py 2000 --size 1.0                # Random tests with 2,000 programs
py -X utf8 fuzz.py 600 --disable-rule loans       # Control run with one rule disabled
py -X utf8 compare.py 5000                        # Compare against rppc after building it in ../rppc
py -X utf8 compare.py 300 --disable-rule loans     # Expect mismatches in this control run
py -X utf8 compare.py 2000 --run                   # Also compare execution with invariant checks
```

## Contents

| Path | Purpose |
|---|---|
| `frpp/ast.py` | Abstract syntax (section 3.1) and back-translation into text |
| `frpp/parser.py` | Parser for the text form |
| `frpp/checker.py` | Type checker: Places, Loans, Rules R-5.1 to R-5.22 |
| `frpp/interp.py` | Interpreter: heap, drop flags, lock, release; testing of invariants I-1, I-2, I-4, I-5 |
| `frpp/gen.py` | Random generator for programs |
| `run_tests.py` | Test run for `tests/` |
| `fuzz.py` | Random test for theorems 5.1 to 5.3 |
| `tests/accepted/*.fr` | Programs that are adopted and must deliver a certain result |
| `tests/rejected/*.fr` | Programs that have to be rejected with a certain rule in a certain line |
| [Test report](test-report.txt) · [Deutsch](test-report.de.txt) | Historical output from `run_tests.py` and `fuzz.py` |
| `frpp/to_rpp.py` | Translation of FR++ into R++ (Chapter 5, Section 7, backwards) |
| `compare.py` | Comparison with the first implementation `rppc`: same judgment for translated programs |
| [Comparison report](comparison-report.txt) · [Deutsch](comparison-report.de.txt) | Historical comparison against `rppc` |

## Text form

As in section 3.1 of the calculus, a program consists of classes and a block `main { … }`. Methods with return type `ref C` or `mut C` provide a borrow (`return &ro this.f;`); their result is tied to `borrow x = p.m(…) { … }` (R-5.23, R-5.24).

```
class A {
    int v;
    int get() ro { return copy this.v; }
    int inc() mut { this.v = copy this.v + 1; return 0; }
    destructor() { print(copy this.v); }
}
main {
    own A a = new A(1);
    shared A s = share new A(2);
    weak A w = demote copy s;
    upgrade t = w { print(t.get()); } else { print(0); }
    borrow x = &mut a { x.inc(); }
}
```

The model adds `print(e);` and the `int` operators `+`, `-`, and `<` to support tests. Methods always return a value. Use `return 0;` for a method that would otherwise return `void`.

## Test Programs

The first comment line `// erwartet:` sets the result:

- `// erwartet: Ausgabe 1 2 3` – the consequence of `print` expenditure, including expenditure of destructors
- `// erwartet: BorrowError` – the program ends with a blocking conflict (V-1)
- `// erwartet: Fehler R-5.9 …` – the type test rejects with this rule in the line that bears `// FEHLER`
- `// ohne Typprüfung: Fehlerzustand` (in addition to rejected programs) – the program runs despite rejection and must then really run into an error state (or end with `BorrowError`). This proves that the rule prevents a real error and not only rejects a harmless program.

Accepted programs run with invariant checking after each statement. A failure state in the sense of sentence 5.1 is always a failure, as well as an object left at the end, although nothing was ever shared.

## Random test

`fuzz.py` creates programs with a fixed class table and random method bodies, destructors and main programs. The generator pays rough attention to moves and borrows, but intentionally violates it in about one in ten cases. Each program is tested and executed:

- **Programs adopted** You must never run into an error state and not violate an invariant (sentence 5.1). You must only end with `BorrowError` if an object has been divided (sentence 5.3). Each violation is filed with program text in `fuzz-findings/`.
- **Rejected Programs** And then we count how many of them actually run into a failure state, which shows that the rules intercept real errors.
- “Budget” means that the program has used up its step budget (endless recursion via random methods), which counts as non-termination and is allowed.

`--disable-rule NAME` switches off a rule of the type checker, then infringements must occur; otherwise the random test does not reach the rule.

| Name of name | Deactivated |
|---|---|
| `loans` | `erlaubt_L` (all loan checks) |
| `move` | Condition ○ after a move |
| `join` | Association of branches in R-5.15 |
| `loop` | Condition `Γ₁ ⊒ Γ` in R-5.16 |
| `write-permission` | `schreibbar`: `ref` roots are treated as writable |
| `receiver-loan` | Does not record a loan for the receiver of a call that returns a borrow (R-5.24) |
| `shared-overlap` | Overlap checking for whole shared roots, leaving prefix checks enabled |

## Boundaries

- No `null`, no exceptions, no `try`; one `BorrowError` terminates the program.
- No vectors, slices, lambdas and strings; they are mapped to the core language in section 7 of the calculus, but are not implemented here.
- Random tests find errors, but do not prove correctness.
