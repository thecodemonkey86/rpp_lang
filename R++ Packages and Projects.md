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
| `registry` | No | Address of own or mirrored directory instead of the central; until the service exists, a directory in the file system relative to the project directory (R-11.26) |
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

## 5a. The registry in the file system

Until the service for the central registry exists, a registry is a directory in the file system (Implementation Plan, milestone M9). R-11.19 to R-11.21 apply unchanged; R-11.26 to R-11.31 settle what they leave open.

**R-11.26 (Location and layout).** The program, that is the project being compiled, determines which registry applies: the `registry` entry in its `rpp.yaml` (a path relative to the project directory), otherwise the environment variable `RPP_REGISTRY`. It applies to all dependencies with `version`, including those of libraries; `registry` in a library has no effect. Version `v` of the library `owner.name` lives in the directory `owner.name/v` as a project with `rpp.yaml`; `name` and `version` in it must match. A file `owner.name/v.withdrawn` marks the version as withdrawn. A missing registry, library or version is an error at the dependency.

**R-11.27 (Versions and names).** A version has the form `MAJOR.MINOR.PATCH` of decimal numbers without leading zeros and with at most nine digits; this applies to `version` in every `rpp.yaml`. A minimum version in `dependencies` may omit trailing parts: `"1.4"` stands for `1.4.0`, `"1"` for `1.0.0`. Versions are compared part by part as numbers; the major version is the first part. A dependency with `version` names a library `owner.name` of exactly two ASCII identifiers that are not keywords (R-11.17). A library in the registry has no dependencies with `path`.

**R-11.28 (Selection).** A requirement (library, minimum version `m`) points to the smallest version `≥ m` that is not withdrawn and has the same major version as `m`. Starting from the requirements of the program and its local dependencies, the requirements of every version reached this way are followed in the order of the entries (breadth-first); a version that a higher one overrides still contributes its requirements. For each library, the highest version reached is selected (R-11.19). A requirement that names a different major version than the first requirement of the same library is the error. A library cannot be included both locally and from the registry, and cycles are an error in the registry too (R-11.16). The types of the selected libraries are visible only through direct entries (R-11.12).

**R-11.29 (Checksum).** The checksum of a library is computed over its directory. `rpp.yaml` and every file with the extension `.rpp` below it give one line: the SHA-256 checksum of the content as 64 lowercase hex digits, two spaces, the path relative to the directory with `/`, and an LF. Every CR LF in the content counts as LF, so the checksum does not depend on line-ending conversion at checkout. The lines are sorted by path (UTF-8 byte order) and concatenated. The checksum is `sha256:` followed by the SHA-256 checksum of this text. Other files do not count because the compiler does not read them.

**R-11.30 (rpp.lock).** The lock file lies next to the `rpp.yaml` of the program:

```yaml
libraries:
  - name: acme.json
    version: 1.4.0
    checksum: sha256:…
```

The `lock` command (`rppc lock PROJECT`, `rppcc lock PROJECT`) selects according to R-11.28 without reading an existing lock file and rewrites it with the entries sorted by name; the same input gives the same file. If an `rpp.lock` exists when compiling, each requirement gets the locked version, even if it has been withdrawn since (R-11.21); it must satisfy the minimum version and have the same major version. Errors are then: a requirement without an entry or with an entry that does not satisfy it; an entry that no requirement names; a checksum that does not match the directory (R-11.20); a lock file that does not have this form. Without `rpp.lock`, selection follows R-11.28. Lock files of libraries are not read.

**R-11.31 (Publishing and withdrawing).** `rppcc publish [--registry DIR] PROJECT` publishes a library to the given registry, otherwise to the one from the project's `registry`, otherwise to the one from `RPP_REGISTRY`. It requires the type `library`, a name `owner.name`, a valid version, no dependency with `path`, and a check without errors; a version that already exists is an error. The exported interface (exported types with their `public` and `protected` parts, R-11.10) is compared with the next lower published version. If parts are removed or changed, the major version must increase; if parts are only added, at least the minor version (R-11.21). `rpp.yaml` and the source files that the compiler loads are copied. `rppcc withdraw [--registry DIR] NAME VERSION` creates the marker from R-11.26.

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

The test programs are in `conformance/ch11_projects/`. Because chapter 11 is mainly about projects and files, many tests consist of multiple files. The registry tests (section 5a) bring their registry along as a `registry/` directory inside the test directory.

| Rules | Accepted | Rejected |
|---|---|---|
| R-11.4, R-11.7, R-11.8 | `a01_using.rpp` | `n01_using_conflict.rpp`, `n02_alias_shadowed.rpp` |
| R-11.11 | `a02_export.rpp` | `n03_export_with_internal_type.rpp` |
| R-11.14 | – | `n04_two_main_functions.rpp` |
| R-11.19, R-11.20, R-11.26, R-11.28 | `l02_registry` (runtime: the highest minimum version wins, with `rpp.lock`) | `n14_major_versions`, `n16_no_matching_version` |
| R-11.21, R-11.28, R-11.30 | `l03_withdrawn`, `l04_locked_withdrawn` (runtime) | `n18_stale_lock_file`, `n19_unneeded_entry` |
| R-11.20, R-11.29 | – | `n17_checksum` |
| R-11.26 | – | `n15_unknown_library` |
| R-11.27 | – | `n20_incomplete_version`, `n21_path_in_registry` |
| R-11.16 and R-11.12 in the registry | – | `n22_cycle_in_registry`, `n23_direct_dependencies_only` |

`publish` and `withdraw` (R-11.31) are commands, not programs; `.github/scripts/check.py registry` checks them: publishing with the rules for major and minor versions, immutable versions, withdrawing, a reproducible `rpp.lock` with which a withdrawn version still builds, and detecting a changed library.
