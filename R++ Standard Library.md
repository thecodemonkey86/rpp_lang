# R++ Standard Library (Chapter 13)

[Deutsch](R%2B%2B%20Standard%20Library.de.md)

As of 26.11.2026, Draft 0.10. This document fills chapter 13 of the R++ Specification Outline with a **first cut**: exactly what a compiler for R++ needs, written in R++ and self-translating (sections 2 to 7). Section 9 describes the further expansion. Sections 10 to 18 specify `rpp.random`, the vector algorithms, `rpp.math`, the text and file aids, `Deque`, `getOrPut`, `rpp.json`, synchronous processes (`rpp.process`) and SHA-256 (`rpp.hash`).

Normative in this draft are the principles (section 1), the parts on which other chapters are based (`Exception`, `Equatable`, `Comparable`, `Hashable`, `Iterable`), and the expansion packages in sections 10 to 18. The other interfaces are a work status: The libraries under `rpp` remain on version 0.x until completion of the self-translation and may change.

Principles that had to be additionally established in writing are in Section 0; they were all adopted on 21.11.2026.

## 0. Supplementary determinations (adopted on 21.11.2026)

| No | Settlement |
|---|---|
| V-13.1 | The standard library consists of several libraries under the owner `rpp`: `rpp.core`, `rpp.text`, `rpp.fmt`, `rpp.collections`, `rpp.io`, `rpp.env` They are shipped with the compiler and do not need an entry in `rpp.yaml`. The types of `rpp.core` are visible everywhere without `using`; the others are integrated with `using`. |
| V-13.2 | Iteration is deterministic: `Map` and `Set` deliver their entries in insert order, `Dir::list` delivers sorted names, so that a compiler always builds the same output when the same input is provided. |
| V-13.3 | There is no function for immediate termination (`exit`). The exit code provides `main`; this is how all destructors run. |
| V-13.4 | `print(text)` writes `text` and a line break to the standard output. |
| V-13.5 | Reading access to a missing key provides `null` (`ref V? get(K key)`); modifiable access with `at` requires an existing key and otherwise triggers a program error (such as an index outside limits). |
| V-13.6 | A Borrow method may also return a Borrow that has a `ref` variable, a loop element or another borrow call of `this` emits (e.g. from a loop over) `this.entries`). This extends R-5.23 in the surface language; the security justification is the same. |
| V-13.7 | For types with `Comparable` `<`, `<=`, `>`, `>=` are allowed and call `compareTo` (extension of Chapter 4, R-4.17, analogous to `==` and `Equatable`). |
| V-13.8 | `File::exists(path)` tells whether there is an ordinary file under `path`; `Path::absolute(p)` provides the absolute path with `/` as separator, `.` and `..` resolved purely textually (references in the file system are not tracked). Both need a compiler that loads projects. |
| V-13.9 | (adopted on 23.11.2026) Expansion principles: New packages under `rpp` supplement the library without changing the first cut (sections 2 to 7); they also remain until self-translation to 0.x (R-13.2). When porting from other languages, the translation rules in Section 9 apply: no `Optional`/`Result` (complete `T?`, `try…` and exceptions), borrowing before consuming, determinism, and equal behavior in both modes – the library does not rely on the overflow behavior of the release mode (R-4.14, R-12.5). New packets are written in R++ itself where possible and thus serve as a load test for the compiler in R++ (M6). |
| V-13.10 | (adopted on 23.11.2026) Vector algorithms that can be written in R++ occur as static generic methods of an auxiliary class (`Vectors`, section 11), not as built-in methods `T[]`. Built-in methods would have to be redrawn in the compiler in R++ (M6) and until then generate a divergence; the auxiliary class treat both compilers the same. Built-in methods remain on `T[]`, which R++ cannot efficiently express (as `sort`). Use mutable algorithms `remove`/`insert` (V-3; `swap` on two indices is always considered overlapping, Chapter 5, Section 7) and are therefore quadratic; this is sufficient for the reference. |
| V-13.11 | (adopted on 23.11.2026) `rpp.math` (section 12) provides `f64` functions. Rounds (`floor`, `ceil`, `trunc`, `round`) and `clamp` are pure R++ source text; `sqrt` is built in because IEEE 754 requires correctly rounded root pulling and is not efficiently expressable in R++ (pattern `fail("eingebaut")` as in `Fmt`, without M6 divergence). Transcendent functions (`pow`, `exp`, `log`, `sin`, `cos`) follow later; they are platform dependent and not bit-equal (section 9). |
| V-13.12 | A `IOException` carries the affected path in `path`; its text (`message`) indicates the path and describes the error; the exact wording of the description, especially the part coming from the operating system, is not fixed and may differ between implementations and systems. Programs and tests do not evaluate it. (adopted on 23.11.2026) |

## 1. Principles

**R-13.1 (Construction).** The standard library consists of the libraries in Sections 2 to 7 and the expansion libraries in Sections 10 to 18. They are under the reserved owner `rpp` (Design Decisions, Section 13b); no one else can publish at `rpp`.

**R-13.2 (Version).** Until a compiler written in R++ translates itself, the libraries carry versions 0.x under `rpp`. Their interfaces may change during this time without a new main version.

**R-13.3 (Methods on built-in types).** Primitive types and `string` have built-in methods and static members; they are listed in Sections 2 to 4 and are called at the value or with `::` on the type: `n.toString()`, `i32::parse(s)`, `i64::MAX`.

**R-13.4 (Error).** Invalid inputs that a program must expect (text that is not a number; file is missing) are reported by the libraries with exceptions. `try…`-Variation instead `null`. Preconditional violations (index, missing key at `at`) are program errors (chapter 9).

## 2. Rpp.core

Without `using` Visible everywhere ()V-13.1).

### 2.1 Exceptions

**R-13.5.** `Exception` is described in chapter 9 (R-9.4).

```
export class FormatException : Exception { … }     // Invalid text during parsing
export class IOException : Exception {             // File and directory errors
public:
    string path;
}
export class ArgumentException : Exception { … }   // Invalid argument reported as an exception
```

### 2.2 Mixins of the language

**R-13.6 (Equatable).**

```
export mixin Equatable<T> {
    nodiscard bool equals(T other);
}
```

`a == b` and `a != b` call `equals` (Chapter 4, R-4.24). Primitive types, `string` and enums without data fulfill `Equatable` by themselves.

**R-13.7 (Comparable).**

```
export mixin Comparable<T> {
    import Equatable<T>;
    nodiscard i32 compareTo(T other);          // < 0, 0, > 0
    bool less(T other) { return compareTo(other) < 0; }
}
```

For types with `Comparable` are: `<`, `<=`, `>`, `>=` Allowed and Called `compareTo` on ()V-13.7Number types, `char`, `string` and `byte` Requirements `Comparable` by itself.

**R-13.8 (Hashable).**

```
export mixin Hashable {
    nodiscard i64 hash();
}
```

Same objects (`equals`) must have the same hash values. Primitive types, `string` and enums without data fulfill `Hashable` by themselves. Hash values are stable within a program run; whether they are the same between runs remains open.

### 2.3 Iteration

**R-13.9 (Iterable).** Classes become iterable using a **cursor protocol**. The cursor is an ordinary value (e.g., a `i64` index or `shared` node), not a borrow; a borrow on an element only returns `at`.

```
export mixin Iterable<T, C> {
    nodiscard C begin();
    nodiscard bool done(C cursor);
    nodiscard C next(C cursor);
    ref T at(C cursor);
}

export mixin MutIterable<T, C> {
    import Iterable<T, C>;
    ref mut T atMut(C cursor) mut;
}
```

The mixin implements the following methods in cursor order, and the callback borrows the respective element by reading.

| Methodology | Behaviour | Empty collection |
|---|---|---|
| `any(Func<T, bool> predicate)` | Finish the search on the first hit. | `false` |
| `all(Func<T, bool> predicate)` | Finishes the test in the first counterexample. | `true` |
| `fold<A>(own A acc, Func<own A, T, A> step)` | Passes the accumulator and the element `step`. The return value becomes the next accumulator. After the last element `fold` returns the accumulator. | Starting value |
| `U[] mapNotNull<U>(Func<T, U?> transform)` | Calls `transform` once per element and collects all results except `null` in a new vector. | Empty vector |
| `Map<K, i64> countBy<K : Hashable & Equatable<K>>(Func<T, K> keySelector)` | Calls `keySelector` exactly once for each element in cursor order. The new map contains the number of its occurrences for each of the different keys according to `equals`. | Empty map |

**R-13.10 (for-each about Iterable).** `for (T x : c) S` with `c` type `Iterable<T, C>` means

```
{
    ref Coll k = c;                                   // c stays borrowed for the entire loop
    for (C cur = k.begin(); !k.done(cur); cur = k.next(cur)) {
        ref T x = k.at(cur);
        S
    }
}
```

`for (mut T x : mut c) S` requires `MutIterable<T, C>`, lends `c` modifiable and binds `ref mut T x = mut k.atMut(cur);`. `break`, `continue` and `return` in `S` function as in any loop. `c` During the loop cannot be changed (Chapter 5, sentence 5.2).

## 3. Built-in methods of primitive types and string

**R-13.11 (Numbers).** For each integer and floating point type `N`:

| Member | Significance |
|---|---|
| `N::MIN`, `N::MAX` | Minimum and maximum value (for floating point: largest finite) |
| `n.toString()` | Decimal representation; for floating point, the shortest one giving an exact reading `n` |
| `N::parse(string s)` | reads a number in the spelling of the literals (chapter 2), without delimiter `_`; throws `FormatException` if `s` is not a valid number or does not fit in `N` |
| `N::tryParse(string s)` | As `parse` but delivers `N?` (`null` instead of exception) |
| `n.abs()`, `a.min(b)`, `a.max(b)` | Amount, minimum, maximum; `abs` of `N::MIN` is an overflow (Chapter 4, R-4.14) |

Floating point text is read in exactly rounded (IEEE 754, to the next representable number).

For each integer type `N` (`i8`, `i16`, `i32`, `i64` and `ui8`, `ui16`, `ui32`, `ui64`) there are additionally the instance methods `N? tryAdd(N b)`, `N? trySub(N b)` and `N? tryMul(N b)` calculating the mathematical sum, difference or product of receiver and argument. If the result is in the closed range from `N::MIN` to `N::MAX`, they provide this value, otherwise `null` This applies equally in debug and release, even if there is a negative difference for an unsigned type.

**R-13.12 (Char).** `c.isLetter()` `c.isDigit()`, `c.isWhitespace()`, `c.isUpper()`, `c.isLower()` according to the Unicode characteristics; `c.toUpper()`, `c.toLower()`; `c.toString()`.

**R-13.13 (String).** In addition `length`, `s[i]`, `byteLength` and `byteAt` (Chapter 4, R-4.19):

| Methodology | Result |
|---|---|
| `substring(i64 start, i64 end)` | Signs from `start` to before `end` |
| `indexOf(string part)` | `i64?` Position of first occurrence or `null` |
| `startsWith`, `endsWith`, `contains` | `bool` |
| `split(string sep)` | `string[]` |
| `trim()`, `toUpper()`, `toLower()` | `string` |
| `replace(string old, string new)` | `string` all occurrences replaced |
| `bytes()` | `byte[]` UTF-8 bytes as a copy |
| `string::fromUtf8(Slice<byte> b)` | `string`; throws `FormatException` at invalid UTF-8 |
| `string::join(string sep, Slice<string> parts)` | `string` |

Strings are `Equatable`, `Comparable` and `Hashable`.

## 4. Rpp.text

`new StringView(text)` creates an immutable view of a string, which keeps the common source code alive even when the original variable is reoccupied or leaves its scope. `length()` `charAt(i)`, `indexOf(part)` and `slice(start, end)` count as `string` Unicode scalar values. The end of a range is exclusive. Invalid ranges and indices lead to one `IndexError`.

`slice` `trim()`, `trimEnd()` and `split(sep)` provide views of the same memory. Empty parts are preserved when splitting. `startsWith` and `endsWith` check the section directly. `toString()` provides its contents as a separate string.

`splitOnce(sep)` provides a nullable pair `(StringView, StringView)?`. The views include the text before and after the first hit. Additional separators remain in the right part. If there is no hit, the result is `null`; a hit at the end provides an empty right view. The empty separator hits at the beginning.

**R-13.14 (StringBuilder).** Build a string gradually:

```
export class StringBuilder {
public:
    constructor();
    nodiscard i64 length();                       // Number of characters so far
    void append(string s) mut;
    void appendChar(char c) mut;
    void appendLine(string s) mut;                // Includes a line break
    void clear() mut;
    nodiscard string toString();
}
```

## 5. Rpp.fmt

**R-13.15.** Help for text output, if it goes beyond `toString()`:

| Function | Result |
|---|---|
| `Fmt::padLeft(string s, i64 width)`, `Fmt::padRight(…)` | Filled with spaces to `width` characters |
| `Fmt::hex(i64 n)`, `Fmt::hex(ui64 n)` | hexadecimal, without prefix |
| `Fmt::fixed(f64 x, i32 digits)` | Floating point with fixed number of decimal places, correctly rounded |

## 6. Rpp.collections

Vectors (`T[]`) are built in. Their methods and the collections `Map` and `Set` are here.

**R-13.16 (Vectors).** For `T[] v`:

| Methodology | Significance |
|---|---|
| `push(own T x) mut`, `insert(i64 i, own T x) mut` | Add, insert |
| `pop() mut`, `remove(i64 i) mut` | `own T` removes the last or `i` th element |
| `clear() mut` | Release all elements |
| `ref T? first()`, `ref T? last()` | Borrow to the first or last element, `null` with empty vector |
| `contains(T x)`, `indexOf(T x)` | request `T : Equatable<T>`; `indexOf` delivers `i64?` |
| `sort() mut` | Ascending, stable; required `T : Comparable<T>` |
| `sortBy(Func<T, T, i32> cmp) mut` | Stable after `cmp` |

`pop` and `remove` on an empty vector or with invalid index are program errors.

**R-13.17 (Map).**

```
export class Map<K : Hashable & Equatable<K>, V> {
    import Iterable<Entry<K, V>, i64>;
public:
    constructor();
    nodiscard i64 length();
    nodiscard bool contains(K key);
    ref V? get(K key);                        // null if key is absent (V-13.5)
    ref mut V at(K key) mut;                  // Program error if key is absent
    void put(own K key, own V value) mut;     // Replaces the existing value and releases the old one
    V? remove(K key) mut;                     // Removes and returns the value
    void clear() mut;
}

export class Entry<K, V> {
public:
    K key;
    V value;
}
```

Entries are passed through in insert order (V-13.2); `put` with an existing key retains the position.

**R-13.18 (Set).** `Set<T : Hashable & Equatable<T>>` with `add(own T x) mut`, `contains(T x)`, `remove(T x) mut`, `clear() mut`, `length()`; iterable in insertion order.

`Set<T>` Implemented `Equatable<Set<T>>` Two sets are exactly the same if they contain the same elements, irrespective of the order of insertion.

| Operation | Elements and iteration order of the new set |
|---|---|
| `a.intersect(b)` | All elements from `a`, which are also included in `b`, in the order of `a`. |
| `a.unite(b)` | All elements from `a`, followed by the additional elements from `b` in their order. |
| `a.subtract(b)` | All elements from `a`, which are missing in `b`, in the order of `a`. |

Both operands remain intact. Membership can be changed independently in any quantity. Objects taken remain alive until their last entry is removed. For identical elements from both operands, the result retains the entry `a`.

## 7. Rpp.io and rpp.env

**R-13.19 (Files).**

```
export class File {
public:
    static own File open(string path);         // Throws IOException
    nodiscard string readAllText();            // Reads UTF-8 and throws FormatException for invalid UTF-8
    nodiscard byte[] readAllBytes();
    static void writeText(string path, string text);
    static void writeBytes(string path, Slice<byte> data);
    static bool exists(string path);           // Whether a regular file exists (V-13.8)
    destructor();                              // Closes the file
}
```

`SaveFile::open(string path)` opens a write operation in its own temporary file in the destination directory. `writeText(string text)` attaches UTF-8 text, `writeBytes(Slice<byte> data)` attaches bytes. Only `commit()` makes the complete content visible by atomic renaming on the destination path.

`cancel()` and the destructor discards an open write operation, even when a block is left by an exception; a write or commit error terminates the operation with a `IOException`. Further write or commit attempts on a completed task also throw a `IOException`. `cancel()` can be called several times.

**R-13.20 (Lists and paths).** `Dir::list(string path)` provides the names of the entries sorted (V-13.2); `Dir::exists`, `Dir::create`. `Path::join(a, b)`, `Path::fileName(p)`, `Path::extension(p)`, `Path::parent(p)`, `Path::absolute(p)` (V-13.8). Paths use `/` as a disconnector; the library converts to Windows.

`Dir::entries(string path)` provides the immediate entries as `DirEntry[]` in the same name order as `Dir::list`.Each entry contains `name`, the path `path` with the transferred directory as a basis and the file type `kind` (`FileType::File`, `Directory`, `Symlink` or `Other`). Symbolic links are of type `Symlink` regardless of destination. An empty directory gives an empty vector. A read error triggers a `IOException`.

`new Path(text)` creates an unchanging path. `toString()` provides its normalized spelling with `/`. Normalization also accepts back strokes, removes excess separators and `.` and resolves `..` purely textually. Leads `..` remain in relative paths. An empty path and `new Path()` stand for `.`. Roots follow the rules of the platform, on Windows including drives and UNC shares.

`Path? relativeTo(Path baseDir)` provides the remaining path components for a path below `baseDir` and for the same path `.`.If the path is outside the base or if the roots are different, the result is `null`. The comparison does not require file access, so `new Path("app/src/main.rpp").relativeTo(new Path("app"))` gives the path `src/main.rpp`.

**R-13.21 (Console).** `print(text)` writes `text` with line break to the standard output (V-13.4). `Console::write(text)` without line break, `Console::error(text)` to the error output, `Console::readLine()` delivers `string?` (`null` at the end of the input).

**R-13.22 (Environment).** `Env::get(string name)` delivers `string?`. `Env::os()` delivers `Os::Windows`, `Os::Linux` or `Os::MacOS`. There is no `exit` (V-13.3).

**Command lines in rpp.env.** `new CommandLineParser(befehl, beschreibung)` declares a command. `addPositional(name)` adds a duty argument, `addFlag("--name", hilfetext)` a switch without value. `helpText()` generates the help from this information.

`parse(Slice<string> args)` expects the arguments after the command name and delivers a new one `CommandLineResult`; if successful, `positional` contains the values in call order, `isSet("--name")` checks a switch. `-h` and `--help` set up `helpRequested`. After `--` all other words are considered positional arguments. `error` The output and the exit code determine the calling program.

`new ElapsedTimer()` from `rpp.env` starts a monotonous stopwatch. `elapsed()` provides the elapsed time in whole milliseconds as `i64`. `start()` resets the starting point. `restart()` provides the duration of the completed interval and uses the same measuring point as the beginning of the next one. Changes in the date, time or time zone do not affect the measurement. `0`.

## 8. What a self-translating compiler covers with it

| Task in the compiler | Library |
|---|---|
| Find and read source files | `Dir::list`, `File::open`, `readAllText` |
| Lexa | `string` accesses, `char.isLetter()` etc., `i64::parse`, `f64::parse` |
| Symbol and type tables | `Map`, `Set`, `Hashable` |
| Error messages | `StringBuilder`, `toString()`, `Console::error` |
| Issuance of the interim representation | `File::writeBytes`, `Fmt` |
| `rpp.yaml` Reading | a small own parser for the required YAML subset in the compiler, not in the standard library |

## 9. Extension beyond the first cut (working position)

The first cut remains unchanged until the compiler translates itself, and the library grows in packages under `rpp` (V-13.9). This section describes the status of work.

**Translation rules when porting from other languages.** It is not the API that is copied, but the concept adapted to R++:

| Model concept | R++ correspondence |
|---|---|
| `Option`/`Maybe`/`Optional` (Rust, Swift, Java) | Do not port: `T?` with flow analysis (Chapter 6) |
| `Result<T, E>` (Rust) | Do not port: Exceptions for expected errors, `try…` provide variants `null` (R-13.4) |
| `into_iter` vs. `iter` (Rust) | borrowing methods (`ref`); using consumable variants `own` – the ownership test enforces correctness |
| Lazy Iterators / LINQ / Kotlin Sequences | Adapters over the cursor protocol `Iterable<T, C>`; they hold a borrow on the source, lifetime = block (R-5.17) |
| RAII / `use` / `defer` (C++, Kotlin, Go) | Destructors; pattern is `SaveFile` (R-13.19) |
| GC-dependent APIs (Finalizer queues, cyclic `WeakRef`s) | Do not port: no garbage collector |
| Determinism | Continue (V-13.2): insertion order, stable sorting, seedable random |

**Roadmap.** Phase A runs alongside M6 and can be written in R++; phase B follows after self-translation; phase C only with the thread library (version 1 is simply threaded, design decisions section 12).

| Phase phase | Package | Content | Models |
|---|---|---|---|
| A | `rpp.collections` (upgrading) | `Vectors` (Section 11), `Deque` and `Map.getOrPut` (Section 15), **defined**; open: Views (`chunked`, `windowed`, `zip`) | Rust `Vec`, Go `slices`, C++ `<algorithm>`, Kotlin, Python |
| A | `rpp.math` | `sqrt` `floor`/`ceil`/`trunc`/`round`, `clamp`, `PI`/`E` (Section 12, **defined**); open: `pow`, `exp`, `log`, `sin`, `cos` (platform dependent) | Rust `f64`, Java `Math` |
| A | `rpp.random` | Seedable random, `shuffle` (Section 10, **set**) | Rust `rand`, Go `math/rand` |
| A | `rpp.text` (upgrading) | `Text`: `lines`, `removePrefix`/`removeSuffix`, `repeat`, `splitLimit`, `isBlank`, `substringBefore`/`substringAfter`, `capitalize`, `toUpperCase`/`toLowerCase` (Section 13, **defined**) | Kotlin, Python, Java, C# |
| A | `rpp.io` (upgrading) | File operations and buffered reading with `TextReader` (Section 14, **specified**) Open is the change time (needs) `rpp.time`). | Java `BufferedReader`, Go `os` |
| B | `rpp.iter` | `map` `filter`, `take` about `Slice<T>` (Section 13, **determined**); open: `flatMap`, `chain` | Rust Iterator, Kotlin Sequence, C++20 ranges |
| B | `rpp.regex` | linear time, no catastrophic backtracking | RE2 (Go) |
| B | `rpp.json` | Typical tree, `parse` and `stringify`, parse error with row and column (section 16, **set**). | Go `encoding/json`, Rust `serde_json` |
| B | `rpp.time` | `Duration` monotone `Instant` (upgrading of `ElapsedTimer`) | Rust `std::time`, Java `java.time` |
| B | `rpp.encoding` | Base64, Hex, UTF-16 | Go, Rust |
| B | `rpp.hash` | SHA-256 for the checksums in `rpp.lock` (section 18, **specified**, M9); open: further algorithms | Rust `sha2`, Go `crypto/sha256` |
| C | `rpp.sync` | `Thread`, `Task`, `Scope`, channels; values move by `own` through the channel (section 19, **specified**, M11) | Go, Rust |
| A | `rpp.process` | synchronous `Process::run` with input and output as text (section 17, **defined**, used for `rppcc build` and `rppcc test`, M8) | Go `os/exec`, Rust `std::process` |
| C | `rpp.process` (upgrading), `rpp.net` | concurrent subprocesses with streams; HTTP client (for the central directory) | Go `os/exec`, Rust `reqwest` |

Consciously not ported `Optional`/`Result` types, reflection, expression trees and `exit` (V-13.3). Transcendent functions in `rpp.math` are not bit-like across platforms; the determinism promise (V-13.2) applies to them only within one platform.

## 10. Rpp.random

Seedable pseudo-random, deterministic and the same in both modes (V-13.9). Incorporated with `using rpp.random.Random;`. The algorithm is defined so that each implementation delivers the same sequence; thus, the FR++ fuzzing can also be ported reproducibly to R++.

**R-13.23 (Random).**

```
export class Random {
public:
    constructor(ui64 seed);
    nodiscard ui64 nextUi64() mut;
    nodiscard i64 nextInt(i64 bound) mut;
    nodiscard f64 nextFloat() mut;
    nodiscard bool nextBool() mut;
    void shuffle<T>(mut T[] v) mut;
}
```

The condition is four `ui64`-Words `s0` up to `s3` All arithmetic in this package counts modulo 264; the library avoids overflow ()R-4.14), so that debug and release modes show the same behavior (R-12.5). Sliding is logical; slid bits are omitted (R-4.16). `rotl(x, k)` is `(x << k) | (x >> (64 - k))`.

**Seeding (SplitMix64).** Excise `seed` The four state words are created: `z = seed`; then per word:

```
z = (z + 0x9E3779B97F4A7C15) mod 2⁶⁴
z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) mod 2⁶⁴
z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) mod 2⁶⁴
sᵢ = z ^ (z >> 31)
```

`seed = 0` The state is never completely zero: the constant is odd, and a word can only be used for a single `z`-value become zero.

**Step (`xoshiro256**`).** `nextUi64()` returns

```
result = (rotl((s1 * 5) mod 2⁶⁴, 7) * 9) mod 2⁶⁴
t = (s1 << 17) mod 2⁶⁴
s2 ^= s0;  s3 ^= s1;  s1 ^= s2;  s0 ^= s3;  s2 ^= t
s3 = rotl(s3, 45)
```

and gives `result` Same seed is the same sequence, on each platform and in both modes.

**R-13.24 (Drawing numbers).**

| Methodology | Behaviour |
|---|---|
| `nextUi64()` | the nearest 64-bit value after R-13.23 |
| `nextInt(bound)` | is equal in `0` to `bound − 1`, without modulo-bias. `bound ≤ 0` is a program error (`FailError`); with `b = cast<ui64>(bound)` and `threshold = ((ui64::MAX % b) + 1) % b` is drawn `x = nextUi64()` until `x ≥ threshold`; the result is `cast<i64>(x % b)`. Expect a maximum of two trains. |
| `nextFloat()` | `cast<f64>(nextUi64() >> 11) / 2⁵³`, evenly distributed in `[0, 1)` with 53 bit resolution |
| `nextBool()` | `(nextUi64() >> 63) == 1` The top bit of the drawn number |

**R-13.25 (Mixing).** `shuffle<T>(mut T[] v)` mixes a vector with Fisher-Yates from rear to front: for `i` from `v.length − 1` to `1`, draw `j = nextInt(i + 1)` and change places `i` and `j`. An empty vector remains unchanged. The result is a permutation; with the same seed, the same order.

The exchange is done by taking out and putting in (`remove`/`insert`, R-13.16), not by exchanging two indices: it is not moved out of elements (V-3), and `swap(mut v[i], mut v[j])` is always considered to be overlapping (Chapter 5, Section 7). The elements are moved; `shuffle` works for any element type, including `own` objects. Fixed arrays cannot be mixed in version 1 because a slice must not change the length (set 5.2).

## 11. Rpp.collections: vector algorithms (upgrading)

Static generic methods of the class `Vectors`, incorporated with `using rpp.collections.Vectors;` Pure R++ source text (V-13.9, V-13.10); the methods complement the built-in vector methods (R-13.16). They stand up as auxiliary classes instead of built-in methods `T[]`, so that both compilers treat them equally (V-13.10).

**R-13.26 (Vectors, Search and Compare).**

```
export class Vectors {
public:
    static i64? binarySearch<T : Comparable<T>>(Slice<T> s, T x);
    static i64 lowerBound<T : Comparable<T>>(Slice<T> s, T x);
    static i64 upperBound<T : Comparable<T>>(Slice<T> s, T x);
    static i64? findIndex<T>(Slice<T> s, Func<T, bool> predicate);
    static i64? findLastIndex<T>(Slice<T> s, Func<T, bool> predicate);
    static bool equals<T : Equatable<T>>(Slice<T> a, Slice<T> b);
    static void reverse<T>(mut T[] v);
    static void retain<T>(mut T[] v, Func<T, bool> keep);
    static void dedup<T : Equatable<T>>(mut T[] v);
}
```

| Methodology | Behaviour |
|---|---|
| `binarySearch(s, x)` | binary search in the **ascendingly sorted** slice `s` (Comparable); provides the position of a hit or `null`. Empty slice: `null`. |
| `lowerBound(s, x)` | First index in the ascending slice with `s[i].compareTo(x) >= 0`, otherwise `s.length`. |
| `upperBound(s, x)` | First index in the ascending slice with `s[i].compareTo(x) > 0`, otherwise `s.length`. |
| `findIndex(s, predicate)` | Search the slice in ascending index order and provide the smallest matching index or `null`. |
| `findLastIndex(s, predicate)` | Search the slice in descending index order and provide the largest matching index or `null`. |
| `equals(a, b)` | Elemental comparison of two slices (equatable); `true` with the same length and the same elements in the same order. fills the gap that arrays do not have a `==` (V-4.8). |

Both limits are relative to the slice and require logarithmically many comparisons. `0` All with `x` equally comparative elements lie in the semi-open region `[lowerBound(s, x), upperBound(s, x))`. Missing `x`, both provide the same insertion position.

`findIndex` and `findLastIndex` count indices from the beginning of the handed slice, both calling up the predicate exactly once for each item visited and ending the search immediately as soon as it delivers `true`. With an empty slice, both deliver `null`, without calling the predicate.

**R-13.27 (Changeable algorithms).** `reverse` `retain` and `dedup` change a vector in place:

| Methodology | Behaviour |
|---|---|
| `reverse(v)` | reverses the order; an empty vector and one with an element remain unchanged |
| `retain(v, keep)` | retains only those elements for which `keep` is true; the others are released; the order of those retained remains. |
| `dedup(v)` | removed **consecutive** duplicates (equatable); non-consecutive identical elements are preserved |

All three are working on `remove`/`insert` (R-13.16): From elements is not moved (V-3), and `swap` on two indices is always considered to be overlapping (Chapter 5, Section 7); the elements are moved; the methods work for each element type, including for `own`Objects. Since each shift is linear, they need time proportional to the square of the length; this is sufficient for the reference interpreter. A slice must not change its length (sentence 5.2), therefore the methods take `mut T[]` and not `mut Slice<T>`; fixed arrays cannot be changed in this way.

The constraints are examined at the call (R-7.26). `binarySearch`, `lowerBound` and `upperBound` request `Comparable`, `equals` and `dedup` request `Equatable`.

**R-13.37 (Transcendents).** `Math.pow(f64 x, f64 y)` `Math.exp(f64 x)`, `Math.log(f64 x)`, `Math.sin(f64 x)`, `Math.cos(f64 x)` and `Math.tan(f64 x)` are present as built-in (`fail("eingebaut")`, Dispatch in `stdlib.rs` above `rpp.math.Math`); the results are platform-dependent and not bit-like, deterministic within a platform; both modes run the same.

**R-13.38 (Counting).** `Vectors.count<T : Equatable<T>>(Slice<T> s, T x)` The number of occurrences of `x` in `s`; an empty slice results `0` Conditions: `T : Equatable<T>` (R-7.26).

**R-13.39 (Predicates).** `Vectors.any<T>(Slice<T> s, Func<T, bool> predicate)` `Vectors.all<T>(…)` and `Vectors.none<T>(…)` test a slice for a predicate; `any` terminates at the first true result, `all` provides the judgment of all elements. For an empty slice: `any` → `false`, `all` → `true`, `none` → `true`.

## 12. Rpp.math

Floating point aids for: `f64`, integrated with `using rpp.math.Math;` The rounding and limiting functions are pure R++ source text, `sqrt` is installed ()V-13.11All functions behave the same in both modes ()V-13.9). `abs` `min` and `max` already exist as methods of number types (R-13.11) and are not repeated here.

**R-13.28 (Math, Constants and Rounds).**

```
export class Math {
public:
    static const f64 PI;
    static const f64 E;
    static f64 sqrt(f64 x);
    static f64 floor(f64 x);
    static f64 ceil(f64 x);
    static f64 trunc(f64 x);
    static f64 round(f64 x);
    static f64 clamp(f64 x, f64 lo, f64 hi);
}
```

`PI` and `E` are the `f64` values closest to π and e.

| Function | Behaviour |
|---|---|
| `sqrt(x)` | correctly rounded square root (IEEE 754). `x < 0`: NaN. `sqrt(±0) = ±0`, `sqrt(+∞) = +∞`, `sqrt(NaN) = NaN`. |
| `floor(x)` | Maximum integer ≤ x |
| `ceil(x)` | Minimum integer ≥ x |
| `trunc(x)` | Integer component direction zero |
| `round(x)` | next integer; away from zero at exactly .5 (2.5 → 3, −2.5 → −3) |

The rounding functions provide exact integer values; for all of them, NaN remains NaN, ±∞ remains ±∞, and ±0 remains ±0 (the sign of zero is retained).

**R-13.29 (clamp).** `clamp(x, lo, hi)` limits x to the interval `[lo, hi]`: smaller than lo → lo, larger than hi → hi, otherwise x. `lo <= hi` is required; `lo > hi` is a program error (`FailError`).

Transcendent functions (`pow`, `exp`, `log`, `sin`, `cos`, `tan`) are defined by R-13.37 (section 12); the result is platform dependent and not bit-like.

## 13. Rpp.text: Text helps (extension)

Static methods of the class `Text` on `string`, integrated with `using rpp.text.Text;` Pure R++ source text (V-13.9, V-13.10) following the pattern of `Fmt` (R-13.15); they complement the built-in string methods (R-13.13), without changing the first cut.

**R-13.30 (Text text).**

```
export class Text {
public:
    static string[] lines(string s);
    static string removePrefix(string s, string prefix);
    static string removeSuffix(string s, string suffix);
    static string repeat(string s, i64 n);
    static string[] splitLimit(string s, string sep, i64 limit);
}
```

| Methodology | Behaviour |
|---|---|
| `lines(s)` | Divided s into rows. Separator is `\n`; one `\r` immediately preceding it is omitted (CRLF). Blank rows remain; a final `\n` does not produce an additional empty row. For `""`, the result is an empty vector. |
| `removePrefix(s, prefix)` | removes prefix from the beginning if s starts with it; otherwise, s remains unchanged. An empty prefix leaves s unchanged. |
| `removeSuffix(s, suffix)` | removes suffix from the end if s ends with it; otherwise s remains unchanged. An empty suffix leaves s unchanged. |
| `repeat(s, n)` | `n >= 0` is required; `n < 0` is a program error (`FailError`). `n = 0` or an empty s is given `""`. |
| `splitLimit(s, sep, limit)` | divides s to sep into maximum limit parts; the last part contains the undivided remainder. `limit >= 1` and a non-empty sep are required, otherwise program errors (`FailError`). `limit = 1` results `[s]`; if the partial number is less than limit, all parts are supplied. Empty parts remain. |

`lines` and `splitLimit` provide new vectors; the other methods return immutable strings (strings are divided implicitly, R-4.18).

**R-13.40 (Text aids).**

```
export class Text {
public:
    static bool isBlank(string s);
    static string substringBefore(string s, string delimiter);
    static string substringAfter(string s, string delimiter);
    static string capitalize(string s);
    static string toUpperCase(string s);
    static string toLowerCase(string s);
}
```

| Methodology | Behaviour |
|---|---|
| `isBlank(s)` | delivers `true`, if s is empty or consists only of whitespace; otherwise `false`. |
| `substringBefore(s, delimiter)` | provides the part of s before the first occurrence of delimiter; if delimiter is missing, s remains unchanged. |
| `substringAfter(s, delimiter)` | delivers the part of s after the first occurrence of delimiter; if delimiter is missing, the part is empty. |
| `capitalize(s)` | delivers s with first letter large and the rest small; with empty s, s remains empty. |
| `toUpperCase(s)` | It gives s in capital letters. |
| `toLowerCase(s)` | It gives s in lower case. |

`substringBefore` and `substringAfter` are implemented above `splitLimit` with `limit = 2`; an empty delimiter or `limit < 1` is, as in `splitLimit`, a program error (`FailError`). `isBlank` checks over `trim`.

**R-13.41 (Math supplements).**

```
export class Math {
public:
    static f64 log10(f64 x);
    static f64 hypot(f64 x, f64 y);
    static f64 atan2(f64 y, f64 x);
    static f64 sinh(f64 x);
    static f64 cosh(f64 x);
    static f64 tanh(f64 x);
    static f64 toDegrees(f64 x);
    static f64 toRadians(f64 x);
}
```

| Methodology | Behaviour |
|---|---|
| `log10(x)` | Logarithm to base 10; platform-dependent, not bit-equal (R-13.37). |
| `hypot(x, y)` | Length of the hypotenuse; platform-dependent, not bit-equal. |
| `atan2(y, x)` | Arcu rod sinus; platform dependent, not bit-equal. |
| `sinh(x)`, `cosh(x)`, `tanh(x)` | Hyperbolic functions; platform-dependent, not bit-equal. |
| `toDegrees(x)` | Conversion degree → Radiant: `x * 180 / PI`. |
| `toRadians(x)` | Conversion radians → degrees: `x * PI / 180`. |

The transcendents (`log10`, `hypot`, `atan2`, `sinh`, `cosh`, `tanh`) are present as incorporated (`fail("eingebaut")`, Dispatch in `stdlib.rs` above `rpp.math.Math`); `toDegrees` and `toRadians` are pure R++ source text (arithmetic above `Math::PI`).

**R-13.42 (Lazy iterators).**

```
export class Iter {
public:
    static shared T[] map<T>(Slice<shared T> source, Func<shared T, shared T> f);
    static shared T[] filter<T>(Slice<shared T> source, Func<shared T, bool> p);
    static shared T[] take<T>(Slice<shared T> source, i64 n);
    static shared T[] flatMap<T>(Slice<shared T> source, Func<shared T, shared T[]> f);
    static shared T[] chain<T>(Slice<shared T> source, Slice<shared T> second);
}
```

The elements are: `shared T`: A result shares it with the source (counter + 1,) V-9And instead of moving them out of it.R-5.3. With `Slice<T>` and `T[]` would be any `push(source[i])` a move from one element and rejected. Each method yields a new vector; the evaluation is not delayed. For basic values and `string` does not change `shared`. A lambda for `flatMap` yields `shared T[]` (vectors distinguish the shape of their elements, R-4.31).

| Methodology | Behaviour |
|---|---|
| `map(source, f)` | delivers a new element per source element under `f`; order and length are preserved. |
| `filter(source, p)` | It is the source elements for which `p` is true. |
| `take(source, n)` | provides the first `n` source elements (n does not exceed the length). |
| `flatMap(source, f)` | concatenates the sequence of each element (`f` provides an array); length is obtained from the subsets. |
| `chain(source, second)` | Source followed by `second`; the elements of both sources in order. |

Porte: Rust `Iterator`, Kotlin Sequence, C++20 ranges.

## 14. Rpp.io: File operations (extension)

The class `File` (R-13.19) receives additional static methods for file operations on paths. They are built-in (`fail("eingebaut")`, file system access as in `File`/`Dir`); as an R++ class, both compilers treat the signatures the same, no M6 divergence (V-13.10) arises. Throwing errors `IOException` (R-13.4).

**R-13.31 (File operations).** In addition to R-13.19:

```
static void appendText(string path, string text);
static i64 copyFile(string from, string to);
static void moveFile(string from, string to);
static void delete(string path);
static i64 size(string path);
static own TextReader openTextReader(string path);
```

| Methodology | Behaviour |
|---|---|
| `appendText(path, text)` | attaches text as UTF-8 to the file; a missing file is created. `IOException` for spelling errors. |
| `copyFile(from, to)` | copies the file from nach to (an existing to is replaced) and provides the number of copied bytes. `IOException`, if from is missing. |
| `moveFile(from, to)` | Names from after to (an existing to be replaced). `IOException`, if from is missing. |
| `delete(path)` | remove the file. `IOException`, if it is missing. |
| `size(path)` | File size in bytes. `IOException`, if it is missing. |
| `openTextReader(path)` | Opens the file for buffered reading of UTF-8. Each call provides its own reader at the top of the file. `IOException` with the affected path. |

`copy` is a keyword (chapter 7), therefore the methods are called `copyFile` and `moveFile`. use paths `/` as separators; the library converts to Windows (R-13.20).

`rpp.io.TextReader` keeps the file open until the reader is destroyed. `nodiscard string? readLine() mut` Read the next line. `Text::lines` (R-13.30) LF separates the lines. One CR immediately before LF is omitted, every other CR remains, even at the end of the file. A blank line gives `""`. After the last line and an empty file, each one provides a further call `null`. A final LF does not generate an additional line. Invalid UTF-8 throws a `FormatException` when reading the affected line.

The change time (Mtime) remains open until a time representation from `rpp.time` is available (Section 9, Phase B).

## 15. Rpp.collections: Deque and getOrPut (extension)

**R-13.32 (Dequeque).** `Deque<T>` is a double-sided queue, integrated with `using rpp.collections.Deque;` Pure R++ source text (V-13.9), vector-based and iterable.

```
export class Deque<T> {
    import Iterable<T, i64>;
public:
    constructor();
    nodiscard i64 length();
    nodiscard bool isEmpty();
    void pushBack(own T x) mut;
    void pushFront(own T x) mut;
    own T popBack() mut;
    own T popFront() mut;
    ref T? front();
    ref T? back();
    void clear() mut;
}
```

| Methodology | Behaviour |
|---|---|
| `pushBack(x)` / `pushFront(x)` | hangs x at the rear or front (takes over `own T`) |
| `popBack()` / `popFront()` | Removes and delivers the rear or front element (`own T`) and a program error (`IndexError`) on an empty deque. |
| `front()` / `back()` | Borrow on the front or rear element, `null` with empty deque |
| `length()` / `isEmpty()` / `clear()` | Number, blank test, release all elements |

The iteration (`Iterable<T, i64>`) runs from front to back in insert order. The elements are moved; `Deque` works for any element type, including `own` objects. The reference is vector-based: operations at the beginning (`pushFront`, `popFront`) require linear time (V-13.10); a constant-time ring buffer would be a later built-in variant.

**R-13.33 (getOrPut).** `Map<K, V>` (R-13.17) is given:

```
ref mut V getOrPut(own K key, Func<V> factory) mut;
```

If the key is missing, `factory()` is called, the value is inserted and its borrow returned; the factory is called **only** in the missing case. The return value is a variable borrow (`ref mut V`). `getOrPut` is built-in (`fail("eingebaut")`) for it to work for `own` keys as well – a pure R++ version would have to reuse the key after `put`, which is not possible with shifted `own` keys. As an R++ method, both compilers treat the signature the same way (V-13.10).

## 16. Rpp.json

**R-13.34 (JSON tree).** A `Json` node has exactly one of the six types `JsonKind`. An array has its elements, an object has its field values. `append` and `addMember` take ownership of the transferred node. The field names are unique. `addMember` throws one `ArgumentException` at an existing key and leaves the object unchanged.

```cpp
export enum JsonKind { Null, Boolean, Number, String, Array, Object }

export class JsonParseException : FormatException {
public:
    i64 line;
    i64 column;
    constructor(string message, i64 line, i64 column);
}

export class Json {
public:
    nodiscard static Json parse(string text);
    nodiscard static string stringify(Json value);
    nodiscard static Json newNull();
    nodiscard static Json fromBoolean(bool boolean);
    nodiscard static Json fromNumber(string text);
    nodiscard static Json fromString(string text);
    nodiscard static Json newArray();
    nodiscard static Json newObject();
    nodiscard JsonKind kind();
    nodiscard bool asBoolean();
    nodiscard string asNumberText();
    nodiscard string asString();
    nodiscard i64 length();
    ref Json at(i64 index);
    ref Json? get(string key);
    nodiscard string keyAt(i64 index);
    void append(own Json value) mut;
    void addMember(string key, own Json value) mut;
}
```

`length()` Counts the elements of an array or the fields of an object. `at(index)` Provides a borrow on an array element. `keyAt(index)` Provides the field name at the specified position in insert order. Both indices start at 0. `get(key)` returns a borrow to the field value or R++-`null` if the key is missing; an existing field with JSON-`null` returns a node of the type `JsonKind::Null`. Keys are compared according to the decoded string contents. Calls for the wrong type of node are program errors. Invalid indices lead to a `IndexError`.

Numbers store their valid JSON numerical text including sign, decimal places and exponent. `fromNumber(text)` checks exactly one number according to R-13.35 without surrounding void space and throws it `JsonParseException`. `asNumberText()` provides the original spelling, which means that numbers outside of the built-in R++ number types are also exactly preserved. Callers can convert the text with `tryParse` into a matching R++ number type if necessary.

**R-13.35 (Reading).** `Json::parse` reads exactly one JSON value from an R++`string`. Any node type may be at the root. Outside strings, spaces, tabs, CR and LF are considered to be voids. Literals `null`, `true` and `false`. Arrays separate values with commas, objects separate fields with commas and set a colon between the key written as a string and its value, both containers may be empty, a final comma, a duplicate key, a comment, a BOM or other text after the value are syntax errors.

```text
number = '-'? ('0' | [1-9][0-9]*) ('.' [0-9]+)? ([eE] [+-]? [0-9]+)?
```

Strings are in double quotes. Allowed escapes are `\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t` and `\u` with exactly four hexadecimal digits. A high surrogate must be followed immediately by a low surrogate written as `\u`. The pair yields a Unicode scalar value, and individual surrogates and unmasked control characters from U+0000 to U+001F are syntax errors.

A syntax error flips `JsonParseException`. `line` and `column` begin at 1 and denote the point of failure in Unicode scalar values. CR, LF and CRLF each count as a line break. In the case of a double key, it points to the opening quotation mark of the second key.

Up to 128 simultaneously open arrays and objects are allowed. A container at the root counts as one. Scalars do not increase the nesting depth. `parse` With the 129th opening container throws a `JsonParseException` at its opening bracket.

**R-13.36 (Letters).** `Json::stringify` outputs compact JSON. Arrays follow the order of elements, objects follow the order of insertion, numbers are output in their stored spelling, strings and keys are placed in quotation marks. Quotations and re-strokes shall be masked; control characters from U+0000 to U+001F shall appear as `\u00xx` with lower case hex digits; the remaining Unicode scalar values shall appear directly; if the nesting depth specified in R-13.35 is exceeded `stringify`, a `ArgumentException` shall be thrown.

## 17. Rpp.process: Processes (extension)

A compiler that generates C and calls a C compiler must be able to start other programs and find its own files (implementation plan, M8). This section sets a **synchronous** form for this: the caller waits until the process ends. `run` collects the outputs, `call` passes through the console. Concurrent processes with streams follow only with the thread library (section 9, phase C). The methods are built-in (`fail("eingebaut")`, as `File` and `Dir`); as R++ classes, both compilers treat the signatures equally (V-13.10).

**R-13.43 (Processes).** `rpp.process` contains:

```
export class ProcessResult {
public:
    i32 exitCode;
    string output;
    string errorOutput;
    constructor(i32 exitCode, string output, string errorOutput);
}

export class Process {
public:
    static own ProcessResult run(string program, string[] args, string input, string? workDir);
    static i32 call(string program, string[] args, string? workDir);
}
```

`run` starts `program` with the arguments `args`, gives it `input` as UTF-8 on the standard input, waits for its end and provides exit code, standard output (`output`) and error output (`errorOutput`). The following shall apply:

| Point | Settlement |
|---|---|
| Program | A name without a path separator is searched in the search path of the system ()`PATH`; under Windows may `.exe` A path with disconnector applies relative to the caller's working directory, not to `workDir`. |
| Arguments | They reach the program unchanged as individual arguments; there is no shell involved. On Windows, they are written to the command line according to the rules of the C runtime of Windows. |
| Working list | `workDir` in the case of `null` that of the caller. the environment variables are inherited. |
| Exit code | the exit code of the process; it ends with a signal (POSIX), `128` plus the signal number |
| Expenditure | Standard and error outputs are fully collected, even if they are large; the call does not jam; bytes that are not valid UTF-8 are replaced by U+FFFD; the number of substitute characters created is not specified. |
| `call` | like `run`, but the process inherits the caller's default input, output and error output; only the exit code comes back. |
| Error | If the process cannot be started (program not found, `workDir` Missing, tosses `run` one of the following: `IOException` with `program` as a path ()V-13.12An exit code unequal to 0 is not an error of `run`. |

**R-13.44 (Program path).** `rpp.env.Env` receives `static string executablePath()`: the absolute path of the running program with `/` as separator (R-13.20).In debug mode with interpreter, this is the interpreter, in release mode the translated program.

**R-13.45 (Remove the list).** `rpp.io.Dir` receives `static void removeAll(string path)`: It removes the directory `path` with all its contents. Symbolic links and directory references in the tree are themselves removed, their destination remains untouched. If `path` there is no directory (is missing, is a file or a reference) or cannot be removed, `removeAll` throws a `IOException` with the path.

## 18. rpp.hash: SHA-256 (extension)

The checksums of the lock file (chapter 11, R-11.29) need SHA-256 (Implementation Plan, M9). The class is written in R++, not built in: the 32-bit words are held in `ui64` and masked to 32 bits after every operation, so no overflow ever occurs (R-4.14) and both modes compute the same result.

**R-13.46 (SHA-256).** `rpp.hash` contains:

```
export class Sha256 {
public:
    constructor();
    void update(Slice<byte> data) mut;
    void updateText(string text) mut;
    nodiscard string finishHex() mut;
    static string hex(Slice<byte> data);
    static string hexText(string text);
}
```

`update` appends bytes to the input, `updateText` the UTF-8 encoding of a string; both may be called any number of times and mixed. `finishHex` finishes and returns the checksum according to FIPS 180-4 as 64 lowercase hex digits. Calling `update`, `updateText` or `finishHex` again afterwards is a program error (`fail`). `hex` and `hexText` compute the checksum in one step.

## 19. rpp.sync: threads and channels (extension)

The package `rpp.sync` provides `Thread`, `Task<T>`, `Scope`, `Channel<T>`, `Sender<T>`, `Receiver<T>` and the built-in constraint `Transferable`. Its interface and all rules are in Chapter 16 ("R++ Threads", R-16.1 to R-16.23); this section only lists it in the standard library.
