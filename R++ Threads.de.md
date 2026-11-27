# R++ Threads (Kapitel 16)

[English](R%2B%2B%20Threads.md)

Stand: 27.11.2026, Entwurf 0.1. Dieses Dokument füllt Kapitel 16 der „R++ Spezifikationsübersicht“. Es legt fest, wie ein R++-Programm Code in mehreren Threads ausführt: Threads starten und auf sie warten, Bereiche, deren Threads Daten des umgebenden Blocks leihen, welche Werte in einen anderen Thread gelangen dürfen, Kanäle als einziger Weg zum Austausch von Daten, Ausnahmen und Programmfehler in Threads und was beobachtbar bleibt, wenn mehrere Threads laufen.

Die Grundzüge sind entschieden (siehe „R++ Designentscheidungen“, Abschnitt 12, Meilenstein M11 im Implementierungsplan). Die Vorschläge in Abschnitt 0 sind beim Ausformulieren entstanden und wurden am 27.11.2026 angenommen, V-16.9 in geänderter Form (alle Threads werden abgewickelt); die Regeln, die auf ihnen beruhen, nennen sie. Jede normative Regel trägt eine Nummer `R-16.n`. Die Konformitätstests werden in `conformance/ch16_threads/` liegen.

## 0. Ergänzungen aus dem Ausformulieren (angenommen am 27.11.2026)

| Nr. | Vorschlag |
|---|---|
| V-16.1 | `void` ist kein Typargument (R-4.42), deshalb gibt es zwei Handle-Typen: `Thread::spawn` mit einem `own Func<void>` liefert ein `own Thread`, dessen `join()` nichts zurückgibt; mit einem `own Func<T>` liefert es ein `own Task<T>`, dessen `join()` `own T` zurückgibt. Das ersetzt das in der Entscheidung genannte `own Thread<T>`. |
| V-16.2 | Ein `own`-Lambda kann seine Erfassungen nicht ändern (R-10.11). Für veränderliche Arbeitsdaten übergibt `Thread::spawnWith(own D input, own Func<own D, …> body)` dem Rumpf `input` als Parameter, den er besitzt. |
| V-16.3 | `Channel<T>::open(capacity)` liefert ein Tupel `(own Sender<T>, own Receiver<T>)`. Es gibt einen Empfänger; ein Sender lässt sich mit `clone()` verdoppeln. `send`, `receive` und `clone` sind keine `mut`-Methoden: Die Enden sind innen synchronisiert, deshalb lässt sich auch ein geliehenes Ende benutzen. |
| V-16.4 | Der Elementtyp eines Kanals ist nicht nullbar. `receive()` liefert `T?`: `null` heißt, der Kanal ist leer und jeder Sender ist freigegeben. `send` liefert `false`, wenn der Empfänger freigegeben ist; der Wert wird dann freigegeben. |
| V-16.5 | Ob ein Typ in einen anderen Thread gelangen darf, drückt die eingebaute Einschränkung `Transferable` in `rpp.sync` aus. Jeder Typ, der R-16.7 erfüllt, hat sie von selbst; eine Klasse kann sie nicht importieren. Generischer Code, der Threads mit Werten eines Typparameters startet, schreibt `T : Transferable`. |
| V-16.6 | Eine Ausnahme gelangt in den wartenden Thread (R-16.5). Deshalb muss jede von `Exception` abgeleitete Klasse übertragbar sein; ein `shared`- oder `weak`-Feld in ihrem Baum ist ein Fehler, in Programmen mit und ohne Threads. |
| V-16.7 | Neue Art von Programmfehler `ThreadError` (R-9.2): ein zweites `join`, ein Thread, der auf sich selbst wartet, ein Handle, das freigegeben wird, während sein Thread mit einer Ausnahme endete, die niemand abgeholt hat, eine Verklemmung (R-16.19), eine Kanalkapazität unter 1, ein Thread, der sich nicht starten lässt. |
| V-16.8 | Mit mehreren Threads hat ein Programm statt genau eines Verhaltens eine Menge erlaubter Verhalten (Ergänzung zu R-12.6): Jede Verzahnung, die die Reihenfolge aus R-16.18 einhält, ist erlaubt. Ein einzelner Ausgabeaufruf wird nie mit einem anderen verzahnt. |
| V-16.9 | Ein Programmfehler in einem beliebigen Thread beendet das ganze Programm mit Exit-Code 2, und vorher wird jeder Thread abgewickelt: der fehlerhafte wie in R-9.14, die anderen, nachdem sie an ihrer nächsten Abbruchstelle abgebrochen wurden (R-16.6). Nur ein Thread, der beim Lesen von Eingabe oder in einem C-Aufruf festhängt, hält ohne Abwicklung an. |
| V-16.10 | `Thread::sleep(i64 millis)` lässt den aktuellen Thread warten. Schlafen zählt nicht als Warten im Sinne von R-16.19. |
| V-16.11 | `Scope.spawn` liefert kein Handle, deshalb kann ein Bereich Threads ohne `discard` starten (R-8.13). Am Ende des Bereichs wird die Ausnahme des ersten Threads in Startreihenfolge, der mit einer endete, erneut geworfen; die anderen werden freigegeben. |

## 1. Threads

**R-16.1 (rpp.sync).** Das Paket `rpp.sync` der Standardbibliothek stellt Threads und Kanäle bereit:

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

Keine dieser Klassen lässt sich mit `new` erzeugen oder ableiten. `Thread`, `Task`, `Sender` und `Receiver` gibt es nur als `own`-Werte; `copy` ist für alle ein Fehler.

**R-16.2 (Start).** `Thread::spawn` und `Thread::spawnWith` starten einen neuen Thread, der `body` nebenläufig zum aufrufenden Thread und zu allen anderen Threads ausführt; `spawnWith` übergibt `input` an `body` als dessen Parameter (V-16.2). Der Aufruf kehrt zurück, sobald der Thread gestartet ist. Das Lambda und `input` gehören von da an dem neuen Thread. Lässt sich der Thread nicht starten, ist das ein Programmfehler `ThreadError`.

**R-16.3 (Warten).** `join()` wartet, bis der Thread geendet hat. `Task<T>.join()` gibt das Ergebnis von `body` als `own T` zurück; `Thread.join()` gibt nichts zurück. Auf ein Handle lässt sich einmal warten; ein zweites `join()` ist ein Programmfehler `ThreadError`, ebenso ein `join()` des Threads auf sein eigenes Handle.

**R-16.4 (Freigabe eines Handles).** Die Freigabe eines `Thread` oder `Task`, auf das noch nicht gewartet wurde, wartet, bis sein Thread geendet hat; ein Ergebnis wird dann freigegeben. Endete der Thread mit einer Ausnahme (R-16.5), ist die Freigabe seines Handles ein Programmfehler `ThreadError`, dessen Meldung `describe()` der Ausnahme enthält. Ein Handle kann deshalb nie das Programmende überdauern, ohne dass sein Thread geendet hat (R-16.20).

**R-16.5 (Ausnahmen).** Eine Ausnahme, die `body` verlässt, beendet den Thread nach der Abwicklung (R-9.11). `join()` wirft sie im wartenden Thread erneut; das Ausnahmeobjekt wandert in diesen Thread (V-16.6). Im Debugmodus zeigt der Stacktrace (R-9.16) den Verlauf des endenden Threads.

**R-16.6 (Programmfehler).** Ein Programmfehler in einem beliebigen Thread bricht alle anderen Threads ab und beendet das Programm, nachdem jeder Thread abgewickelt ist (V-16.9):
- der fehlerhafte Thread wird wie in R-9.14 abgewickelt;
- jeder andere Thread wird an seiner nächsten Abbruchstelle abgebrochen: einem Methodenaufruf, dem nächsten Durchlauf einer Schleife oder einem Warten in `join()`, bei der Freigabe eines Handles, am Ende von `Thread::scope`, in `send`, `receive` oder `Thread::sleep`. Von dort wird er wie bei einem Programmfehler abgewickelt: Destruktoren und `finally`-Blöcke laufen, kein `catch` wird gewählt;
- ein Thread, der auf Eingabe wartet oder in einem C-Aufruf steckt, wird abgebrochen, wenn dieser Aufruf zurückkehrt; haben alle anderen Threads ihre Abwicklung beendet und steckt er noch in einem solchen Aufruf, hält er ohne Abwicklung an;
- dann endet das Programm mit der Meldung des ersten Programmfehlers auf der Fehlerausgabe und Exit-Code 2. Weitere Programmfehler während der Abwicklung werden nicht gemeldet.

## 2. Übergabe zwischen Threads

**R-16.7 (Übertragbare Typen).** Ein Typ ist übertragbar, wenn ein Wert von ihm keine Referenz enthält, die ein anderer Thread gleichzeitig benutzen könnte:
- primitive Typen, `string`, Enums und Handle-Typen (Kapitel 15) sind übertragbar;
- `T?`, `T[]` und Tupel sind übertragbar, wenn ihre Elementtypen es sind;
- ein Klassentyp ist übertragbar, wenn kein Feld der Klasse, ihrer Basisklassen und keiner Unterklasse im Programm einen Typ mit `shared` oder `weak` oder einen nicht übertragbaren Typ hat; bei generischen Klassen mit eingesetzten Typargumenten;
- ein Mixin-Typ ist übertragbar, wenn jede Klasse im Programm, die das Mixin importiert, es ist;
- `Sender<T>`, `Receiver<T>`, `Task<T>` und `Thread` sind übertragbar; `Scope` und `Func<…>` nicht.

Strings werden intern geteilt; Implementierungen zählen ihre Referenzen atomar (R-12.12).

**R-16.8 (Einschränkung Transferable).** `Transferable` (V-16.5) ist eine Einschränkung wie ein Mixin-Typ (R-7.23), die jeder übertragbare Typ erfüllt. Sie lässt sich nicht importieren und ist kein Typ von Werten. Ein Typparameter mit dieser Einschränkung ist übertragbar, einer ohne sie nicht.

**R-16.9 (Was beim Start hinüber geht).** Bei `Thread::spawn` und `spawnWith` darf das `own`-Lambda nur Werte übertragbarer Typen erfassen (R-10.8), und `D` und `T` müssen übertragbar sein. Ein gewöhnliches Lambda oder ein vorhandener Funktionswert wird nicht angenommen: `body` muss beim Aufruf ein `own`-Lambda-Ausdruck sein, damit sich seine Erfassungen prüfen lassen.

**R-16.10 (Ausnahmen gehen hinüber).** Jede von `Exception` abgeleitete Klasse muss übertragbar sein (V-16.6). Ein Feld vom Typ `shared`, `weak` oder eines nicht übertragbaren Typs in einer solchen Klasse oder einer ihrer Unterklassen ist ein Fehler am Feld.

## 3. Bereiche

**R-16.11 (Bereich).** `Thread::scope(body)` ruft `body` mit einem neuen `Scope` auf und kehrt erst zurück, wenn `body` geendet hat und jeder mit `Scope.spawn` gestartete Thread geendet hat. `Scope.spawn(f)` startet einen Thread, der `f` ausführt; es liefert kein Handle (V-16.11).

**R-16.12 (Leihen in einem Bereich).** Das an `Scope.spawn` übergebene Lambda muss beim Aufruf ein Lambda-Ausdruck sein. Es darf ein `own`-Lambda sein; dann gilt R-16.9. Ein gewöhnliches Lambda darf nur lesend leihen, und nur Variablen, die das an `Thread::scope` übergebene Lambda erfasst; ihre Typen müssen übertragbar sein. Diese Variablen gelten für den ganzen Aufruf von `Thread::scope` als von `body` lesend geliehen (R-10.7), deshalb kann auch `body` sie nicht ändern. Eine veränderliche Leihe, eine Leihe einer lokalen Variablen von `body` und `this` mit nicht übertragbarem Typ sind Fehler.

**R-16.13 (Ausnahmen in einem Bereich).** Eine Ausnahme, die `body` oder einen Thread des Bereichs verlässt, beendet die anderen Threads des Bereichs nicht. Wenn alle geendet haben, wirft `Thread::scope` die Ausnahme von `body`, falls es eine gibt, sonst die des ersten Threads in Startreihenfolge, der mit einer Ausnahme endete; alle anderen Ausnahmen werden freigegeben (V-16.11).

## 4. Kanäle

**R-16.14 (Öffnen).** `Channel<T>::open(capacity)` erzeugt einen Kanal, der bis zu `capacity` Werte fasst, und liefert seine beiden Enden. Eine Kapazität unter 1 ist ein Programmfehler `ThreadError`. `T` darf kein nullbarer Typ sein (V-16.4).

**R-16.15 (Senden).** `send(value)` verschiebt `value` in den Kanal und liefert `true`. Ist der Kanal voll, wartet der Aufruf, bis Platz frei wird. Ist der Empfänger freigegeben, gibt `send` `value` frei und liefert `false`, auch während des Wartens.

**R-16.16 (Empfangen).** `receive()` nimmt den ältesten Wert aus dem Kanal und liefert ihn. Ist der Kanal leer, wartet der Aufruf, bis ein Wert ankommt. Ist der Kanal leer und jeder Sender freigegeben, liefert `receive()` `null`, auch während des Wartens.

**R-16.17 (Sender).** `clone()` liefert einen weiteren Sender desselben Kanals. Die Werte eines Senders kommen in der Reihenfolge an, in der er sie gesendet hat; Werte verschiedener Sender dürfen in jeder Reihenfolge ankommen, die R-16.18 einhält.

## 5. Reihenfolge und Programmende

**R-16.18 (Reihenfolge).** Innerhalb eines Threads geschieht alles in Programmreihenfolge (R-8.2). Zwischen Threads gelten genau diese Reihenfolgen:
- alles vor `spawn`, `spawnWith` oder `Scope.spawn` geschieht vor allem im gestarteten Thread;
- alles in einem Thread geschieht vor der Rückkehr seines `join()`, vor der Freigabe seines Handles (R-16.4) und vor dem Ende seines `Thread::scope`;
- alles vor einem `send` geschieht vor allem nach dem `receive`, das diesen Wert nimmt;
- alles vor der Freigabe des letzten Senders geschieht vor allem nach dem `receive`, das `null` liefert.

Wirkungen, die diese Regeln nicht ordnen, dürfen in jeder Reihenfolge geschehen (V-16.8). Ein einzelner Aufruf, der auf die Standard- oder Fehlerausgabe schreibt oder von der Standardeingabe liest, wird nicht mit einem anderen solchen Aufruf verzahnt.

**R-16.19 (Verklemmung).** Wartet jeder Thread des Programms in `join()`, bei der Freigabe eines Handles, am Ende von `Thread::scope`, in `send` oder in `receive`, und kann keiner von ihnen weitermachen, ist das ein Programmfehler `ThreadError`. `Thread::sleep` und das Lesen von Eingabe zählen nicht als Warten (V-16.10).

**R-16.20 (Programmende).** R-12.9 gilt unverändert. Weil jeder Thread zu einem Handle oder einem Bereich gehört und die Freigabe der Handles von `main` auf ihre Threads wartet (R-16.4), endet das Programm erst, wenn alle Threads geendet haben, außer nach einem Programmfehler (R-16.6).

**R-16.21 (Ressourcen).** R-12.10 gilt für jeden Thread: Jeder Thread erlaubt mindestens 10.000 verschachtelte Methodenaufrufe (V-12.2).

**R-16.22 (Destruktoren).** Ein Destruktor läuft in dem Thread, der das Objekt freigibt. Ein Objekt führt deshalb nie in zwei Threads gleichzeitig Code aus.

**R-16.23 (Beide Modi).** Threads verhalten sich im Debug- und Releasemodus gleich (R-12.5). C-Code darf aus jedem Thread gerufen werden; R-15.16 gilt für jeden Aufruf. Wie eine Implementierung Threads auf das System abbildet, ist ihr überlassen: Sie darf jeden Thread als Thread des Betriebssystems ausführen oder als leichten Thread (Koroutine) mit eigenem Stapel, den ein Scheduler der Laufzeit auf weniger Threads des Betriebssystems verteilt, solange R-16.18 und R-16.19 gelten. Bei leichten Threads lässt das Warten in `join()`, `send`, `receive`, am Ende von `Thread::scope`, in `Thread::sleep` und bei Ein- und Ausgabe andere Threads laufen; ein Thread, der lange rechnet, darf die anderen nicht für immer am Laufen hindern. Damit bleibt der Weg zu vielen gleichzeitigen Wartevorgängen offen, ohne neue Syntax wie `async` und `await`.

## 6. Beispiel

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
        Squares::produce(tx, n);          // übernimmt den Sender und gibt ihn am Ende frei
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

`sumTo` verschiebt den Empfänger mit `spawnWith` in den Thread, weil der Rumpf seine Arbeitsdaten ändert (V-16.2). `produce` besitzt den Sender; wenn es zurückkehrt, wird der Sender freigegeben, das letzte `receive()` liefert `null` (R-16.16), und das Ergebnis kommt über `join()`. In `main` leihen beide Threads des Bereichs `data` lesend; die beiden Zeilen dürfen in beliebiger Reihenfolge erscheinen (R-16.18), jede als Ganzes.

## 7. Änderungen an anderen Kapiteln

Mit diesem Kapitel haben sich die folgenden Stellen geändert (eingetragen am 27.11.2026):

| Stelle | Änderung |
|---|---|
| R-4.42, R-7.23 | die eingebaute Einschränkung `Transferable` (V-16.5) |
| R-9.2 | neue Art `ThreadError` (V-16.7) |
| R-9.4 | von `Exception` abgeleitete Klassen müssen übertragbar sein (V-16.6, R-16.10) |
| R-12.4, R-12.6 | mehrere erlaubte Verhalten mit Threads; Ausgabeaufrufe werden nicht verzahnt (V-16.8) |
| R-12.12 | verweist auf dieses Kapitel statt „Version 1 hat keine Threads“ |
| Standardbibliothek | neuer Abschnitt `rpp.sync` mit der Schnittstelle aus R-16.1; Fahrplan Phase C |
| Ownership-Kernkalkül, Abschnitt 7 | Verweis auf R-16.7 und R-16.12 |

Keine Änderung an `RPP.g4`: Threads, Bereiche und Kanäle sind Klassen der Standardbibliothek.

## 8. Konformitätstests

Die Laufzeittests sind so geschrieben, dass ihre Ausgabe nicht von der Verzahnung abhängt: Entweder schreibt nur ein Thread, oder die Reihenfolge folgt aus R-16.18. Der Testlauf führt jeden Laufzeittest dieses Kapitels mehrfach aus.

| Regeln | Angenommen oder Laufzeit | Abgelehnt |
|---|---|---|
| R-16.1 bis R-16.3 | `l01_spawn_join`, `l02_task_result` | `n01_new_thread`, `n02_copy_task` |
| R-16.2, V-16.2 | `l03_spawn_with_input` | – |
| R-16.3 | `l04_join_twice` (Laufzeit: `ThreadError`) | – |
| R-16.4 | `l05_release_waits`, `l06_release_with_exception` (Laufzeit: `ThreadError`) | – |
| R-16.5 | `l07_join_rethrows` | – |
| R-16.6 | `l08_program_error_in_thread` (Laufzeit: Exit-Code 2; die Destruktoren des anderen Threads laufen) | – |
| R-16.7 bis R-16.9 | `a01_transferable_generic` | `n03_capture_shared`, `n04_field_weak`, `n05_subclass_shared`, `n06_ordinary_lambda`, `n07_generic_without_constraint` |
| R-16.10 | – | `n08_exception_with_shared` |
| R-16.11 bis R-16.13 | `l09_scope_borrows`, `l10_scope_exception` | `n09_scope_mut_borrow`, `n10_scope_local_borrow`, `n11_scope_change_borrowed` |
| R-16.14 bis R-16.17 | `l11_channel_order`, `l12_channel_closed`, `l13_receiver_gone`, `l14_clone_sender` | `n12_nullable_element` |
| R-16.14 | `l15_capacity_zero` (Laufzeit: `ThreadError`) | – |
| R-16.18 | `l16_order_through_channel` | – |
| R-16.19 | `l17_deadlock` (Laufzeit: `ThreadError`) | – |
| R-16.20 bis R-16.22 | `l18_end_waits`, `l19_destructor_thread` | – |

R-16.23 lässt sich nicht mit einzelnen Programmen prüfen; es ist dadurch abgedeckt, dass alle Laufzeittests dieses Kapitels in beiden Modi laufen.
