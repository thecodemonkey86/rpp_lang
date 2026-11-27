# R++ C-Anbindung (Kapitel 15)

[English](R%2B%2B%20C%20Interop.md)

Stand: 27.11.2026, Entwurf 0.1. Dieses Dokument füllt Kapitel 15 des „R++ Spezifikationsgerüsts“. Es regelt, wie ein R++-Programm Funktionen einer C-Bibliothek aufruft: die Deklaration mit `extern`, welche Typen über die Grenze gehen und wie, Handles für Objekte, die C verwaltet, das Laden der Bibliothek und die Sicherheitsgrenze, die festlegt, welche Projekte C-Code mitbringen dürfen und welche Garantien dann noch gelten.

Die Grundzüge sind entschieden (siehe „R++ Designentscheidungen“, Abschnitt 12, Meilenstein M10 im Implementierungsplan). Die Vorschläge in Abschnitt 0 sind beim Ausformulieren entstanden und wurden am 27.11.2026 angenommen; die Regeln, die auf ihnen beruhen, nennen sie. Jede normative Regel trägt eine Nummer `R-15.n`. Die Konformitätstests liegen in `conformance/ch15_c_interop/`.

## 0. Ergänzungen beim Ausformulieren (angenommen am 27.11.2026)

| Nr. | Vorschlag |
|---|---|
| V-15.1 | Ein Handle-Typ wird als eigene Typdeklaration neben der `extern`-Klasse geschrieben: `extern("demo") handle Counter(demo_counter_free);`. `handle` ist nur direkt nach `extern(…)` ein Schlüsselwort und bleibt sonst ein gewöhnlicher Name. |
| V-15.2 | Der Name einer Methode in einer `extern`-Klasse ist zugleich der Name des C-Symbols; eine Umbenennung gibt es nicht. Methoden einer `extern`-Klasse lassen sich deshalb nicht überladen. |
| V-15.3 | Ein Parameter `byte[]` oder `mut byte[]` wird in C zu **zwei** Parametern: Zeiger auf das erste Element und Länge als `size_t`. |
| V-15.4 | Ein `string`, den C zurückgibt, gehört weiter C: R++ kopiert ihn sofort und gibt den Zeiger nie frei. Zeichenketten, die der Aufrufer freigeben muss, bildet man über Handles ab. |
| V-15.5 | Alle C-Bibliotheken eines Programms werden vor `main` geladen und alle Symbole aufgelöst. Fehlt etwas, läuft das Programm nicht: Der Releasemodus meldet es beim Bauen, der Debugmodus beim Start als Programmfehler (Exit-Code 2). Das wird ein neuer zulässiger Unterschied in R-12.5. |
| V-15.6 | Vor jedem C-Aufruf werden Standard- und Fehlerausgabe von R++ geleert, damit Ausgaben von R++ und C in der Reihenfolge erscheinen, in der sie entstehen (R-12.4). |
| V-15.7 | Neuer Eintrag `nativePaths` in `rpp.yaml`: Ordner relativ zum Projektverzeichnis, in denen C-Bibliotheken beim Bauen und beim Laden zuerst gesucht werden. |
| V-15.8 | Eine Bibliothek „bringt C-Code mit“, wenn sie `native: true` ist oder von einer Bibliothek abhängt, die C-Code mitbringt. Jede Kante zu einer solchen Bibliothek braucht `allowNative: true`. So entscheidet das Programm über seine direkten Abhängigkeiten über alles C darunter, und jede Bibliothek dazwischen hat ihre C-Abhängigkeiten selbst freigegeben. |
| V-15.9 | Neue Art von Programmfehler `NativeError` (R-9.2) für Fehler an der Grenze: ein Argument, das sich nicht nach C umwandeln lässt, ein Ergebnis, das sich nicht nach R++ umwandeln lässt, eine fehlende Bibliothek oder ein fehlendes Symbol beim Start. |

## 1. Deklaration

**R-15.1 (extern-Klasse).** Eine `extern`-Klasse deklariert Funktionen einer C-Bibliothek:

```
externClassDecl : 'export'? 'extern' '(' STRING ')' 'class' IDENT '{' externMember* '}' ;
externMember    : accessLabel | externMethod ;
externMethod    : 'static' returnType IDENT '(' paramList? ')' ';' ;
```

Die Zeichenkette in Klammern ist der Name der Bibliothek (R-15.4). Die Klasse steht wie jede Typdeklaration in einem Paket und wird wie eine Klasse mit statischen Methoden verwendet: `Zlib::crc32(0, data)`. `extern` ist ein reserviertes Wort.

**R-15.2 (Inhalt).** Eine `extern`-Klasse enthält nur statische Methoden ohne Rumpf. Felder, Konstruktoren, ein Destruktor, `copy`, Instanzmethoden, Methoden mit Rumpf, Konstanten, Basisklassen, Mixin-Importe und Typ-Parameter sind Fehler. Von einer `extern`-Klasse kann man keine Objekte erzeugen, nicht erben und sie nicht als Typ einer Variablen verwenden. Sichtbarkeits-Labels und `export` gelten wie bei anderen Klassen (R-7.2, Kapitel 11); Methoden vor dem ersten Label sind also `private`.

**R-15.3 (Symbolnamen).** Der Name einer Methode ist der Name des C-Symbols (V-15.2). Zwei Methoden gleichen Namens in einer `extern`-Klasse sind ein Fehler. Deklarieren zwei `extern`-Klassen eines Programms dasselbe Symbol derselben Bibliothek, müssen beide Deklarationen dieselbe Signatur haben; sonst ist das ein Fehler an der zweiten.

**R-15.4 (Name der Bibliothek).** Der Name besteht aus Buchstaben, Ziffern, `_`, `-` und `.`, ist nicht leer und enthält keine Pfadtrenner. Er wird je Plattform zu einem Dateinamen: unter Linux `lib<name>.so`, unter macOS `lib<name>.dylib`, unter Windows `<name>.dll` (zum Binden `<name>.lib`). Gesucht wird zuerst in den Ordnern aus `nativePaths` (V-15.7) in ihrer Reihenfolge, dann an den Orten, die das System für Bibliotheken vorsieht.

**R-15.5 (Handle-Typ).** Ein Handle-Typ steht für ein Objekt, das C verwaltet (V-15.1):

```
externHandleDecl : 'export'? 'extern' '(' STRING ')' 'handle' IDENT '(' IDENT ')' ';' ;
```

Der Name in Klammern ist eine C-Funktion derselben Bibliothek mit der Signatur `void f(void*)`, die das Objekt freigibt. In C ist ein Handle ein undurchsichtiger Zeiger (`void*`). Ein Handle-Typ hat keine Member; R++-Code kann ihn nur halten, leihen, übergeben und freigeben.

```
extern("demo") handle Counter(demo_counter_free);

extern("demo") class Demo {
public:
    static own Counter? demo_counter_new(i64 start);
    static i64 demo_counter_next(mut Counter c);
}
```

## 2. Typen über die Grenze

**R-15.6 (Erlaubte Typen).** Parameter und Rückgabetypen von Methoden einer `extern`-Klasse bilden sich so auf C ab:

| R++ | als Parameter | als Rückgabe |
|---|---|---|
| `i8`, `i16`, `i32`, `i64` | `int8_t` … `int64_t` | ebenso |
| `ui8`, `ui16`, `ui32`, `ui64` | `uint8_t` … `uint64_t` | ebenso |
| `f32`, `f64` | `float`, `double` | ebenso |
| `bool` | `bool` (C99) | ebenso |
| `byte` | `uint8_t` | ebenso |
| `string` | `const char*`, kopiert (R-15.7) | `const char*`, kopiert (R-15.8) |
| `string?` | – | wie `string`; `NULL` wird `null` |
| `byte[]` | `const uint8_t*`, `size_t` (R-15.9) | – |
| `mut byte[]` | `uint8_t*`, `size_t` (R-15.9) | – |
| Handle `H` (geliehen) | `void*` (R-15.10) | – |
| `mut H` | `void*` (R-15.10) | – |
| `own H` | `void*`, Besitz geht an C (R-15.10) | `void*`, Besitz geht an R++ |
| `own H?` | – | `void*`; `NULL` wird `null` |
| `void` | – | `void` |

Andere Typen sind ein Fehler an der Deklaration, darunter `char`, Klassen, Enums, Tupel, Funktionstypen, Arrays anderer Elementtypen, nullbare Grundwerte, `shared` und `weak`.

**R-15.7 (string als Parameter).** R++ übergibt eine Kopie des Strings in UTF-8 mit abschließendem Nullbyte. Die Kopie gilt nur für die Dauer des Aufrufs und wird danach freigegeben. Enthält der String das Zeichen U+0000, ist das ein Programmfehler (`NativeError`, V-15.9), bevor C aufgerufen wird.

**R-15.8 (string als Rückgabe).** Den zurückgegebenen Zeiger liest R++ bis zum ersten Nullbyte und kopiert den Inhalt in einen neuen String (V-15.4). Kein gültiges UTF-8 ist ein Programmfehler (`NativeError`). `NULL` ergibt bei `string?` den Wert `null` und ist bei `string` ein Programmfehler (`NativeError`).

**R-15.9 (byte[]).** Ein Parameter `byte[]` ist für die Dauer des Aufrufs lesend geliehen, `mut byte[]` veränderbar (Kapitel 5); C erhält Zeiger und Anzahl der Elemente (V-15.3). C darf über `mut byte[]` die Elemente ändern, nicht die Länge. Bei Länge 0 darf der Zeiger beliebig sein, auch `NULL`.

**R-15.10 (Handles).** Ein Handle-Typ besteht nur als `own`-Wert; `shared H`, `weak H` und `copy` eines Handles sind Fehler. Ein Parameter `H` leiht das Handle lesend, `mut H` veränderbar, beide nur für die Dauer des Aufrufs. Ein Parameter `own H` verschiebt das Handle nach C: R++ gibt es danach nicht mehr frei. Ein Rückgabewert `own H` gehört R++; `NULL` ist dabei ein Programmfehler (`NativeError`), bei `own H?` wird es `null`. Gibt R++ ein Handle frei (Kapitel 5, R-12.11), ruft es die Freigabefunktion aus R-15.5 genau einmal auf.

## 3. Aufrufe

**R-15.11 (Ablauf).** Ein Aufruf einer Methode einer `extern`-Klasse läuft so ab:
1. Die Argumente werden ausgewertet wie bei jedem Aufruf (R-8.2).
2. Sie werden nach R-15.6 bis R-15.10 umgewandelt; schlägt das fehl, endet das Programm mit `NativeError`, und C wird nicht aufgerufen.
3. Standard- und Fehlerausgabe werden geleert (V-15.6).
4. Die C-Funktion wird aufgerufen.
5. Das Ergebnis wird umgewandelt (R-15.8, R-15.10); danach enden alle Leihen des Aufrufs, und die Kopien aus R-15.7 werden freigegeben.

Eine C-Funktion wirft keine Exceptions; Fehler meldet sie über ihren Rückgabewert, den R++-Code prüft und gegebenenfalls in eine Exception übersetzt.

**R-15.12 (Laden).** Alle Bibliotheken, die `extern`-Klassen und Handle-Typen eines Programms nennen, werden vor `main` geladen, und jedes deklarierte Symbol wird aufgelöst (V-15.5), auch wenn das Programm es nie aufruft. Fehlt eine Bibliothek oder ein Symbol, meldet der Releasemodus das beim Bauen; der Debugmodus beendet das Programm vor `main` mit `NativeError` und Exit-Code 2.

## 4. Sicherheitsgrenze

**R-15.13 (native).** Nur ein Projekt mit `native: true` in seiner `rpp.yaml` darf `extern`-Klassen und Handle-Typen deklarieren; sonst ist jede solche Deklaration ein Fehler. Das gilt für Programme wie für Bibliotheken.

**R-15.14 (Freigabe).** Eine Bibliothek bringt C-Code mit, wenn sie `native: true` ist oder von einer Bibliothek abhängt, die C-Code mitbringt (V-15.8). Jede Abhängigkeit auf eine solche Bibliothek braucht in der `rpp.yaml` des abhängigen Projekts `allowNative: true`, lokal wie aus dem Verzeichnis:

```yaml
dependencies:
  - name: acme.zlib
    version: "1.2"
    allowNative: true
```

Fehlt die Freigabe, ist das ein Fehler an dieser Anforderung, der die Bibliothek nennt, die den C-Code enthält. `allowNative: true` an einer Abhängigkeit ohne C-Code ist eine Warnung.

**R-15.15 (Sichtbarkeit).** Die Ausgabe von `project` zeigt bei jeder Bibliothek, ob sie C-Code mitbringt. In `rpp.lock` (R-11.30) trägt jeder Eintrag einer Bibliothek mit `native: true` die Zeile `native: true`; weil die Prüfsumme `rpp.yaml` umfasst (R-11.29), fällt auch eine nachträglich hinzugefügte Markierung auf.

**R-15.16 (Garantien).** Die Prüfungen der Kapitel 4 bis 6 gelten für den R++-Code eines Programms unverändert, auch an Aufrufen von `extern`-Methoden. R-12.6 (kein undefiniertes Verhalten) gilt für ein Programm mit C-Code nur, solange sich jede aufgerufene C-Funktion an ihre Deklaration hält:
- sie hat die Signatur, die R-15.6 aus der Deklaration ergibt;
- sie liest und schreibt nur den Speicher, den sie erhalten hat, und nur in den Grenzen aus R-15.9;
- sie behält keinen Zeiger auf übergebene Strings oder Arrays über den Aufruf hinaus;
- sie gibt keinen Speicher frei, der R++ gehört, und ein geliehenes Handle nicht;
- sie ruft keinen R++-Code auf und startet keine Threads, die nach dem Aufruf R++-Werte berühren.

Verletzt C-Code eine dieser Bedingungen, legt die Spezifikation das Verhalten des Programms nicht fest. Die Prüfung dieser Bedingungen ist nicht Aufgabe des Compilers; deshalb muss jede Stelle, an der sie gelten müssen, nach R-15.13 und R-15.14 ausdrücklich zugelassen sein.

## 5. Bauen und Ausführen

**R-15.17 (Beide Modi).** C-Aufrufe verhalten sich im Debug- und im Releasemodus gleich (R-12.5 mit der Ergänzung aus V-15.5). Wie eine Implementierung die Bibliothek anbindet, ist ihr überlassen; die erste Implementierung lädt sie im Interpreter von `rppc` zur Laufzeit und ruft über `libffi` auf, `rppcc` erzeugt Prototypen und direkte Aufrufe und bindet die Bibliothek beim Bauen. Wie das gebaute Programm die Bibliothek zur Laufzeit findet (Suchpfad, Kopie neben das Programm), bleibt der Implementierung überlassen, solange R-15.4 gilt.

## 6. Beispiel

`rpp.yaml` einer Bibliothek, die zlib anbindet:

```yaml
name: acme.zlib
version: 1.0.0
type: library
sources: [src]
native: true
```

`src/acme/zlib/Zlib.rpp`:

```
package acme.zlib;

extern("z") class ZlibC {
public:
    static ui32 crc32(ui32 crc, byte[] data);
}

export class Crc32 {
public:
    static ui32 of(byte[] data) {
        return ZlibC::crc32(0, data);
    }
}
```

Ein Programm, das sie nutzt, gibt sie frei (R-15.14):

```yaml
name: app
version: 0.1.0
type: executable
main: src/main.rpp
sources: [src]
dependencies:
  - name: acme.zlib
    version: "1.0"
    allowNative: true
```

## 7. Änderungen an anderen Kapiteln

Mit diesem Kapitel haben sich die folgenden Stellen geändert (eingetragen am 27.11.2026). `RPP.g4` und die Parser von `rppc` und `rppcc` folgten mit der Umsetzung in M10 (27.11.2026).

| Stelle | Änderung |
|---|---|
| Gerüst, Kapitel 2 | `extern` in der Liste der reservierten Wörter |
| `RPP.g4` | `externClassDecl` und `externHandleDecl` als Typdeklarationen |
| R-9.2 | neue Art `NativeError` (V-15.9) |
| R-11.13 | Einträge `native` und `nativePaths` (V-15.7) |
| R-11.15 | `allowNative` an Abhängigkeiten |
| R-11.30 | Zeile `native: true` in `rpp.lock` |
| R-12.5 | fehlende Bibliothek: beim Bauen bzw. beim Start (V-15.5) |
| R-12.6 | Verweis auf die Grenze aus R-15.16 |

## 8. Konformitätstests

Die Testprogramme liegen in `conformance/ch15_c_interop/`. Die Tests bringen eine kleine C-Bibliothek `native/rpptest.c` mit, die der Testlauf vor den Tests mit dem C-Compiler der Plattform übersetzt; die Projekte der Tests nennen ihren Ordner in `nativePaths`.

| Regeln | Akzeptiert bzw. Laufzeit | Abgelehnt |
|---|---|---|
| R-15.1, R-15.2 | `l01_call_integers` | `n01_extern_with_body`, `n02_extern_field`, `n03_extern_instance` |
| R-15.3 | – | `n04_overloaded_symbol`, `n05_conflicting_signatures` |
| R-15.4 | – | `n06_library_name_with_path` |
| R-15.5, R-15.10 | `l02_handle_release`, `l03_handle_to_c` | `n07_shared_handle`, `n08_handle_copy` |
| R-15.6 | `l04_floats_and_bool` | `n09_class_parameter`, `n10_char_parameter` |
| R-15.7, R-15.8 | `l05_strings`, `l06_null_string` (Laufzeit: `NativeError`) | – |
| R-15.9 | `l07_byte_arrays` | `n11_byte_array_return` |
| R-15.11 | `l08_output_order` | – |
| R-15.12 | `l09_missing_symbol` (Laufzeit: Exit-Code 2 im Debugmodus, Fehler beim Bauen im Releasemodus) | – |
| R-15.13 | – | `n12_extern_without_native` |
| R-15.14, R-15.15 | `l10_allowed_dependency` | `n13_not_allowed`, `n14_not_allowed_indirect` |

R-15.16 und R-15.17 sind nicht durch einzelne Programme prüfbar; sie werden durch den Vergleich beider Modi in allen Laufzeittests dieses Kapitels abgedeckt.
