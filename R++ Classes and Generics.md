# R++ Classes, Mixins and Type Parameters (Chapter 7)

[Deutsch](R%2B%2B%20Classes%20and%20Generics.de.md)

Status: 11/21/2026, Draft 0.1. This document fills Chapter 7 of the R++ Specification Outline, which regulates the structure of classes, constructors, destructors and copies, inheritance and overwriting, overloading, mixins and generic types and methods. Which types result from this and how they relate to each other is governed by Chapter 4; Ownership and Borrows Chapter 5; `export` Chapter 11.

All determinations have been decided (see R++ Design Decisions, Sections 8–10 and 20). `R-7.n` Conformance tests shall be carried out in: `conformance/ch07_classes/`.

## 1. Class

**R-7.1 (Construction).** A class consists of fields, methods, constructors, at most one destructor, at most one copy declaration `copy(…)`, mixin imports and visibility labels. A field and a method must not be called the same.

**R-7.2 (visibility).** A label `public:`, `protected:` or `private:` applies to all subsequent members until the next label. `private`.
- `public`: Visible wherever the class is visible.
- `protected`: in class and in all subclasses, also in other projects (chapter 11).
- `private`: only in the body of the class itself, but there for all its objects.

Mixin imports are outside the visibility rules; the methods of a mixin are always `public` (R-7.18).

**R-7.3 (Fields).** A field can have an initializer (`i32 count = 0;`). Initializers are evaluated in declaration order when they are created (R-7.6). `static const` Fields need a value that is fixed at compile time; there are no variable static fields.

**R-7.4 (This is).** In Methods, Constructors, Destructors and `copy` it denotes `this` the current object. In a method without `mut` it is a reading Borrow, in a `mut` method, in the constructor, in the destructor and in `copy` an alterable Borrow (chapter 5). Static methods do not have `this`.

## 2. Constructors

**R-7.5 (Constructors).** A class can have multiple constructors with different parameter lists; `new C(args)` choose one according to the rules for overloading (section 5); if a class does not have a constructor, it gets a parameterless one if each field has an initializer or is nullable; otherwise `new C()` there is an error.

**R-7.6 (End of production).** `new C(args)` proceeds in this order:
1. The base constructor: the one named with `: base(…)` or the non-parameterized base class; if the base class does not have a parameterless constructor, `: base(…)` is mandatory.
2. The field initializers of the class in declaration order.
3. The body of the constructor.

**R-7.7 (Complete initialization).** At the end of the constructor, each non-nullable field is assigned, on each path through the hull. Before this is the case, `this` only the setting and reading of fields already set may be used: no method calls to `this`, no handover of `this` (Chapter 5, V-10). Nullable fields without initializers `null`.

**R-7.8 (Virtual calls).** When `this` is fully initialized, virtual method calls in the constructor are dynamic: it runs the real class version, but the base constructor has not initialized the subclass yet; therefore, calls to `this` after R-7.7 are not possible as long as the base constructor runs.

## 3. Destructor and Copy

**R-7.9 (Destructor).** A destructor `destructor() { … }` has no parameters and is never called directly; on release, the destructors run from the actual class to the top base class, after which the fields are released (Chapter 5, V-7). Virtual methods in the destructor are dynamic, because the fields of derived classes are still alive at this time.V-11).

**R-7.10 (Copy declaration).** `copy(C other) { … }` specifies how a copy of `C` is made. `other` is a reading borrow. The body initializes `this` like a constructor (R-7.7). If the base class has a `copy`, the `copy` is executed first for the base part; if it has none, `copy` is an error in the subclass.

**R-7.11 (Copy is virtual).** `copy(a)` selects the copy declaration according to the actual class of the object; the result has the static type of `a` as a `own` value and is a complete object of the actual class; therefore, if a class has a `copy`, each subclass must have its own `copy`. If the static class of `a` has no `copy`, `copy(a)` is an error.

## 4. Inheritance and overriding

**R-7.12 (inheritance).** A class inherits from at most one class (`class A : B`); `B` is a class, not a mixin; heredity cycles are an error; the subclass inherits all fields and methods.

**R-7.13 (virtual, abstract, override).**
- Only methods with `virtual` or `abstract` can be overwritten. An overwritten method remains overwritten; a `sealed` does not exist in version 1.
- A method that overrides an inherited one or implements an abstract one carries `override`.Without `override`, a method of the same name with the same parameter types is an error.
- `override` requires the same parameter types, including ownership modes; `mut`, same `mut` Mode of the method and the same return type.`public` Remaining `public`).
- Static methods are never virtual.

**R-7.14 (Abstract classes).** A class with at least one abstract method, whether owned, inherited or as an unfulfilled mixin requirement, must: `abstract class` Abstract classes can have constructors, but not with `new` are produced.

**R-7.15 (Call for basic versions).** In a class method `base.m(…)`, the version of `m` calls from the base class and `M::m(…)` calls from the imported mixin `M`, both for `this` Outside the class and for objects other than `this`, these calls do not exist.

## 5. Overloading

**R-7.16 (Overloaded methods).** Several methods or constructors of a class may be called identical if their lists of parameters differ.
- in the return type; or
- in ownership or borrow marking of the parameters (`f(Foo x)` next to `f(own Foo x)` or `f(mut Foo x)`)

A static and an instance method may not be called the same. Inherited and imported methods count in the resolution.

**R-7.17 (Dissolution).** In the case of a call, all visible variants with the name and the appropriate number of arguments are considered, and for each argument, each variant is ranked:

| Rank | Arguments → Parameters |
|---|---|
| 0 – exactly | Same type (ownership according to Chapter 5); Literal with its standard type (`1` → `i32`, `1.0` → `f64`); `null` → nullable type; Lambda with exactly matching parameter types |
| 1 – Adjustment of the number | Literal fits into another number type; automatic number conversion (Chapter 4, R-4.6) |
| 2 – Upcast | Class → Base class or mixin (R-4.22) |
| 3 – Zero | `T` → `T?` (together with one of the above ranks counts the worse) |

If two variants of an argument have the same rank, the one with the **special** parameter type is better: the one that can be automatically converted into the other (`i32` before `i64`, `Circle` before `Shape`).

The variant chosen is the one that is worse on no argument and better on at least one than any other. If there is a tie, a non-generic variant wins against a generic one. If there is no such variant, the call is ambiguous and an error; help with `cast` or a variable of the desired type.

Examples:
- For `f(i32)` and `f(i64)` select `f(1)` `f(i32)` (exact), and `f(x)` with `x : i8` also (both number adjustment, `i32` is more specific).
- For `f(i64)` and `f(f64)` `f(1)` is ambiguous: both are number adjustment, and neither `i64` nor `f64` can be automatically converted into the other.
- For `f(Shape)` and `f(Circle)` select `f(circle)` `f(Circle)`.

## 6. Mixins

**R-7.18 (Construction).** A mixin contains instance methods with hull, requirements (methods without hull) and imports of other mixins. It has no fields, no constructors, no destructor, no `copy`, no static methods and no constants. All of its methods are `public`.A mixin may have type parameters (`mixin Equatable<T>`).

**R-7.19 (Definition test).** The hulls of a mixin are tested once, at the definition. `this` has the type of mixin; only the own methods, the requirements and the methods of imported mixins are available.

**R-7.20 (Imports).** `import M;` in a class acts as if the methods of `M` were in the class. Each requirement of `M` must be met by a method of the class: owned, inherited or from another mixin, with exactly the signature of the requirement. If one mixin imports another, its methods and requirements are passed over, and if the same mixin is imported in several ways, it counts once.

**R-7.21 (Conflict).** If two imported mixins, or a mixin and the class, bring a method with the same name and the same parameter types, this is an error, unless the class itself defines this method with `override`. In it, she can call a version specifically (`Printable::describe()`, R-7.15). Same names with different parameter types are not conflicts, but overloads.

**R-7.22 (Mixin as a type).** A mixin is a type (chapter 4), R-4.22A value of the Mixin type can be used to call up its methods and requirements; the call goes to the version of the actual class. `virtual` can be overwritten into subclasses.

## 7. Type parameters

**R-7.23 (Declaration).** Classes, mixins and methods may have type parameters. A constraint is a mixin type, `new()` or `Transferable` (R-16.8); several are linked to `&`: `class Pool<T : Comparable<T> & new()>`.

**R-7.24 (Definition test).** A generic hull shall be tested once for all type arguments; at type `T` values only:
- the methods of the mixins from the constraints,
- assignment, move and borrow in accordance with Chapter 5,
- `==` and `!=` if `T : Equatable<T>` (Chapter 4, R-4.42),
- `new T()` if `T : new()`,
- `T?` and the null tests of Chapter 6.

**R-7.25 (New()).** The type argument for a parameter with `new()` must be a non-abstract class with a visible parameterless constructor ()R-7.5Primitive Types `new()` not.

**R-7.26 (instantiation).** Whenever a generic type is used or a generic method is called, the compiler checks whether the type arguments meet the constraints. Generic types are invariant (R-4.43).

**R-7.27 (Static Members).** Static methods and constants of a generic class are addressed with type arguments: `List<Foo>::create()` A static constant must not depend on a type parameter.

**R-7.28 (Generic methods).** Type arguments of a generic method are only derived from the arguments, never written out, and if a type parameter cannot be clearly derived, this is an error.

## 8. Conformance tests

The test programs are in `conformance/ch07_classes/`; they are tested with `rppc test` (first implementation, milestone M3).

| Rules | Accepted | Rejected |
|---|---|---|
| R-7.2 Visibility | `a01_classes.rpp` | `n01_private_access_from_outside.rpp`, `n02_default_visibility_is_private.rpp` |
| R-7.5 – R-7.7 Constructors | `a01_classes.rpp` | `n03_no_default_constructor.rpp`, `n04_missing_base_call.rpp`, `n05_uninitialized_field.rpp` |
| R-7.10 R-7.11 Copy | `a03_copy.rpp` | `n06_missing_copy_in_subclass.rpp` |
| R-7.13 R-7.14 Override | `a02_inheritance.rpp` | `n07_missing_override.rpp`, `n08_non_virtual.rpp`, `n09_different_mode.rpp`, `n10_narrower_visibility.rpp`, `n11_instantiate_abstract_class.rpp` |
| R-7.16 R-7.17 Overloading | `a05_overloading.rpp` | `n12_return_type_only.rpp`, `n13_ownership_only.rpp`, `n14_ambiguous.rpp` |
| R-7.18 – R-7.21 Mixins | `a04_mixins.rpp` | `n15_mixin_conflict.rpp`, `n16_missing_requirement.rpp`, `n17_static_mixin_member.rpp` |
| R-7.23 – R-7.28 Generics | `a06_generics.rpp` | `n18_new_without_constraint.rpp`, `n19_unsatisfied_constraint.rpp`, `n20_method_without_constraint.rpp` |
