# FR++ – ausführbares Modell des Ownership-Kernkalküls

Typprüfer und Interpreter für die Kernsprache FR++ aus „R++ Ownership-Kernkalkül“ (Kapitel 5 der Spezifikation). Das Modell setzt die Regeln R-5.1 bis R-5.22 und die Auswertungsregeln aus Abschnitt 5 möglichst wörtlich um, damit man sie an Programmen ausprobieren und die Sätze 5.1 bis 5.3 mit Zufallstests prüfen kann. Es ist kein Compiler für R++ und ersetzt keinen Beweis.

Voraussetzung: Python 3.10 oder neuer, keine weiteren Pakete. Alle Befehle laufen in diesem Verzeichnis.

```
py -X utf8 run_tests.py                          # handgeschriebene Testprogramme
py -X utf8 fuzz.py 2000                          # Zufallstest mit 2000 Programmen
py -X utf8 fuzz.py 600 --abschwaechen loans      # Gegenprobe mit abgeschalteter Regel
```

## Inhalt

| Pfad | Zweck |
|---|---|
| `frpp/ast.py` | Abstrakte Syntax (Abschnitt 3.1) und Rückübersetzung in Text |
| `frpp/parser.py` | Parser für die Textform |
| `frpp/checker.py` | Typprüfer: Orte, Loans, Regeln R-5.1 bis R-5.22 |
| `frpp/interp.py` | Interpreter: Heap, Drop-Flags, Sperren, Freigabe; Prüfung der Invarianten I-1, I-2, I-4, I-5 |
| `frpp/gen.py` | Zufallsgenerator für Programme |
| `run_tests.py` | Testlauf für `tests/` |
| `fuzz.py` | Zufallstest für die Sätze 5.1 bis 5.3 |
| `tests/akzeptiert/*.fr` | Programme, die angenommen werden und ein bestimmtes Ergebnis liefern müssen |
| `tests/abgelehnt/*.fr` | Programme, die mit einer bestimmten Regel in einer bestimmten Zeile abgelehnt werden müssen |
| `testbericht.txt` | Ausgabe des letzten Laufs von `run_tests.py` und `fuzz.py` |

## Textform

Wie in Abschnitt 3.1 des Kalküls. Ein Programm besteht aus Klassen und einem Block `main { … }`. Methoden mit Rückgabetyp `ref C` oder `mut C` liefern einen Borrow (`return &ro this.f;`); ihr Ergebnis wird mit `borrow x = p.m(…) { … }` gebunden (R-5.23, R-5.24).

```
class A {
    int v;
    int get() ro { return copy this.v; }
    int inc() mut { this.v = copy this.v + 1; return 0; }
    destructor() { print(copy this.v); }
}
main {
    own A a = new A(1);
    shared A s = share new A(2);
    weak A w = demote copy s;
    upgrade t = w { print(t.get()); } else { print(0); }
    borrow x = &mut a { x.inc(); }
}
```

Erweiterungen gegenüber dem Kalkül, nur für Tests: `print(e);` und die int-Operatoren `+`, `-`, `<`. Methoden geben immer einen Wert zurück (`return 0;` für „void“).

## Testprogramme

Die erste Kommentarzeile `// erwartet:` legt das Ergebnis fest:

- `// erwartet: Ausgabe 1 2 3` – die Folge der `print`-Ausgaben, einschließlich der Ausgaben von Destruktoren
- `// erwartet: BorrowError` – das Programm endet mit einem Sperrkonflikt (V-1)
- `// erwartet: Fehler R-5.9 …` – die Typprüfung lehnt mit dieser Regel in der Zeile ab, die `// FEHLER` trägt
- `// ohne Typprüfung: Fehlerzustand` (zusätzlich bei abgelehnten Programmen) – das Programm wird trotz Ablehnung ausgeführt und muss dann wirklich in einen Fehlerzustand laufen (bzw. mit `BorrowError` enden). Das belegt, dass die Regel einen echten Fehler verhindert und nicht nur ein harmloses Programm ablehnt.

Akzeptierte Programme laufen mit Invariantenprüfung nach jeder Anweisung. Ein Fehlerzustand im Sinne von Satz 5.1 ist immer ein Fehlschlag, ebenso ein Objekt, das am Ende übrig bleibt, obwohl nie etwas geteilt wurde.

## Zufallstest

`fuzz.py` erzeugt Programme mit einer festen Klassentabelle und zufälligen Methodenrümpfen, Destruktoren und Hauptprogrammen. Der Generator achtet grob auf Moves und Borrows, verstößt aber absichtlich in etwa jedem zehnten Fall dagegen. Jedes Programm wird geprüft und ausgeführt:

- **Angenommene Programme** dürfen nie in einen Fehlerzustand laufen und keine Invariante verletzen (Satz 5.1). Sie dürfen nur dann mit `BorrowError` enden, wenn ein Objekt geteilt wurde (Satz 5.3). Ohne geteilte Objekte darf am Ende nichts übrig bleiben. Jeder Verstoß wird mit Programmtext in `fuzz-befunde/` abgelegt.
- **Abgelehnte Programme** werden trotzdem ausgeführt. Gezählt wird, wie viele davon tatsächlich in einen Fehlerzustand laufen; das zeigt, dass die Regeln echte Fehler abfangen.
- „Budget“ heißt, das Programm hat sein Schrittbudget aufgebraucht (Endlosrekursion über zufällige Methoden). Das zählt wie Nichtterminierung und ist zulässig.

Mit `--abschwaechen NAME` wird eine Regel des Typprüfers abgeschaltet. Dann müssen Verstöße auftreten; sonst erreicht der Zufallstest die Regel nicht.

| Name | Abgeschaltet |
|---|---|
| `loans` | `erlaubt_L` (alle Loan-Prüfungen) |
| `move` | Zustand ○ nach einem Move |
| `join` | Vereinigung der Zweige in R-5.15 |
| `schleife` | Bedingung `Γ₁ ⊒ Γ` in R-5.16 |
| `schreibrecht` | `schreibbar`: ref-Wurzeln gelten als schreibbar |
| `empfaenger-loan` | R-5.24 trägt keinen Loan auf den Empfänger eines Borrow-Aufrufs ein |
| `shared-ueberlappung` | Überlappung ganzer shared-Wurzeln (nur Präfix) |

## Grenzen

- Kein `null`, keine Exceptions, kein `try`; ein `BorrowError` beendet das Programm.
- Keine Vektoren, Slices, Lambdas und Strings; sie werden in Abschnitt 7 des Kalküls auf die Kernsprache abgebildet, sind hier aber nicht umgesetzt.
- Zufallstests finden Fehler, beweisen aber keine Korrektheit. Der maschinelle Beweis steht weiter aus.
