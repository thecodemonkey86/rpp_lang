# R++ Exceptions (Chapter 9)

[Deutsch](R%2B%2B%20Exceptions.de.md)

Status: 11/21/2026, Draft 0.2. This document fills Chapter 9 of the R++ Specification Outline, which regulates what can be thrown and caught, how `try`, `catch` and `finally` run, what happens during unwinding, and how program errors terminate the program. For ownership and release, Chapter 5 applies, for narrowing, Chapter 6 (R-6.12), for accessibility, Chapter 8.

All determinations have been decided (see “R++ Design Decisions”, Sections 11 and 22). `R-9.n` Conformance tests shall be carried out in: `conformance/ch09_exceptions/`.

## 0. Supplement when writing the runtime tests (adopted on 21.11.2026)

| No | Proposal |
|---|---|
| V-9.1 | Each type of program error has a fixed name that appears in the message (table in R-9.2); runtime tests check it; stack overflow and memory deficiency are sort of “resources” in the table. |

## 1. Two types of errors at runtime

**R-9.1 (Exceptions and program errors).**
- An **Exception** is triggered with `throw` and can be caught with `catch`. Exceptions are unchecked: methods do not specify what they throw.
- A **program error** is triggered by the language itself or by `fail`. It cannot be caught: the program still cleans up (R-9.14) and then ends.

**R-9.2 (Program error).** Program errors are:

| Species | Triggers | Mode mode | Name in the message (V-9.1) |
|---|---|---|---|
| Index | Index or range outside boundaries (arrays, vectors, slices, strings) | both | `IndexError` |
| Overflow | Integer overflow, out-of-range shift count | only debug (release: wraparound or modulo) | `OverflowError` |
| Division | integer division or `%` by 0 | both | `DivisionError` |
| Conversion | `cast` outside the target range, NaN to integer | only debug (release: cutting or saturation) | `CastError` |
| Conversion | `cast<char>` without a valid scalar value, `cast<E>` without a suitable variant | both | `CastError` |
| Commitment | `assert(c, text)` with `c == false` | both | `AssertError` |
| Lock | Conflict of access to a `shared` object (Chapter 5, V-1) | both | `BorrowError` |
| Destructor | an exception leaves a destructor (V-11) | both | `DestructorError` |
| Abortion | Call of `fail(text)` (R-9.19) | both | `FailError` |
| Resources | Stack overflow, missing memory (Chapter 12, R-12.10) | both | `StackOverflowError`, `OutOfMemoryError` |
| C interop | an argument or result that cannot be converted, a missing library or a missing symbol at start (Chapter 15) | both (missing library in release mode: build error, R-12.5) | `NativeError` |

**R-9.3 (Speaking in other chapters).** Where chapters 4 to 8 say for these checks that “an exception is thrown”, a program error is meant after R-9.2. Program errors are not classes; their names (`IndexError`, `OverflowError`, `BorrowError`, etc.) appear only in the error message.

Because a program error cannot be caught, debug and release modes never branch out differently just because a check in debug mode strikes.

## 2. The class Exception

**R-9.4 (Exception).** The standard library provides the class `rpp.core.Exception`:

```
export class Exception {
public:
    string message;
    Exception? cause;            // Owns the cause
    Exception[] suppressed;      // Suppressed during unwinding (R-9.10)
    constructor(string message);
    constructor(string message, own Exception cause);
    virtual string describe();   // Message with causes and suppressed exceptions
}
```

Own exceptions inherit directly or indirectly from `Exception` and may have further fields.

## 3. Throw

**R-9.5 (Throw).** `throw e;` requires a type `own C` expression with `C <: Exception`. The value is shifted; a `shared` value, borrow or nullable value cannot be thrown. `throw` is an statement that does not end normally (Chapter 6, Section 3.3).

## 4. Try, catch, finally.

**R-9.6 (Construction).** One `try` has at least one `catch` or one or more `finally` The type in `catch (C e)` is a class `C <: Exception`, without `?` and without ownership mode.

**R-9.7 (Selection).** Formed in `try`-block an exception, the first `catch` whose type is an upper class of the actual class of the exception. `catch (Exception e)` Catch every exception. `catch` The one behind one `catch` stands for one of its upper classes, can never be chosen; the compiler reports it as an unattainable code (warning, R-8.27). Does not fit `catch`, the exception continues.

**R-9.8 (Catch variable).** The variable of a `catch` has the exception (`own`). `throw e;` throws it further (move); otherwise it is released at the end of the `catch` block.

**R-9.9 (finally).** The `finally`-Block always runs when the `try`-block or an `catch`-block is left: at the normal end, by `return`, `break` or `continue` And while an exception is going on, then it goes on like it does without. `finally` would have gone further.

**R-9.10 (Leaving finally).** One `finally`-Block must not be with `return` are abandoned; `break` and `continue` not to leave it (within a loop in the `finally` They are allowed. `throw` directly to `finally`-Block is an error. But if a call is thrown in the `finally` block while an exception is already running, the new exception replaces the old one; the old one is attached to the field `suppressed` of the new one and is not lost.

## 5. Unwinding

**R-9.11 (Expiry).** If an exception runs, the surrounding blocks are left from the inside out until a suitable `catch` is found.
1. the variables declared there are released in reverse order (chapter 5; shifted variables are skipped thanks to the drop flag),
2. lift the locks of `shared` objects held by the block (Chapter 5, Section 5.2),
3. runs an existing `finally` block.

**R-9.12 (Destructors).** Destructors run unwinding as they normally leave a block, leaving an exception to a destructor is a program error (R-9.2).

**R-9.13 (Constructors).** If a constructor is thrown, the object is never finished; the fields already set and, if the base constructor was already finished, the base part are released; the destructors of the base classes run. The destructor of the class itself does not run. `copy` The same applies.

## 6. Program Error and Program End

**R-9.14 (Abortion).** If a program error is cleared up as in R-9.11: variables are released, destructors and `finally` blocks run, but none `catch` is selected, after which the program ends with a message on the error output and the exit code **2**. If another program error occurs during cleaning up or an exception leaves a destructor or `finally` block, the program ends immediately.

**R-9.15 (Unprisoned Exception).** If an exception `main` leaves, the program ends after cleaning up with `describe()` the exception on the error output and the exit code **1**.

**R-9.16 (Stacktrace).** In debug mode, each exception and program error carry a stacktrace that appears in the message, while in release mode, a stacktrace is left to the implementation.

## 7. Interaction with other chapters

**R-9.17 (Moves and assignments in try).** For the purpose of checking `catch` and `finally` blocks and the statements thereafter, a variable shall be considered to have been moved if it can be moved anywhere in the `try` block and shall be attributed only if it was before `try`. At runtime, drop flags decide (chapter 5), V-5For narrowing: R-6.12.

**R-9.18 (No try-with resources).** There is no `using` and no try-with-resources. Resources like files free up their destructor; it also runs during unwinding.

**R-9.19 (Failure).** The built-in function `fail(string text)` triggers a program error with the message `text` and never returns. Therefore, a call to `fail` does not end normally: it counts for narrowing (Chapter 6, Section 3.3), for secure return (R-8.26) and for unreachable code (R-8.27) as `throw`. `assert` does not count as such, because it ends normally when the condition is met.

## 8. Example

```
class ParseError : Exception {
public:
    i64 line;
    constructor(string message, i64 line) : base(message) {
        this.line = line;
    }
}

class Loader {
public:
    Config load(string path) {
        File f = File::open(path);        // The destructor closes the file even if an exception occurs
        try {
            return parse(f.readAll());
        } catch (ParseError e) {
            throw new Exception("Konfiguration ungültig: " + path, e);   // e becomes the cause
        }
    }

    Config parse(string text) {
        if (text.length == 0) {
            throw new ParseError("leer", 1);
        }
        …
    }
}
```

## 9. Conformance tests

The test programs are in `conformance/ch09_exceptions/`; they are tested with `rppc test` (first implementation, milestone M3).

| Rules | Accepted | Rejected |
|---|---|---|
| R-9.4 R-9.5 Exception, throw | `a01_throw_and_catch.rpp` | `n01_non_exception_type.rpp`, `n02_throw_shared.rpp` |
| R-9.6 – R-9.8 catch | `a01_throw_and_catch.rpp`, `a02_rethrow.rpp` | `n03_catch_non_exception_type.rpp`, `n04_used_after_rethrow.rpp` |
| R-9.7 Unreachable catch | `a04_warning.rpp` | – |
| R-9.9, R-9.10 finally | `a01_throw_and_catch.rpp` | `n05_return_in_finally.rpp`, `n06_throw_in_finally.rpp`, `n07_break_from_finally.rpp` |
| R-9.17 Moves in try | `a02_rethrow.rpp` | `n08_move_in_try.rpp` |
| R-9.19 Fail, assert | `a03_fail.rpp` | `n09_assert_does_not_exit.rpp` |
