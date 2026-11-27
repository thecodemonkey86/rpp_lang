# R++ Packages and Projects (chapter 11)

[Deutsch](R%2B%2B%20Packages%20and%20Projects.de.md)

Status: 11.11.2026, Draft 0.1. This document fills Chapter 11 of the R++ Specification Outlines. It regulates source files and packages, visibility across package and project boundaries, the project file `rpp.yaml`, dependencies and central directory, and conditional compilation. The lexical form of the directives is in the scaffolding (chapter 2), the visibility within classes in chapter 7.

All determinations have been decided (see “R++ Design Decisions”, Sections 13, 13a, 13b and 25). `R-11.n` Conformance tests shall be carried out in: `conformance/ch11_projects/`.

## 1. Source files and packages

**R-11.1 (Source files).** A source file has the ending `.rpp` and is encoded in UTF-8, starting with an optional `package` line followed by `using` lines and type declarations (classes, mixins, enums) and not more than one `main` function. There are no header files.

**R-11.2 (Package and list).** A file in the directory `a/b` relative to a source folder begins with `package a.b;`. A file directly in the source folder does not have a `package` line and belongs to the **root package** of the project. Libraries do not have a root package (R-11.18).

**R-11.3 (Package names).** Packet names consist of identifiers separated by `.`; no part must be a keyword (`package my.package;` is an error).

**R-11.4 (Types in files).** A file can contain any number of types, and its name is free; only the directory must match the package; type names are unique within a package, even across files.

**R-11.5 (Cycles).** Packages from a project can use each other, even in a circle. The compiler reads all the files of a project before it resolves names; the order of the files does not matter.

## 2. Names and using

**R-11.6 (Full names).** Each type has a full package name and type name (`rpp.collections.Map`), and a full name can be used anywhere a type is expected.

**R-11.7 (Using).** `using a.b.C;` Makes the type `C` from the package `a.b` available in the file under the name `C`. `using X = a.b.C;` Makes it available under the name `X`. `using` Always name a type, never a whole package; placeholders like `using a.b.*;` do not exist. A `using` to a non-existent or invisible type is an error. `using` applies only in the file in which it is written.

**R-11.8 (Name resolution).** A simple name is searched in this order; the first hit applies:
1. local variables and parameters,
2. Member of the class and its base classes, including imported mixin methods,
3. type parameters,
4. Types of own package,
5. Names from `using` lines of the file,
6. Types from `rpp.core` (Chapter 13, V-13.1).

If two names (two `using` lines with the same name) fit on the same level 5, this is an error once the name is used; a `using` alias covering a type of your package is an error.

## 3. Visibility across project boundaries

**R-11.9 (Within a project).** Each type is visible in the whole of its own project, in all packages. A “package private” does not exist; within classes, the visibility labels apply (Chapter 7, R-7.2).

**R-11.10 (Exports).** `export` before `class`, `mixin` or `enum` makes a type usable in other projects where its `public` members and, for subclasses, its `protected` members are visible.

**R-11.11 (Full interface).** The signatures of the members of an exported type visible in other projects (field types, parameter and return types, base class, imported mixins, constraints) only use exported types, types from `rpp.*` and built-in types; otherwise this is an error.

**R-11.12 (Only direct dependencies).** A project can only use types from libraries that are in its `rpp.yaml`. Types from the dependencies of a library can only be used if the project itself enters them. The libraries at `rpp` are available without an entry (V-13.1).

## 4. The project file rpp.yaml

**R-11.13 (Place and entries).** Each project has in the project directory a file `rpp.yaml` with these entries:

| Entry | Obligation | Significance |
|---|---|---|
| `name` | Yes | Name of the project; for libraries in the directory `besitzer.name` (R-11.17) |
| `version` | Yes | Version after Semantic Versioning (`1.2.0`) |
| `type` | Yes | `executable` or `library` |
| `main` | at `executable` | File with `main`, relative to the project directory |
| `sources` | Yes | List of source folders |
| `dependencies` | No | List of dependencies (R-11.15) |
| `registry` | No | Address of own or mirrored directory instead of the central |
| `symbols` | No | Specific conditional compilation symbols, per mode: `debug: [ … ]` `release: [ … ]` |

Unknown entries are an error, and the file uses only the YAML forms that are necessary for these entries (mappings, lists, simple values and comments).

**R-11.14 (Maintenance).** A project of the type `executable` has exactly one function `i32 main(string[] args)` in the file which: `main` A library does not have any `main`.

**R-11.15 (Dependencies).** A dependency is either local or from the directory:

```yaml
dependencies:
  - name: mathlib            # Local dependency
    path: ../mathlib
  - name: acme.json          # Minimum version from the registry
    version: "1.4"
```

**R-11.16 (Cycle freedom).** The graph of dependencies between projects is cycle-free; a cycle is an error.

**R-11.17 (Library Names).** A library in the directory is called `besitzer.name`; the owner `rpp` is reserved; a local library may have any name that does not contain a keyword; before publishing it must have a name with owner.

**R-11.18 (Packages of a library).** All packages in a library start with their name (`acme.json.parser`), so two libraries can never contain the same package.

## 5. Versions and central directory

**R-11.19 (Dissolution).** For each library, the highest minimum version is chosen, which is required somewhere in the dependency graph (Minimal Version Selection), each library has only one version per program, and if dependencies require different main versions of the same library, this is an error.

**R-11.20 (Locked file).** `rpp.lock` in the project directory keeps the selected version and a checksum of its content for each library. If the checksum is not correct when loading, this is an error.

**R-11.21 (Publishing).** The directory rejects a version if the exported interface (R-11.10) changes incompatiblely from the previous version without increasing the main version, or if new parts are added without a new minor version. Published versions are immutable and are marked as withdrawn at most.

**R-11.22 (Interim representation).** From the source code of a library, the intermediate representation per mode and operating system is created. The directory builds the six standard combinations; if your own icons or missing variants, the compiler builds itself. Generic types of a library are generated from the intermediate representation for new type arguments.

## 6. Conditional Compilation

**R-11.23 (Directives).** A line whose first character is after void `#` is a directive: `#if B`, `#elif B`, `#else`, `#endif`. Conditions `B` consist of symbols, `!`, `&&`, `||` and brackets; `&&` binds stronger than `||`. Any other directive, even `#define`, is an error. After a directive, a line commentary may appear (`#if debug // During testing only`).

**R-11.24 (Symbols).** Assuming exactly one of `debug` and `release`, exactly one of `windows`, `linux` and `macos` as well as the own symbols from `symbols` for the selected mode. Symbols in the code cannot be defined. An unset symbol is wrong.

**R-11.25 (Evaluation).** Directives are evaluated before parsing; the rows of inactive branches are skipped and checked neither syntactically nor semantically. `#if` and `#endif` must pair within a file.

## 7. Example

```
hello/
├── rpp.yaml
├── rpp.lock
└── src/
    ├── main.rpp              // Root package containing main
    └── hello/
        └── app/
            └── Greeter.rpp   // package hello.app;
```

```yaml
name: hello
version: 0.1.0
type: executable
main: src/main.rpp
sources:
  - src
dependencies:
  - name: mathlib
    path: ../mathlib
  - name: acme.json
    version: "1.4"
symbols:
  debug: [TRACE]
  release: []
```

## 8. Conformance tests

The test programs are in `conformance/ch11_projects/` Because Chapter 11 is mainly about projects and files, many tests consist of multiple files; so far, there are only single file tests for name resolution and for files. `using`.

| Rules | Accepted | Rejected |
|---|---|---|
| R-11.4, R-11.7, R-11.8 | `a01_using.rpp` | `n01_using_conflict.rpp`, `n02_alias_shadowed.rpp` |
| R-11.11 | `a02_export.rpp` | `n03_export_with_internal_type.rpp` |
| R-11.14 | – | `n04_two_main_functions.rpp` |
