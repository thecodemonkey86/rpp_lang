# R++ Anweisungen und Ausdrücke (Kapitel 8)

Stand: 21.11.2026, Entwurf 0.1. Dieses Dokument füllt Kapitel 8 des „R++ Spezifikationsgerüsts“. Es regelt die Bedeutung von Ausdrücken und Anweisungen, die Auswertungsreihenfolge und die Kontrollstrukturen. Die Syntax steht in der Grammatik (Gerüst, Kapitel 3); die Typregeln der Operatoren in Kapitel 4; Besitz und Borrows in Kapitel 5; Null-Prüfungen in Kapitel 6; Methodenauswahl beim Überladen in Kapitel 7; Exceptions in Kapitel 9; Lambdas in Kapitel 10.

Alle Festlegungen sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 11 und 21). Dazu gehören zwei kleine Ergänzungen aus dem Ausformulieren (Abschnitt 0), die ebenfalls angenommen sind. Jede normative Regel trägt eine Nummer `R-8.n`. Die Konformitätstests liegen in `konformitaet/kap08_anweisungen/`.

## 0. Ergänzungen beim Ausformulieren (angenommen am 21.11.2026)

| Nr. | Festlegung |
|---|---|
| V-8.1 | Eine lokale Variable darf keine andere sichtbare lokale Variable oder einen Parameter verdecken (wie in C#). Felder dürfen verdeckt werden; man erreicht sie dann mit `this.name`. |
| V-8.2 | `nodiscard` gehört zur Signatur: Eine überschreibende Methode oder eine Mixin-Methode, die eine Anforderung erfüllt, muss `nodiscard` übernehmen, wenn die überschriebene Methode bzw. die Anforderung es trägt. `nodiscard` an einer `void`-Methode ist ein Fehler. |

## 1. Diagnosen

**R-8.1 (Fehler und Warnungen).** Ein Compiler meldet Verstöße gegen die Spezifikation als **Fehler**; ein Programm mit Fehlern wird nicht übersetzt. Zusätzlich gibt es **Warnungen**: Sie müssen gemeldet werden, verhindern die Übersetzung aber nicht. Die Spezifikation legt fest, wo eine Warnung Pflicht ist; in Version 1 nur bei unerreichbarem Code (R-8.27).

## 2. Auswertungsreihenfolge

**R-8.2 (Links nach rechts).** Teilausdrücke werden von links nach rechts ausgewertet: die Operanden zweistelliger Operatoren, die Argumente eines Aufrufs, die Elemente von Array- und Tupel-Literalen und die Argumente von `new`. Bei einem Methodenaufruf wird zuerst der Empfänger ausgewertet, dann die Argumente, dann wird die Methode aufgerufen.

**R-8.3 (Kurzschluss).** `a && b` wertet `b` nur aus, wenn `a` wahr ist; `a || b` nur, wenn `a` falsch ist. `c ? a : b` wertet genau einen der beiden Zweige aus. `a ?? b` wertet `b` nur aus, wenn `a` `null` ist; bei `x?.…` wird der Rest der Kette übersprungen, wenn `x` `null` ist (Kapitel 6).

**R-8.4 (Zuweisung).** Bei `ziel = e;` werden zuerst die Teilausdrücke des Ziels ausgewertet (in `a[i()].f = g();` also `a`, dann `i()`), dann `e`, dann wird zugewiesen. Bei `(a, b) = e;` wird `e` vollständig ausgewertet, bevor zugewiesen wird; `(a, b) = (b, a);` vertauscht deshalb die Werte, bei `own`-Werten mit zwei Moves.

## 3. Ausdrücke

**R-8.5 (Arten).** Ausdrücke sind Literale, Namen, Feldzugriffe, Methodenaufrufe, statische Zugriffe mit `::`, Indexzugriffe, Operatoren nach Kapitel 4, `? :`, `??`, `?.`, `as`, `cast`, `copy`, `new`, Array- und Tupel-Literale und Lambdas. Zuweisungen sind **keine** Ausdrücke (R-8.19).

**R-8.6 (Vorrang).** Vorrang und Assoziativität der Operatoren stehen in der Tabelle des Gerüsts (Abschnitt 3.2). Klammern ändern nur die Gruppierung, nicht die Reihenfolge aus R-8.2.

**R-8.7 (Indexzugriff und Bereiche).** `a[i]` ist für feste Arrays, Vektoren, Slices und Strings definiert (Strings: Kapitel 4, R-4.19); ein Index außerhalb des Bereichs wirft eine Exception. Bereiche `a[i..j]`, `a[..j]`, `a[i..]`, `a[..]` gibt es nur für feste Arrays, Vektoren und Slices; sie liefern ein `Slice<T>` als Borrow (Kapitel 5). Auf Strings gibt es keine Bereiche; Teilstrings liefert die Standardbibliothek. Es muss `0 ≤ i ≤ j ≤ length` gelten, sonst wird eine Exception geworfen.

**R-8.8 (`++` und `--`).** `++` und `--` gibt es für Ganzzahlen; ihr Operand ist eine Variable, ein Feld oder ein Element. Sie dürfen auch in Ausdrücken stehen: `i++` liefert den alten, `++i` den neuen Wert. Der Operand wird nur einmal ausgewertet. Überlauf nach Kapitel 4, R-4.14.

**R-8.9 (Bedingungsausdruck).** In `c ? a : b` ist `c` vom Typ `bool`. Der Ausdruck hat
- bei Zahltypen den gemeinsamen Typ von `a` und `b` (R-4.12),
- sonst den Typ eines Zweigs, in den der andere automatisch umgewandelt wird (`Circle` und `Shape` ergibt `Shape`; `Foo` und `null` ergibt `Foo?`).

Gibt es keinen solchen Typ, ist der Ausdruck ein Fehler; eine gemeinsame Basisklasse wird nicht gesucht.

**R-8.10 (Literale).** Array-Literale `[a, b]` und Tupel-Literale `(a, b)` übernehmen ihren Typ aus dem Kontext (Kapitel 4, R-4.32 und R-4.35). Die Elemente werden nach R-8.2 ausgewertet; `own`-Werte werden hineinverschoben.

**R-8.11 (Weitere Ausdrücke).** `cast`: Kapitel 4, Abschnitt 3; `as`: Kapitel 4, R-4.23; `copy` und `new`: Kapitel 7; `?.` und `??`: Kapitel 6; Lambdas: Kapitel 10.

## 4. Anweisungen

### 4.1 Ausdrucksanweisungen

**R-8.12 (Erlaubte Formen).** Als Ausdrucksanweisung stehen nur ein Aufruf (auch `x?.m()`), `++`/`--` und eine Zuweisung (R-8.19). `a + b;`, `x;` und `new Foo();` sind Fehler.

**R-8.13 (Unbenutzte Ergebnisse).** Liefert ein Aufruf als Anweisung einen Wert (Rückgabetyp nicht `void`), ist das ein Fehler. Das gilt auch für eingebaute Funktionen wie `replace` und `take`. Für `++`/`--` als Anweisung gilt die Regel nicht.

**R-8.14 (`discard`).** `discard e;` wertet `e` aus und verwirft das Ergebnis; ein `own`-Wert wird dabei freigegeben (Kapitel 5). `e` ist ein Aufruf mit einem Rückgabetyp außer `void`; sonst ist `discard` ein Fehler.

**R-8.15 (`nodiscard`).** Eine Methode mit dem Zusatz `nodiscard` (`nodiscard i32 parse(string s)`) hat ein Ergebnis, das immer verwendet werden muss: `discard parse(s);` ist ein Fehler. `nodiscard` steht vor `static`, `virtual`, `override` und `abstract` und ist auch an Mixin-Anforderungen erlaubt. Für Überschreiben und `void` siehe V-8.2.

### 4.2 Deklarationen und Zuweisungen

**R-8.16 (Lokale Variablen).** Eine Deklaration führt genau eine Variable ein (`T x = e;`, `const T x = e;`, `var x = e;`, `ref T x = p;`) oder mehrere per Tupel (`(T₁ a, T₂ b) = e;`). Ihr Gültigkeitsbereich reicht von der Deklaration bis zum Ende des umgebenden Blocks. Vor dem ersten Lesen muss sie auf jedem Weg zugewiesen sein (Kapitel 4, R-4.46).

**R-8.17 (Verdecken, V-8.1).** Eine lokale Variable darf keine andere sichtbare lokale Variable und keinen Parameter verdecken; auch in einem inneren Block nicht. Felder dürfen verdeckt werden.

**R-8.18 (const).** Eine `const`-Variable wird bei der Deklaration gesetzt und danach nie wieder zugewiesen. `const` gilt tief: Über sie dürfen keine `&`-Methoden aufgerufen und keine Felder gesetzt werden.

**R-8.19 (Zuweisungen).** Zuweisungen sind Anweisungen und stehen sonst nur im Kopf einer `for`-Schleife. Ziel ist eine Variable, ein Feld, ein Element oder ein Tupel aus Zielen. Der Wert wird nach Kapitel 4 umgewandelt; für Besitz gilt Kapitel 5. Eine Zuweisung an einen String-Index, eine `const`-Variable, einen Parameter ohne `own` (solche Parameter sind nur lesbar, auch primitive) oder eine `ref`-Variable ist ein Fehler.

**R-8.20 (Verbundzuweisung).** `x op= e;` mit `op` aus `+ - * / %` bedeutet `x = x op e;`, wobei die Teilausdrücke von `x` nur einmal ausgewertet werden. Das Ergebnis von `x op e` muss automatisch in den Typ von `x` umgewandelt werden können: Bei `i8 x` ist `x += 1;` erlaubt (das Literal passt sich an), `x += n;` mit `n : i32` ist ein Fehler.

### 4.3 Verzweigungen und Schleifen

**R-8.21 (if).** Die Bedingung hat den Typ `bool`. `else` gehört zum nächstgelegenen `if`.

**R-8.22 (while, do, for).** Die Bedingungen haben den Typ `bool`. Bei `do S while (c);` läuft `S` mindestens einmal. In `for (init; c; update) S` wird `init` einmal ausgeführt; vor jedem Durchlauf wird `c` geprüft (fehlt `c`, gilt es als wahr); nach jedem Durchlauf und bei `continue` läuft `update`. Variablen aus `init` gelten nur in der Schleife.

**R-8.23 (for-each).** `for (T x : c) S` läuft über
- feste Arrays, Vektoren und Slices (Elemente in Indexreihenfolge),
- Strings (Zeichen als `char`) und
- jede Klasse, die das Bibliotheks-Mixin `Iterable<T>` importiert (Form von `Iterable`: Kapitel 13).

`c` wird für die ganze Schleife lesend geliehen, `x` ist ein lesender Borrow auf das Element (primitive Werte und Strings: eine Kopie). Mit `for (T& x : &c)` sind Sammlung und Element veränderbar geliehen. Während der Schleife kann `c` deshalb nicht wachsen oder schrumpfen (Kapitel 5, Satz 5.2).

**R-8.24 (switch).** `switch (e)` ist erlaubt für Ganzzahlen, `char`, `string` und Enums.
- Jeder `case` nennt eine oder mehrere Marken: Literale, Konstanten oder Enum-Varianten, auch mit Bindungen (`case Shape::Circle(r):`). Doppelte Marken sind ein Fehler.
- Es gibt keinen Fallthrough: Nach dem letzten Befehl eines `case` endet das `switch`.
- Ohne `default` muss ein `switch` über ein Enum alle Varianten nennen; über andere Typen ist `default` Pflicht.
- Gebundene Daten sind lesende Borrows auf die Daten der Variante; primitive Werte werden kopiert.

**R-8.25 (break und continue).** `break` beendet die innerste umgebende Schleife oder das innerste umgebende `switch`, je nachdem, was näher liegt. `continue` setzt die innerste umgebende Schleife fort, auch aus einem `switch` heraus. Sprungmarken (`break outer;`) gibt es in Version 1 nicht. `break` und `continue` außerhalb einer Schleife bzw. eines `switch` sind Fehler.

### 4.4 Rückkehr und Erreichbarkeit

**R-8.26 (return).** In einer `void`-Methode steht `return` ohne Wert und ist am Ende nicht nötig. In einer Methode mit Rückgabetyp `R` steht `return e;` mit `e` umwandelbar in `R`, und jeder Weg durch den Rumpf endet mit `return` oder `throw`; sonst ist das ein Fehler. Vor der Rückkehr werden die lokalen Variablen freigegeben (Kapitel 5).

**R-8.27 (Unerreichbarer Code).** Eine Anweisung, die direkt auf eine Anweisung folgt, die nicht normal endet (Kapitel 6, Abschnitt 3.3: `return`, `throw`, `break`, `continue` und daraus zusammengesetzte), ist unerreichbar. Der Compiler meldet dafür eine **Warnung** (R-8.1) und übersetzt das Programm.

**R-8.28 (throw, try).** Exceptions regelt Kapitel 9.

## 5. Konformitätstests

Die Testprogramme liegen in `konformitaet/kap08_anweisungen/`; geprüft wird bisher nur die Syntax. Programme mit `// WARNUNG` müssen übersetzt werden und an dieser Stelle eine Warnung erzeugen.

| Regeln | Akzeptiert | Abgelehnt |
|---|---|---|
| R-8.2 – R-8.4, R-8.8 Auswertung, `++` | `a01_auswertung.rpp` | `n01_inkrement_gleitkomma.rpp` |
| R-8.9 Bedingungsausdruck | `a01_auswertung.rpp` | `n02_kein_gemeinsamer_typ.rpp` |
| R-8.7 Bereiche | `a01_auswertung.rpp` | `n03_bereich_auf_string.rpp` |
| R-8.13 – R-8.15 Ergebnisse, `discard`, `nodiscard` | `a03_discard.rpp` | `n04_ergebnis_unbenutzt.rpp`, `n05_discard_nodiscard.rpp`, `n06_discard_void.rpp`, `n07_nodiscard_void.rpp`, `n08_override_ohne_nodiscard.rpp` |
| R-8.17 – R-8.20 Deklaration, Zuweisung | `a01_auswertung.rpp` | `n09_verdecken.rpp`, `n10_const_zuweisen.rpp`, `n11_verbund_verengt.rpp` |
| R-8.23 for-each | `a02_kontrollfluss.rpp` | `n12_foreach_nicht_iterierbar.rpp` |
| R-8.24, R-8.25 switch, break | `a02_kontrollfluss.rpp` | `n13_doppelte_marke.rpp`, `n14_default_fehlt.rpp`, `n15_enum_unvollstaendig.rpp`, `n16_break_ausserhalb.rpp` |
| R-8.26 return | `a04_rueckgabe.rpp` | `n17_rueckgabe_fehlt.rpp` |
| R-8.27 unerreichbarer Code | `a05_warnung.rpp` | – |
