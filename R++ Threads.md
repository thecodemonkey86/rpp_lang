# R++ Threads (Chapter 16)

[Deutsch](R%2B%2B%20Threads.de.md)

Status: 27.11.2026, draft 0.1. This document fills Chapter 16 of the "R++ Specification Outline". It specifies how an R++ program runs code in several threads: starting and joining threads, scopes whose threads borrow data of the enclosing block, which values may cross to another thread, channels as the only way to exchange data, exceptions and program errors in threads, and what remains observable when several threads run.

The fundamentals are decided (see "R++ Design Decisions", Section 12, milestone M11 in the Implementation Plan). The proposals in Section 0 arose while writing out the rules and were accepted on 27.11.2026, V-16.9 in an amended form (all threads unwind); the rules that depend on them name them. Every normative rule carries a number `R-16.n`. The conformance tests will be in `conformance/ch16_threads/`.

## 0. Amendments from writing out the rules (adopted on 27.11.2026)

| No. | Proposal |
|---|---|
| V-16.1 | `void` is not a type argument (R-4.42), so there are two handle types: `Thread::spawn` with an `own Func<void>` returns an `own Thread` whose `join()` returns nothing; with an `own Func<T>` it returns an `own Task<T>` whose `join()` returns `own T`. This replaces the `own Thread<T>` named in the decision. |
| V-16.2 | An `own` lambda cannot change its captures (R-10.11). For mutable working data, `Thread::spawnWith(own D input, own Func<own D, …> body)` hands `input` to the body as a parameter it owns. |
| V-16.3 | `Channel<T>::open(capacity)` returns a tuple `(own Sender<T>, own Receiver<T>)`. There is one receiver; a sender can be duplicated with `clone()`. `send`, `receive` and `clone` are not `mut` methods: the ends are synchronized internally, so a borrowed end can be used too. |
| V-16.4 | The element type of a channel is not nullable. `receive()` returns `T?`: `null` means the channel is empty and every sender has been released. `send` returns `false` if the receiver has been released; the value is then released. |
| V-16.5 | Whether a type may cross to another thread is expressed by the built-in constraint `Transferable` in `rpp.sync`. Every type that satisfies R-16.7 has it automatically; a class cannot import it. Generic code that starts threads with values of a type parameter writes `T : Transferable`. |
| V-16.6 | An exception crosses to the joining thread (R-16.5). Therefore every class derived from `Exception` must be transferable; a `shared` or `weak` field in its tree is an error, in programs with and without threads. |
| V-16.7 | New kind of program error `ThreadError` (R-9.2): a second `join`, a thread that waits for itself, a handle released while its thread ended with an exception nobody retrieved, a deadlock (R-16.19), a channel capacity below 1, a thread that cannot be started. |
| V-16.8 | With several threads, a program has a set of permitted behaviors instead of exactly one (amendment to R-12.6): every interleaving that respects the order of R-16.18 is permitted. A single output call is never interleaved with another. |
| V-16.9 | A program error in any thread ends the whole program with exit code 2, and every thread unwinds first: the failing thread as in R-9.14, the other threads after they have been cancelled at their next cancellation point (R-16.6). Only a thread that is stuck in reading input or in a C call stops without unwinding. |
| V-16.10 | `Thread::sleep(i64 millis)` lets the current thread wait. Sleeping does not count as waiting in the sense of R-16.19. |
| V-16.11 | `Scope.spawn` returns no handle, so a scope can start threads without `discard` (R-8.13). At the end of the scope, the exception of the first thread in start order that ended with one is thrown again; the others are released. |

## 1. Threads

**R-16.1 (rpp.sync).** The package `rpp.sync` of the standard library provides threads and channels:

```
export class Thread {
public:
    static own Thread spawn(own Func<void> body);
    static own Task<T> spawn<T : Transferable>(own Func<T> body);
    static own Thread spawnWith<D : Transferable>(own D input, own Func<own D, void> body);
    static own Task<T> spawnWith<D : Transferable, T : Transferable>(own D input, own Func<own D, T> body);
    static void scope(Func<mut Scope, void> body);
    static void sleep(i64 millis);
    void join() mut;
}

export class Task<T : Transferable> {
public:
    own T join() mut;
}

export class Scope {
public:
    void spawn(Func<void> body) mut;
}

export class Channel<T : Transferable> {
public:
    static (own Sender<T>, own Receiver<T>) open(i64 capacity);
}

export class Sender<T : Transferable> {
public:
    bool send(own T value);
    own Sender<T> clone();
}

export class Receiver<T : Transferable> {
public:
    T? receive();
}
```

None of these classes can be constructed with `new` or derived from. `Thread`, `Task`, `Sender` and `Receiver` exist only as `own` values; `copy` is an error for all of them.

**R-16.2 (Start).** `Thread::spawn` and `Thread::spawnWith` start a new thread that runs `body` concurrently with the calling thread and with all other threads; `spawnWith` passes `input` to `body` as its parameter (V-16.2). The call returns as soon as the thread has been started. The lambda and `input` belong to the new thread from then on. If the thread cannot be started, this is a program error `ThreadError`.

**R-16.3 (Join).** `join()` waits until the thread has ended. `Task<T>.join()` returns the result of `body` as `own T`; `Thread.join()` returns nothing. A handle can be joined once; a second `join()` is a program error `ThreadError`, as is a `join()` by the thread on its own handle.

**R-16.4 (Releasing a handle).** Releasing a `Thread` or `Task` that has not been joined waits until its thread has ended; a result is then released. If the thread ended with an exception (R-16.5), releasing its handle is a program error `ThreadError` whose message contains `describe()` of the exception. A handle can therefore never outlive the end of the program without its thread having ended (R-16.20).

**R-16.5 (Exceptions).** An exception that leaves `body` ends the thread after unwinding (R-9.11). `join()` throws it again in the joining thread; the exception object moves to that thread (V-16.6). In debug mode, the stack trace (R-9.16) shows the trace of the ending thread.

**R-16.6 (Program errors).** A program error in any thread cancels all other threads and ends the program after every thread has unwound (V-16.9):
- the failing thread unwinds as in R-9.14;
- every other thread is cancelled at its next cancellation point: a method call, the next iteration of a loop, or a wait in `join()`, at the release of a handle, at the end of `Thread::scope`, in `send`, `receive` or `Thread::sleep`. From there it unwinds like a program error: destructors and `finally` blocks run, no `catch` is selected;
- a thread that waits for input or is inside a C call is cancelled when that call returns; if all other threads have unwound and it is still inside such a call, it stops without unwinding;
- then the program ends with the message of the first program error on standard error and exit code 2. Further program errors during the unwinding are not reported.

## 2. Transfer between threads

**R-16.7 (Transferable types).** A type is transferable if a value of it contains no reference that another thread could use at the same time:
- primitive types, `string`, enums and handle types (Chapter 15) are transferable;
- `T?`, `T[]` and tuples are transferable if their element types are;
- a class type is transferable if no field of the class, of its base classes and of any subclass in the program has a type with `shared` or `weak` or a non-transferable type; for generic classes, with the type arguments substituted;
- a mixin type is transferable if every class in the program that imports the mixin is;
- `Sender<T>`, `Receiver<T>`, `Task<T>` and `Thread` are transferable; `Scope` and `Func<…>` are not.

Strings are shared internally; implementations count their references atomically (R-12.12).

**R-16.8 (Transferable constraint).** `Transferable` (V-16.5) is a constraint like a mixin type (R-7.23) that every transferable type satisfies. It cannot be imported and is not a type of values. A type parameter with this constraint is transferable; a type parameter without it is not.

**R-16.9 (What crosses at the start).** For `Thread::spawn` and `spawnWith`, the `own` lambda must capture only values of transferable types (R-10.8), and `D` and `T` must be transferable. An ordinary lambda or an existing function value is not accepted: `body` must be an `own` lambda expression at the call, so that its captures can be checked.

**R-16.10 (Exceptions cross).** Every class derived from `Exception` must be transferable (V-16.6). A field of type `shared`, `weak` or of a non-transferable type in such a class or in one of its subclasses is an error at the field.

## 3. Scopes

**R-16.11 (Scope).** `Thread::scope(body)` calls `body` with a new `Scope` and returns only after `body` has ended and every thread started with `Scope.spawn` has ended. `Scope.spawn(f)` starts a thread that runs `f`; it returns no handle (V-16.11).

**R-16.12 (Borrows in a scope).** The lambda passed to `Scope.spawn` must be a lambda expression at the call. It may be an `own` lambda; then R-16.9 applies. An ordinary lambda may only borrow read-only, and only variables that the lambda passed to `Thread::scope` captures; their types must be transferable. These variables count as borrowed read-only by `body` for the whole call of `Thread::scope` (R-10.7), so `body` cannot change them either. A mutable borrow, a borrow of a local variable of `body` and `this` with a non-transferable type are errors.

**R-16.13 (Exceptions in a scope).** An exception that leaves `body` or a thread of the scope does not end the other threads of the scope. When all have ended, `Thread::scope` throws the exception of `body` if there is one, otherwise that of the first thread in start order that ended with an exception; all other exceptions are released (V-16.11).

## 4. Channels

**R-16.14 (Opening).** `Channel<T>::open(capacity)` creates a channel that holds up to `capacity` values and returns its two ends. A capacity below 1 is a program error `ThreadError`. `T` must not be a nullable type (V-16.4).

**R-16.15 (Sending).** `send(value)` moves `value` into the channel and returns `true`. If the channel is full, the call waits until there is room. If the receiver has been released, `send` releases `value` and returns `false`, also while waiting.

**R-16.16 (Receiving).** `receive()` takes the oldest value out of the channel and returns it. If the channel is empty, the call waits until a value arrives. If the channel is empty and every sender has been released, `receive()` returns `null`, also while waiting.

**R-16.17 (Senders).** `clone()` returns another sender of the same channel. The values of one sender arrive in the order in which it sent them; values of different senders may arrive in any order that respects R-16.18.

## 5. Order and the end of the program

**R-16.18 (Order).** Within a thread, everything happens in program order (R-8.2). Between threads, exactly these orders hold:
- everything before `spawn`, `spawnWith` or `Scope.spawn` happens before everything in the started thread;
- everything in a thread happens before the return of its `join()`, before the release of its handle (R-16.4) and before the end of its `Thread::scope`;
- everything before a `send` happens before everything after the `receive` that takes this value;
- everything before the release of the last sender happens before everything after the `receive` that returns `null`.

Effects that are not ordered by these rules may happen in any order (V-16.8). A single call that writes to standard output or standard error, or reads from standard input, is not interleaved with another such call.

**R-16.19 (Deadlock).** If every thread of the program waits in `join()`, at the release of a handle, at the end of `Thread::scope`, in `send` or in `receive`, and none of them can continue, this is a program error `ThreadError`. `Thread::sleep` and reading input do not count as waiting (V-16.10).

**R-16.20 (End of the program).** R-12.9 applies unchanged. Because every thread belongs to a handle or a scope, and releasing the handles of `main` waits for their threads (R-16.4), the program ends only after all threads have ended, except after a program error (R-16.6).

**R-16.21 (Resources).** R-12.10 applies to every thread: each thread allows at least 10,000 nested method calls (V-12.2).

**R-16.22 (Destructors).** A destructor runs in the thread that releases the object. An object therefore never runs code in two threads at the same time.

**R-16.23 (Both modes).** Threads behave the same in debug and release mode (R-12.5). C code may be called from any thread; R-15.16 applies to each call. How an implementation maps threads onto the system is up to it: it may run each thread as an operating system thread or as a lightweight thread (coroutine) with its own stack that a scheduler of the runtime places on fewer operating system threads, as long as R-16.18 and R-16.19 hold. With lightweight threads, waiting in `join()`, `send`, `receive`, at the end of `Thread::scope`, in `Thread::sleep` and in input and output lets other threads run; a thread that computes for a long time must not keep the others from running forever. This keeps the way open to many concurrent waits without new syntax such as `async` and `await`.

## 6. Example

```
using rpp.sync.Thread;
using rpp.sync.Task;
using rpp.sync.Scope;
using rpp.sync.Channel;
using rpp.sync.Sender;
using rpp.sync.Receiver;

class Squares {
public:
    static i64 sumTo(i64 n) {
        (Sender<i64> tx, Receiver<i64> rx) = Channel<i64>::open(4);
        Task<i64> total = Thread::spawnWith(rx, own (own Receiver<i64> r) => {
            i64 sum = 0;
            i64? v = r.receive();
            while (v != null) {
                sum += v;
                v = r.receive();
            }
            return sum;
        });
        Squares::produce(tx, n);          // takes the sender and releases it at its end
        return total.join();
    }

    static void produce(own Sender<i64> tx, i64 n) {
        for (i64 i = 1; i <= n; i++) {
            discard tx.send(i * i);
        }
    }
}

i32 main(string[] args) {
    print(Squares::sumTo(10).toString());     // 385
    i64[] data = [1, 2, 3, 4];
    Thread::scope((mut Scope s) => {
        s.spawn(() => print("first " + data[0].toString()));
        s.spawn(() => print("last " + data[3].toString()));
    });
    return 0;
}
```

`sumTo` moves the receiver into the thread with `spawnWith`, because the body changes its working data (V-16.2). `produce` owns the sender; when it returns, the sender is released, the last `receive()` returns `null` (R-16.16), and the result arrives through `join()`. In `main`, both threads of the scope borrow `data` read-only; the two lines may appear in either order (R-16.18), each as a whole.

## 7. Changes to other chapters

With this chapter, the following places have changed (entered on 27.11.2026):

| Place | Change |
|---|---|
| R-4.42, R-7.23 | the built-in constraint `Transferable` (V-16.5) |
| R-9.2 | new kind `ThreadError` (V-16.7) |
| R-9.4 | classes derived from `Exception` must be transferable (V-16.6, R-16.10) |
| R-12.4, R-12.6 | several permitted behaviors with threads; output calls are not interleaved (V-16.8) |
| R-12.12 | refers to this chapter instead of "version 1 has no threads" |
| Standard library | new section `rpp.sync` with the interface of R-16.1; roadmap phase C |
| Ownership core calculus, Section 7 | reference to R-16.7 and R-16.12 |

No change to `RPP.g4`: threads, scopes and channels are classes of the standard library.

## 8. Conformance tests

The runtime tests are written so that their output does not depend on the interleaving: either only one thread writes, or the order follows from R-16.18. The test run executes every runtime test of this chapter several times.

| Rules | Accepted or runtime | Rejected |
|---|---|---|
| R-16.1 to R-16.3 | `l01_spawn_join`, `l02_task_result` | `n01_new_thread`, `n02_copy_task` |
| R-16.2, V-16.2 | `l03_spawn_with_input` | – |
| R-16.3 | `l04_join_twice` (runtime: `ThreadError`) | – |
| R-16.4 | `l05_release_waits`, `l06_release_with_exception` (runtime: `ThreadError`) | – |
| R-16.5 | `l07_join_rethrows` | – |
| R-16.6 | `l08_program_error_in_thread` (runtime: exit code 2; the destructors of the other thread run) | – |
| R-16.7 to R-16.9 | `a01_transferable_generic` | `n03_capture_shared`, `n04_field_weak`, `n05_subclass_shared`, `n06_ordinary_lambda`, `n07_generic_without_constraint` |
| R-16.10 | – | `n08_exception_with_shared` |
| R-16.11 to R-16.13 | `l09_scope_borrows`, `l10_scope_exception` | `n09_scope_mut_borrow`, `n10_scope_local_borrow`, `n11_scope_change_borrowed` |
| R-16.14 to R-16.17 | `l11_channel_order`, `l12_channel_closed`, `l13_receiver_gone`, `l14_clone_sender` | `n12_nullable_element` |
| R-16.14 | `l15_capacity_zero` (runtime: `ThreadError`) | – |
| R-16.18 | `l16_order_through_channel` | – |
| R-16.19 | `l17_deadlock` (runtime: `ThreadError`) | – |
| R-16.20 to R-16.22 | `l18_end_waits`, `l19_destructor_thread` | – |

R-16.23 cannot be checked by individual programs; it is covered by running all runtime tests of this chapter in both modes.
