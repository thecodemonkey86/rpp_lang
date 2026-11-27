# R++ Execution Model (Chapter 12)

[Deutsch](R%2B%2B%20Execution%20Model.de.md)

Status: 11/21/2026, Draft 0.2. This document fills chapter 12 of the R++ Specification Outlines, which regulates how debug and release modes are related, what behavior a program can observe, what modes may differ, how a program starts and ends, and what happens when resources are exhausted.

All determinations have been decided (see “R++ Design Decisions”, Sections 1 and 25). `R-12.n` Conformance tests shall be carried out in: `conformance/ch12_execution/`.

## 0. Amendments to the formulation (adopted on 21.11.2026)

| No | Proposal |
|---|---|
| V-12.1 | `args` in `main` contains only the arguments, not the program name (such as Java and C#). |
| V-12.2 | Each implementation guarantees at least 10,000 nested method calls before a stack overflow is allowed to occur. |

## 1. Two modes, one language

**R-12.1 (Modes).** A program is executed in **debug mode** (interpreted or translated via JIT in a virtual machine, for short development cycles) or **release mode** to a native program.

**R-12.2 (Working together).** Both modes go through the same frontend: conditional compilation, parsing, name resolution, type checking, ownership and null checking; a program that is rejected in one mode is also rejected in the other, unless the difference lies in a branch that is active only in one mode (`#if debug`). Both modes work with the same intermediate representation.

**R-12.3 (Reference).** The debug interpreter is the reference implementation and the release compiler is tested against it; if there is a different observable behavior (R-12.4) outside of the allowed differences (R-12.5), the release compiler is faulty.

## 2. Observable behavior

**R-12.4 (What is Observable).** Observable are:
- the output to standard and error output in their order,
- the contents of written files and directories,
- the read input in its order,
- the exit code and whether the program ends.

Unobservable are runtime, memory usage, addresses of objects, and the sequence of operations that have none of the above effects.

**R-12.5 (Allowed differences).** Debug and release modes must show the same observable behavior, and they may differ only in these points:

| Point | Debug | Release | Rules |
|---|---|---|---|
| Integer overrun | Program error | Invoice modulo 2n | R-4.14 |
| Moving width outside the area | Program error | modulo bit width | R-4.16 |
| `cast` between integers outside the target range | Program error | Lower bits | R-4.9 |
| `cast` from floating point outside of range, NaN | Program error | Saturation, NaN → 0 | R-4.10 |
| Stacktrace in messages | always | Leave to implementation | R-9.16 |
| Stack overflow, storage deficiency | Program error | Program error, but possible elsewhere | R-12.10 |
| Code in `#if debug` or `#if release` | Active | Inactive or vice versa | R-11.24 |
| Missing C library or missing symbol | Program error before `main` (`NativeError`) | Build error | R-15.12 |

Because program errors cannot be caught (Chapter 9), none of these differences will cause a program to take different `catch` branches in both modes.

**R-12.6 (No undefined behavior).** For each translated program, the specification specifies the observable behavior, except for the points in R-12.5. There is no undefined behavior: no invalid memory accesses (chapter 5), no zero dereference (chapter 6), no uninitialized values (R-4.46), a fixed evaluation order (R-8.2) and defined behavior at overflow. For programs with C code, this holds only under the conditions of R-15.16.

**R-12.7 (floating-point).** Floating point operations are executed bit-accurately according to IEEE 754 in both modes: each operation individually with rounding to the next value, without summarizing to an FMA operation and without extended intermediate accuracy. Debug and release deliver the same bits. Mathematical functions of the standard library (if available) deliver the same results in both modes.

## 3. Program start and end

**R-12.8 (Start).** A program begins by calling `main(args)`. `args` contains the command line arguments without the program name (V-12.1), as `string` values in UTF-8. There are no global variables and no initialization before `main`: `static const` Values are fixed at compile time.

**R-12.9 (End date).** The Program ends
- with the return value of `main` as an exit code after the local variables of `main` are released;
- with exit code 1, if an exception `main` leaves (R-9.15);
- with exit code 2 in the event of a program error (R-9.14).

There is no premature quit function (V-13.3).

**R-12.10 (Resources).** A stack overflow and missing memory are program errors: The program ends with a message and exit code 2 is cleaned up, as far as it is still possible; after a memory deficiency, the specification does not require unwinding. Each implementation guarantees at least 10,000 nested method calls (V-12.2).

## 4. Memory and threads

**R-12.11 (Release).** Objects are released deterministically as soon as their owner abandons them (chapter 5); there is no garbage collection. Destructors run in the specified order (V-7); objects in cycles out `shared` references are never released.

**R-12.12 (Threads).** Version 1 does not have a thread library; a program runs in a thread. For later versions, each object belongs to exactly one thread; another thread has only one `own` value whose tree contains no `shared` or `weak` references (Chapter 5, Section 7). Strings are exempt from this and then need an atomic counter.

## 5. Conformance tests

The runtime tests are in `conformance/ch12_execution/runtime/` and set the expected behavior per mode (format: Chapter 14), as long as there is no implementation, only the syntax can be checked on them.

| Rules | Run-time test |
|---|---|
| R-12.5 Overflow | `l01_overflow.rpp` |
| R-12.5 Cast | `l02_cast.rpp` |
| R-12.7 floating point | `l03_floating_point.rpp` |
| R-12.9 Exit codes | `l04_exit_code.rpp`, `l05_exception_from_main.rpp`, `l06_program_error.rpp` |
| R-12.11 Release order | `l07_destructors.rpp` |
