# R++ Introduction (Chapter 1)

Status: 21.11.2026, draft 0.1. This document fills Chapter 1 of the “R++ Specification”. It describes what R++ is intended for, how the specification is structured, what terms it uses and what conformity means.

## 1. What R++ is

R++ is an object-oriented, statically typed language with a syntax close to C, C++, Java and C#, but simpler. It has three objectives:

1. **Certain error classes are formally excluded. ** A translated program never access shared memory, does not release anything twice, never dereferences `null`, does not read uninitialized values, and does not use invalid iterators. This is made possible by an ownership model with `own`, `shared`, `weak` and Borrows, which manages without lifetime annotations (chapter 5), and type narrowing for nullable values (chapter 6). What cannot be checked at the translation time (index, overflow in debugging mode, conflicts in `shared` objects) ends in a controlled manner as a program error (Chapter 9).
2. **Short development cycles and fast programs. ** In debugging mode, interpretation is made or executed via JIT, natively translated in release mode. Both modes show the same observable behavior except for a small, fixed list of differences (Chapter 12).
3. **Few surprises. ** There is no undefined behavior, no macros, no header files, no implicit copy of objects, and no implicit conversions that change values.

## 2. Structure of the specification

| Cap. | Theme | Document |
|----|----|--|-|
| 1 | Introduction | “R++ Introduction” (this document) |
| 2 | Lexical structure | “R++ specification framework”, section 2 |
| 3 | Grammar | “R++ Specification Scaffolding”, Section 3, and `rpp-grammar/RPP.g4` (normative) |
| 4 | Types | “R++ types” |
| 5 | Ownership and Borrowing | “R++ Ownership Kernkalkül” |
| 6 | Zero security | “R++ zero security” |
| 7 | Classes, Mixins, Type Parameters | “R++ Classes and Generics” |
| 8 | Instructions and expressions | “R++ Instructions and Expressions” |
| 9 | Exceptions | “R++ Exceptions” |
| 10 | Lambdas | “R++ Lambdas” |
| 11 | Packages and projects | “R++ packages and projects” |
| 12 | Model | “R++ execution model” |
| 13 | Standard Library | “R++ Standard Library” |
| 14 | Conformity tests | “R++ conformity tests” |

The decisions on which the chapters are based are in the document “R++ Design Decisions” with date and status. Chapter 5 is additionally converted into `fr-model/` as an executable model.

## 3. Conventions

**R-1.1 (Normative and informative). ** Normative are the numbered rules `R-x.y`, the grammar `RPP.g4` and the tables to which a rule refers. Examples, justifications, overview tables and annotations are informative.

**R-1.2 (rule numbers). ** Each normative rule carries a number `R-<chapter>.<running number>`. Numbers are not reassigned; if a rule is omitted, their number remains free.

**R-1.3 (notation). ** Syntax is in ANTLR4 notation (`:` definition, `|` alternative, `? `optional, `*` as often as you like, `+` at least once, `'....''' terminal). Chapter 5 also uses inference rules: above the line are the prerequisites, including the conclusion.

## 4. Terms

| term | meaning |
|---|--|
| Program | A `executable` type project with all libraries it uses |
| Project | A directory with `rpp.yaml` and source folders (Chapter 11) |
| Library | A project of the type `library`; its exported types can be used in other projects |
| Package | A group of types whose name corresponds to the directory in the source folder |
| Implementation | A tool that checks R++ programs and runs in debug and/or release mode |
| Debug mode, Release mode | The two types of execution (Chapter 12) |
| Error | A violation of a rule that the implementation reports before execution; the program is not translated |
| Warning | A message that requires the specification but does not prevent translation (Chapter 8, R-8.1) |
| Exception | A catchable object thrown at runtime (Chapter 9) |
| Program Errors | A term-related injury (index, overflow in debug, `fail` ...) that cannot be caught and terminates the program after cleaning up (Chapter 9) |
| Possession, Borrow, Loan | Who releases an object, who uses it temporarily and which accesses are blocked (Chapter 5) |
| Observable behavior | Issues, written files, read input, exit code and whether the program ends (Chapter 12, R-12.4) |

## 5. Conformity

**R-1.4 (Compliant program). ** A program is compliant if it does not violate any rule of specification. For a compliant program, the specification specifies the observable behavior, except for the permissible differences between the modes (R-12.5).

**R-1.5 (Compliant Implementation). ** Implementation is compliant when they
- each compliant program translates and performs with the specified behavior,
- rejects any non-compliant program with at least one error, citing the rule number,
- reports the required warnings and
- the conformity test suite exists (Chapter 14, R-14.7).

**R-1.6 (limits of implementation). ** An implementation may have limits (size of memory, depth of calls), but must meet the minimum values from Chapter 12. If a program crosses a boundary, it is a program error and not an undefined behavior.

## 6. State

Chapters 4 to 14 are available as draft. The grammar is executable and tested, Chapter 5 is tested as a core calculus with executable model and random test. There is no implementation yet; the conformity tests are therefore so far only tested for syntax. The first implementation is created in Rust (“R++ Implementation Plan”). Also open is the machine proof of sentence 5.1 (storage security).
