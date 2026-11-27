# R++ Introduction (Chapter 1)

[Deutsch](R%2B%2B%20Introduction.de.md)

Draft 0.1, 21 November 2026. This chapter describes the goals of R++, the structure and terminology of the language specification, and what conformance means.

## 1. What is R++?

R++ is a statically typed, object-oriented language with a simpler syntax close to C, C++, Java, and C#. It has three goals.

1. **Rule out several classes of errors.** A compiled program never accesses freed memory, frees the same allocation twice, dereferences `null`, reads uninitialized values, or uses invalid iterators. R++ uses `own`, `shared`, `weak`, and borrows to manage ownership without lifetime annotations (Chapter 5). It narrows nullable types based on checks (Chapter 6). Index errors, overflow in debug mode, and conflicts involving `shared` objects are checked at runtime and handled as program errors (Chapter 9).
2. **Support short development cycles and fast programs.** Debug mode uses interpretation or JIT compilation. Release mode compiles to native code. Both modes have the same observable behavior apart from the specific differences listed in Chapter 12.
3. **Few surprises.** There is no undefined behavior, no macros, no header files, no implicit copy of objects, and no implicit conversions that change values.

## 2. Structure of the specification

| Chapter | Topic | Documentation |
|---|---|---|
| 1 | Introduction | “R++ Introduction” (this document) |
| 2 | Lexical Structure | ‘R++ Specification Outline’, Section 2 |
| 3 | Grammar | ‘R++ Specification Outline’, Section 3, and `rpp-grammar/RPP.g4` (normative) |
| 4 | types | “R++ Types” |
| 5 | Ownership and Borrowing | R++ Ownership Core Calculus |
| 6 | null safety | R++ Null safety |
| 7 | Classes, Mixins, Type Parameters | R++ Classes and Generics |
| 8 | Statements and expressions | R++ Statements and Expressions |
| 9 | Exceptions | R++ Exceptions |
| 10 | Lambdas | ‘R++ Lambdas’ |
| 11 | Packages and projects | R++ Packages and Projects |
| 12 | Implementation model | R++ Execution Model |
| 13 | standard library | R++ Standard Library |
| 14 | conformance tests | R++ Conformance Tests |

The design decisions record the date and status of the changes behind these chapters. The executable FR++ model implements the ownership rules in Chapter 5. [Design decisions](R%2B%2B%20Design%20Decisions.md) · [FR++ model](fr-model/README.md)

## 3. Conventions

**R-1.1 (Normative and informative).** Normative are the numbered rules `R-x.y`, the grammar `RPP.g4` and the tables to which a rule refers. Examples, justifications, overview tables and annotations are informative.

**R-1.2 (Rule numbers).** Each normative rule has a number of the form `R-<chapter>.<number>`. If a rule is removed, its number stays vacant.

**R-1.3 (Notation).** The grammar uses ANTLR4 notation. `:` defines a rule, `|` separates alternatives, `?` marks optional elements, `*` denotes zero or more repetitions, `+` denotes one or more, and `'…'` denotes a terminal. Chapter 5 also uses inference rules with premises above the line and a conclusion below.

## 4. Definitions

| Definitions | Significance |
|---|---|
| Program | A project of type `executable` with all libraries it uses |
| Project | A list of `rpp.yaml` and source folders (Chapter 11) |
| Library | A project of type `library`; its exported types can be used in other projects |
| Package | A group of types whose name corresponds to the directory in the source folder |
| Implementation | A tool that checks R++ programs and runs them in debug und/oder release mode |
| Debug mode, release mode | The two types of embodiment (Chapter 12) |
| Error | A violation of a rule that the implementation reports before execution; the program is not translated |
| Warning | A message which requires the specification but which does not prevent translation (Chapter 8, R-8.1) |
| Exception | A catchable object thrown at run time (Chapter 9) |
| Program error | An injury detected at runtime (index, debug overflow, `fail` ...) that cannot be caught and ends the program after cleaning up (chapter 9) |
| Owned, Borrow, Loan | Who releases an object, who uses it temporarily and which accesses are blocked (chapter 5) |
| Observable behaviour | Expenditure, written files, read input, exit code and whether the program ends (Chapter 12, R-12.4) |

## 5. Conformance

**R-1.4 (Conforming Program).** A program is compliant if it does not violate any rule of the specification; for a compliant program, the specification specifies the observable behavior, except for the allowable differences between the modes (R-12.5).

**R-1.5 (Conforming implementation).** An implementation is compliant if it
- Each compliant program is translated and executed with the specified behavior,
- rejects each non-compliant program with at least one error, indicating the rule number,
- notify the requested warnings; and
- passes the Conformance test suite (Chapter 14, R-14.7).

**R-1.6 (Limitations of implementation).** An implementation may have limits (size of memory, depth of calls), but must meet the minimum values from Chapter 12; if a program exceeds a limit, this is a program error and not an undefined behavior.

## 6. Status

Chapters 4 to 14 are available in draft form. The grammar is executable and tested, Chapter 5 is tested as a core calculus with executable model and random tests. There is no implementation yet; therefore, the conformance tests have only been tested for syntax so far. The first implementation is created in Rust ("R++ Implementation Plan"). Also open is the machine proof of sentence 5.1 (storage security).
