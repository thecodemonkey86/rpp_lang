# R++ Lambdas und Funktionstypen (Kapitel 10)

Stand: 21.11.2026, Entwurf 0.1. Dieses Dokument füllt Kapitel 10 des „R++ Spezifikationsgerüsts“. Es regelt Lambdas, das Erfassen von Variablen und Funktionstypen. Die Typregeln für `Func` stehen in Kapitel 4 (R-4.37, R-4.38); wie Erfassen auf Borrows und Moves abgebildet wird, in Kapitel 5 (Abschnitt 7); Verengung in Lambdas in Kapitel 6 (R-6.13).

Alle Festlegungen sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 11 und 23). Jede normative Regel trägt eine Nummer `R-10.n`. Die Konformitätstests liegen in `konformitaet/kap10_lambdas/`.

## 1. Überblick

| | Normales Lambda | `own`-Lambda |
|---|---|---|
| Schreibweise | `(i32 x) => x + offset` | `own (i32 x) => x + offset` |
| Erfasst | per Borrow, lesend oder veränderbar | per Wert: Kopie, geteilt oder verschoben |
| Darf stehen | nur als Argument eines Aufrufs oder direkt aufgerufen | überall, wo ein `Func`-Wert stehen darf |
| Lebensdauer | bis zum Ende des Aufrufs | wie jeder `own`-Wert |
| Typische Verwendung | `list.forEach(…)`, `sort(…)` | Rückrufe speichern, Funktionen zurückgeben |

## 2. Lambdas

**R-10.1 (Schreibweise).** Ein Lambda besteht aus einer Parameterliste mit Typen, `=>` und einem Rumpf: einem Ausdruck oder einem Block. Ein vorangestelltes `own` macht es zu einem `own`-Lambda. Parameter tragen dieselben Markierungen wie Methodenparameter: ohne Markierung lesend geliehen, `mut` veränderbar geliehen, `own` übernommen. Eine Capture-Liste gibt es nicht.

**R-10.2 (Ausdrucksebene).** Ein Lambda steht auf Ausdrucksebene; sein Rumpf reicht bis zum Ende des umgebenden Ausdrucks. Als Operand oder für einen direkten Aufruf wird es geklammert: `((i32 a) => a)(3)` (Gerüst, Abschnitt 3.1).

**R-10.3 (Rückgabetyp).** Bei einem Ausdrucksrumpf ist der Rückgabetyp der Typ des Ausdrucks; ist dieser `void`, ist der Rückgabetyp `void`. Bei einem Blockrumpf ist er der gemeinsame Typ aller `return`-Werte nach den Regeln für `? :` (R-8.9); ohne `return` mit Wert ist er `void`. Ist ein `Func`-Typ erwartet, wird das Lambda gegen ihn geprüft, und Literale passen sich an den erwarteten Rückgabetyp an.

**R-10.4 (Typ ohne Kontext).** Steht ein Lambda ohne erwarteten Funktionstyp, etwa in `var f = own (i32 x) => x + 1;`, bekommt es den Typ `Func<P₁, …, Pₙ, R>` aus seinen Parametertypen und dem Rückgabetyp nach R-10.3. Ein normales Lambda kann so nicht stehen, weil es nur Argument sein oder direkt aufgerufen werden darf (R-10.7).

**R-10.5 (Rumpf).** Im Rumpf gelten dieselben Regeln wie in einem Methodenrumpf: sichere Rückgabe (R-8.26), unbenutzte Ergebnisse (R-8.13), Exceptions (Kapitel 9). `return` verlässt nur das Lambda, nicht die umgebende Methode. `break` und `continue` können das Lambda nicht verlassen. Ein Lambda liefert nie einen Borrow (Kapitel 5, V-4).

## 3. Erfassen

**R-10.6 (Was erfasst wird).** Ein Lambda erfasst jede lokale Variable und jeden Parameter der umgebenden Methode, die es im Rumpf verwendet, außerdem `this`, wenn es `this` oder Member ohne Objekt verwendet.

**R-10.7 (Normales Lambda).** Ein normales Lambda erfasst per Borrow:
- **veränderbar**, wenn der Rumpf der Variable etwas zuweist, sie veränderbar leiht (`mut x`) oder eine `mut`-Methode auf ihr aufruft, und dasselbe für ihre Felder;
- sonst **lesend**.

Ein normales Lambda darf nur als Argument eines Aufrufs stehen oder geklammert direkt aufgerufen werden (`((i32 v) => v + 1)(3)`). Seine Borrows gelten für die Dauer dieses Aufrufs und gehen in die Loan-Prüfung des Aufrufs ein wie Borrow-Argumente (Kapitel 5, R-5.9 und R-5.10). `sort(mut list, (Foo a, Foo b) => a.x < list.length)` ist deshalb ein Fehler, weil `list` zugleich veränderbar und lesend geliehen würde.

**R-10.8 (`own`-Lambda).** Ein `own`-Lambda erfasst per Wert, und zwar in dem Moment, in dem der Lambda-Ausdruck ausgewertet wird:
- primitive Werte werden kopiert,
- Strings und `shared`-Werte werden geteilt (Zähler + 1), `weak`-Werte kopiert,
- `own`-Werte werden hineinverschoben; die Variable ist danach verschoben (Kapitel 5, R-5.3).

Spätere Änderungen an der Variable sieht das Lambda nicht.

**R-10.9 (Keine Borrows per Wert).** Ein `own`-Lambda kann keine Variable erfassen, die nur einen Borrow auf ein Objekt hält: Parameter ohne `own`, `ref`-Variablen, Schleifenelemente und Variablen aus Borrow-Blöcken. Ein Borrow lässt sich weder kopieren noch verschieben; man schreibt vorher `copy(x)`. Für primitive Werte, Strings und `shared`-Werte gilt das nicht.

**R-10.10 (Kein this in `own`-Lambdas).** Ein `own`-Lambda darf `this` weder direkt noch indirekt (über Member ohne Objekt) verwenden, denn `this` ist ein Borrow. Man holt die benötigten Werte vorher in lokale Variablen.

**R-10.11 (Unveränderlich).** Im Rumpf eines `own`-Lambdas sind die erfassten Werte unveränderlich: keine Zuweisung an eine erfasste Variable, und auf einem erfassten `own`-Objekt weder `mut`-Borrow noch `mut`-Methode. Ein Aufruf verändert das Lambda deshalb nie. Ausgenommen sind Objekte hinter einem erfassten `shared`-Verweis: Sie bleiben veränderbar, wie über jeden `shared`-Verweis (Kapitel 5, V-1 und V-8, mit Laufzeitprüfung). Veränderlichen Zustand hält man deshalb in einem erfassten `shared`-Objekt.

**R-10.12 (Verengung).** Eine verengte Variable ist auch im Lambda verengt (Kapitel 6, R-6.13).

## 4. Funktionstypen und Funktionswerte

**R-10.13 (Func).** `Func<P₁, …, Pₙ, R>` ist der Typ einer Funktion mit den Parametern `P₁ … Pₙ` und dem Rückgabetyp `R`. Für die Parameter gelten die Markierungen aus R-10.1: `Func<Foo, void>` leiht lesend, `Func<mut Foo, void>` veränderbar, `Func<own Foo, void>` übernimmt. `mut` ist nur an Parametern von `Func` erlaubt, nicht am Rückgabetyp und nicht in anderen generischen Typen. `void` steht nur an letzter Stelle. Funktionstypen sind strukturell und invariant (R-4.37).

**R-10.14 (Passen).** Ein Lambda passt zu `Func<P₁, …, Pₙ, R>`, wenn es `n` Parameter mit genau den Typen und Markierungen `Pᵢ` hat und sein Rückgabewert in `R` umgewandelt werden kann (bei `R = void`: kein Wert).

**R-10.15 (Funktionswerte).** Ein Wert vom Typ `Func<…>` ist ein `own`-Objekt; er entsteht nur aus einem `own`-Lambda. Zuweisen und Übergeben an einen `own`-Parameter verschieben ihn. Er kann `shared` werden (`shared Func<i32, i32> f = …;`), aber nicht kopiert werden (`copy(f)` ist ein Fehler).

**R-10.16 (Parameter vom Typ Func).** Ein Parameter `Func<…> f` ohne Markierung nimmt ein normales Lambda, ein `own`-Lambda oder einen vorhandenen Funktionswert als lesenden Borrow. Die Methode darf `f` beliebig oft aufrufen und als Argument weitergeben, aber nicht speichern oder zurückgeben. Ein Parameter `own Func<…> f` nimmt nur `own`-Lambdas und Funktionswerte; die Methode besitzt sie danach.

**R-10.17 (Aufruf).** Ein Funktionswert wird mit `f(a₁, …, aₙ)` aufgerufen, auch als Feld (`this.handler(x)`) oder Parameter. Die Argumente folgen den Regeln für Methodenaufrufe (Kapitel 5 und 8). Ein Aufruf liest den Funktionswert nur und ist über jeden Borrow möglich. Liefert er einen Wert, gilt R-8.13 (unbenutztes Ergebnis ist ein Fehler, `discard` verwirft).

**R-10.18 (Überladen).** Ein Lambda-Argument passt bei der Auflösung überladener Methoden mit Rang „genau“, wenn es nach R-10.14 zum Parametertyp passt; sonst kommt die Variante nicht in Frage (Kapitel 7, R-7.17).

## 5. Beispiel

```
class Button {
private:
    Func<void>? onClick;              // gespeicherter Rückruf: nur own-Lambdas
public:
    void setHandler(own Func<void> f) mut {
        this.onClick = f;
    }
    void click() {
        ref Func<void>? h = this.onClick;   // nullbares own-Feld mit ref holen (R-6.15)
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
        list.forEach((Foo f) => { if (f.x < limit) { sum += f.x; } });   // limit lesend, sum veränderbar geliehen
        list.forEach((mut Foo f) => f.bump());                            // Func<mut Foo, void>
        b.setHandler(own () => { c.increment(); });                       // c wird geteilt
        var square = own (i32 x) => x * x;                                // Func<i32, i32>
        i32 n = square(4);
    }
}
```

`click` zeigt das Zusammenspiel mit Kapitel 5 und 6: `Func<void>? h = this.onClick;` wäre ein Move aus dem Feld; zum Aufrufen holt man das nullbare Feld deshalb mit `ref` und prüft es. In `run` verändert das `own`-Lambda den Zähler über den erfassten `shared`-Verweis `c` (R-10.11).

## 6. Konformitätstests

Die Testprogramme liegen in `konformitaet/kap10_lambdas/`; geprüft werden sie mit `rppc test` (erste Implementierung, Meilenstein M3).

| Regeln | Akzeptiert | Abgelehnt |
|---|---|---|
| R-10.3, R-10.4 Rückgabetyp, `var` | `a01_grundlagen.rpp` | `n01_normales_lambda_in_var.rpp`, `n02_rueckgabe_passt_nicht.rpp` |
| R-10.5 Rumpf | `a01_grundlagen.rpp` | `n03_break_aus_lambda.rpp` |
| R-10.7 normales Lambda | `a02_erfassen.rpp` | `n04_normales_lambda_speichern.rpp`, `n05_loan_konflikt.rpp` |
| R-10.8 – R-10.11 `own`-Lambda | `a02_erfassen.rpp` | `n06_borrow_per_wert.rpp`, `n07_this_im_own_lambda.rpp`, `n08_erfasst_veraendern.rpp`, `n09_nach_erfassen_verschoben.rpp` |
| R-10.13, R-10.14 Func | `a03_funktionstypen.rpp` | `n10_mut_am_rueckgabetyp.rpp`, `n11_mut_in_anderem_typ.rpp`, `n12_parameter_passt_nicht.rpp` |
| R-10.15 – R-10.17 Funktionswerte | `a03_funktionstypen.rpp` | `n13_func_kopieren.rpp`, `n14_parameter_speichern.rpp`, `n15_ergebnis_unbenutzt.rpp` |
