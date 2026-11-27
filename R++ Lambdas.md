# R++ Lambdas and Function Types (Chapter 10)

[Deutsch](R%2B%2B%20Lambdas.de.md)

Draft 0.1, 21 November 2026. Chapter 10 specifies lambdas, variable capture, and function types. See R-4.37 and R-4.38 for `Func`, Chapter 5 Section 7 for the mapping from captures to borrows and moves, and R-6.13 for narrowing in lambdas.

The decisions for this chapter appear in Sections 11 and 23 of the design decisions. Rule numbers follow the form `R-10.n`. Tests are in `conformance/ch10_lambdas/`.

## 1. Overview

| | Ordinary Lambda | `own` Lambda |
|---|---|---|
| Syntax | `(i32 x) => x + offset` | `own (i32 x) => x + offset` |
| Captures | By read-only or mutable borrow | By value, copying, sharing, or moving as appropriate |
| Where allowed | As a call argument or in a direct call | Anywhere a `Func` value is allowed |
| lifetime | until the end of the call | Like any `own` value |
| Typical use | `list.forEach(…)`, `sort(…)` | Store callbacks or return functions |

## 2. Lambdas

**R-10.1 (Syntax).** A lambda consists of a typed parameter list, `=>`, and a body that is either an expression or a block. An initial `own` makes it an `own` lambda. Parameters use the same markers as method parameters. The default is a read-only borrow, `mut` is a mutable borrow, and `own` takes ownership. There is no explicit capture list.

**R-10.2 (Expression syntax).** A lambda body extends to the end of the enclosing expression. Parenthesize a lambda used as an operand or called directly, as in `((i32 a) => a)(3)` (Specification Outline, Section 3.1).

**R-10.3 (Return type).** An expression-bodied lambda returns the type of its expression, including `void`. A block-bodied lambda returns the common type of its `return` values under the rules for `? :` (R-8.9). Without a value-bearing `return`, the type is `void`. If a `Func` type is expected, the compiler checks the lambda against it and types its literals using the expected return type.

**R-10.4 (Type without context).** Without an expected function type, an `own` lambda such as `var f = own (i32 x) => x + 1;` has type `Func<P₁, …, Pₙ, R>`, inferred from its parameter types and the return type in R-10.3. Ordinary lambdas can appear as call arguments or direct callees (R-10.7).

**R-10.5 (Body).** A lambda body follows the method-body rules for definite return (R-8.26), unused results (R-8.13), and exceptions (Chapter 9). `return` exits the lambda. `break` and `continue` cannot leave the lambda. A lambda cannot return a borrow (Chapter 5, V-4).

## 3. Capture

**R-10.6 (What is captured).** A lambda captures every local variable and parameter of the enclosing method that its body uses. It also captures `this` if it refers to `this` or accesses a member without an explicit receiver.

**R-10.7 (Ordinary lambdas).** An ordinary lambda captures by borrow.
- A **mutable borrow** is used if the body assigns to the variable, borrows it with `mut x`, or calls a `mut` method on it. The same applies to operations on its fields.
- Other captures are **read-only borrows**.

An ordinary lambda may appear only as an argument or in a direct call such as `((i32 v) => v + 1)(3)`. Its borrows last for the call and participate in loan checking alongside borrowed arguments (R-5.9 and R-5.10). `sort(mut list, (Foo a, Foo b) => a.x < list.length)` is an error because `list` would be borrowed both mutably and read-only at the same time.

**R-10.8 (`own` lambdas).** An `own` lambda captures by value when the lambda expression is evaluated.
- Copy primitive values.
- Share strings and `shared` values, incrementing the reference count. Copy `weak` values.
- Move `own` values into the lambda. The source variable becomes moved (R-5.3).

Later assignments to the original variable do not affect the captured value.

**R-10.9 (Borrows cannot be captured by value).** An `own` lambda cannot capture a variable holding only a borrow of an object. Examples include parameters without `own`, `ref` variables, loop elements, and variables from borrow blocks. Use `copy(x)` first to make an owned copy. This restriction leaves primitive values, strings, and `shared` values eligible for capture.

**R-10.10 (`this` in `own` lambdas).** An `own` lambda cannot use `this` directly or through an implicit member receiver because `this` is a borrow.

**R-10.11 (Immutable captures).** An `own` lambda cannot assign to a captured variable. It cannot borrow a captured `own` object mutably or call its `mut` methods, so calls leave the lambda unchanged. Captured `shared` objects retain their normal runtime-checked mutability (Chapter 5, V-1 and V-8). A lambda can hold mutable state in such an object.

**R-10.12 (Narrowing).** A captured variable that is narrowed at the lambda expression is also narrowed inside the lambda (R-6.13).

## 4. Function Types and Function Values

**R-10.13 (Func).** `Func<P₁, …, Pₙ, R>` describes a function with parameters `P₁ … Pₙ` and return type `R`. Its parameters use the markers from R-10.1. `Func<Foo, void>` borrows read-only, `Func<mut Foo, void>` borrows mutably, and `Func<own Foo, void>` takes ownership. `mut` is allowed only for parameters of `Func`. `void` is allowed only as the final type argument. Function types are structural and invariant (R-4.37).

**R-10.14 (Matching a function type).** A lambda matches `Func<P₁, …, Pₙ, R>` if it has `n` parameters with exactly the types and markers `Pᵢ`, and its return value can be converted to `R`. For `R = void`, it must return no value.

**R-10.15 (Function values).** Only `own` lambdas create owned objects of type `Func<…>`. Assignment to an owner or passing the object to an `own` parameter moves it. The object can be shared (`shared Func<i32, i32> f = …;`). `copy(f)` is an error.

**R-10.16 (Parameters of type Func).** A parameter `Func<…> f` takes an ordinary lambda, an `own` lambda, or an existing function value as a read-only borrow. The method may call `f` any number of times or pass it as an argument. It cannot store or return it. An `own Func<…> f` parameter takes ownership and accepts only `own` lambdas and function values.

**R-10.17 (Calling a function value).** Use `f(a₁, …, aₙ)` for a variable or parameter, or call a field such as `this.handler(x)`. Arguments follow the method-call rules (Chapters 5 and 8). R-8.13 requires a result to be used or explicitly discarded with `discard`.

**R-10.18 (Overloading).** A lambda argument is an exact match for an overload if it matches the parameter type under R-10.14. Otherwise the candidate is inapplicable (R-7.17).

## 5. Example

```
class Button {
private:
    Func<void>? onClick;              // Stored callback: own lambdas only
public:
    void setHandler(own Func<void> f) mut {
        this.onClick = f;
    }
    void click() {
        ref Func<void>? h = this.onClick;   // Borrow the nullable own field with ref (R-6.15)
        if (h != null) {
            h();
        }
    }
}

class Demo {
public:
    void run(mut Foo[] list, mut Button b, shared Counter c) {
        i32 limit = 10;
        i32 sum = 0;
        list.forEach((Foo f) => { if (f.x < limit) { sum += f.x; } });   // Read-only borrow of limit, mutable borrow of sum
        list.forEach((mut Foo f) => f.bump());                            // Func<mut Foo, void>
        b.setHandler(own () => { c.increment(); });                       // Shares c
        var square = own (i32 x) => x * x;                                // Func<i32, i32>
        i32 n = square(4);
    }
}
```

`click` shows the interplay with chapters 5 and 6: `Func<void>? h = this.onClick;` would be a move from the field; therefore, to call it, get the nullable field `ref` and check it. `run` In the `own` lambda changes the counter via the detected `shared` reference `c` (R-10.11).

## 6. Conformance tests

The test programs are in `conformance/ch10_lambdas/`; they are tested with `rppc test` (first implementation, milestone M3).

| Rules | Accepted | Rejected |
|---|---|---|
| R-10.3 R-10.4 Return type, `var` | `a01_basics.rpp` | `n01_non_own_lambda_in_var.rpp`, `n02_return_type_mismatch.rpp` |
| R-10.5 Tortoise | `a01_basics.rpp` | `n03_break_from_lambda.rpp` |
| R-10.7 Normal Lambda | `a02_capture.rpp` | `n04_store_non_own_lambda.rpp`, `n05_loan_conflict.rpp` |
| R-10.8 – R-10.11 `own` Lambda | `a02_capture.rpp` | `n06_borrow_by_value.rpp`, `n07_this_in_own_lambda.rpp`, `n08_mutate_captured_value.rpp`, `n09_moved_after_capture.rpp` |
| R-10.13 R-10.14 Func | `a03_function_types.rpp` | `n10_mut_on_return_type.rpp`, `n11_mut_in_other_type.rpp`, `n12_parameter_mismatch.rpp` |
| R-10.15 – R-10.17 Functional values | `a03_function_types.rpp` | `n13_copy_func.rpp`, `n14_store_parameter.rpp`, `n15_unused_result.rpp` |
