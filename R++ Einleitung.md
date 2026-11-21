# R++ Einleitung (Kapitel 1)

Stand: 21.11.2026, Entwurf 0.1. Dieses Dokument füllt Kapitel 1 des „R++ Spezifikationsgerüsts“. Es beschreibt, wofür R++ gedacht ist, wie die Spezifikation aufgebaut ist, welche Begriffe sie verwendet und was Konformität bedeutet.

## 1. Was R++ ist

R++ ist eine objektorientierte, statisch typisierte Sprache mit einer Syntax nahe an C, C++, Java und C#, aber einfacher. Sie verfolgt drei Ziele:

1. **Bestimmte Fehlerklassen sind formal ausgeschlossen.** Ein übersetztes Programm greift nie auf freigegebenen Speicher zu, gibt nichts doppelt frei, dereferenziert nie `null`, liest keine uninitialisierten Werte und verwendet keine ungültigen Iteratoren. Möglich macht das ein Besitzmodell mit `own`, `shared`, `weak` und Borrows, das ohne Lifetime-Annotationen auskommt (Kapitel 5), und Typverengung für nullbare Werte (Kapitel 6). Was sich nicht zur Übersetzungszeit prüfen lässt (Index, Überlauf im Debugmodus, Konflikte bei `shared`-Objekten), endet kontrolliert als Programmfehler (Kapitel 9).
2. **Kurze Entwicklungszyklen und schnelle Programme.** Im Debugmodus wird interpretiert oder per JIT ausgeführt, im Releasemodus nativ übersetzt. Beide Modi zeigen dasselbe beobachtbare Verhalten bis auf eine kleine, festgelegte Liste von Unterschieden (Kapitel 12).
3. **Wenig Überraschungen.** Es gibt kein undefiniertes Verhalten, keine Makros, keine Headerdateien, keine implizite Kopie von Objekten und keine impliziten Umwandlungen, die Werte verändern.

## 2. Aufbau der Spezifikation

| Kap. | Thema | Dokument |
|---|---|---|
| 1 | Einleitung | „R++ Einleitung“ (dieses Dokument) |
| 2 | Lexikalische Struktur | „R++ Spezifikationsgerüst“, Abschnitt 2 |
| 3 | Grammatik | „R++ Spezifikationsgerüst“, Abschnitt 3, und `rpp-grammatik/RPP.g4` (normativ) |
| 4 | Typen | „R++ Typen“ |
| 5 | Ownership und Borrowing | „R++ Ownership-Kernkalkül“ |
| 6 | Null-Sicherheit | „R++ Null-Sicherheit“ |
| 7 | Klassen, Mixins, Typ-Parameter | „R++ Klassen und Generics“ |
| 8 | Anweisungen und Ausdrücke | „R++ Anweisungen und Ausdrücke“ |
| 9 | Exceptions | „R++ Exceptions“ |
| 10 | Lambdas | „R++ Lambdas“ |
| 11 | Pakete und Projekte | „R++ Pakete und Projekte“ |
| 12 | Ausführungsmodell | „R++ Ausführungsmodell“ |
| 13 | Standardbibliothek | „R++ Standardbibliothek“ |
| 14 | Konformitätstests | „R++ Konformitätstests“ |

Die Entscheidungen, auf denen die Kapitel beruhen, stehen mit Datum und Status im Dokument „R++ Designentscheidungen“. Kapitel 5 ist zusätzlich als ausführbares Modell in `fr-modell/` umgesetzt.

## 3. Konventionen

**R-1.1 (Normativ und informativ).** Normativ sind die nummerierten Regeln `R-x.y`, die Grammatik `RPP.g4` und die Tabellen, auf die eine Regel verweist. Beispiele, Begründungen, Überblickstabellen und Anmerkungen sind informativ.

**R-1.2 (Regelnummern).** Jede normative Regel trägt eine Nummer `R-<Kapitel>.<laufende Nummer>`. Nummern werden nicht neu vergeben; entfällt eine Regel, bleibt ihre Nummer frei.

**R-1.3 (Notation).** Syntax steht in ANTLR4-Notation (`:` Definition, `|` Alternative, `?` optional, `*` beliebig oft, `+` mindestens einmal, `'…'` Terminal). Kapitel 5 verwendet zusätzlich Inferenzregeln: Über dem Strich stehen die Voraussetzungen, darunter die Folgerung.

## 4. Begriffe

| Begriff | Bedeutung |
|---|---|
| Programm | Ein Projekt vom Typ `executable` mit allen Bibliotheken, die es verwendet |
| Projekt | Ein Verzeichnis mit `rpp.yaml` und Quellordnern (Kapitel 11) |
| Bibliothek | Ein Projekt vom Typ `library`; seine exportierten Typen sind in anderen Projekten nutzbar |
| Paket | Eine Gruppe von Typen, deren Name dem Verzeichnis im Quellordner entspricht |
| Implementierung | Ein Werkzeug, das R++-Programme prüft und im Debug- und/oder Releasemodus ausführt |
| Debugmodus, Releasemodus | Die beiden Ausführungsarten (Kapitel 12) |
| Fehler | Ein Verstoß gegen eine Regel, den die Implementierung vor der Ausführung meldet; das Programm wird nicht übersetzt |
| Warnung | Eine Meldung, die die Spezifikation verlangt, die aber die Übersetzung nicht verhindert (Kapitel 8, R-8.1) |
| Exception | Ein zur Laufzeit geworfenes, fangbares Objekt (Kapitel 9) |
| Programmfehler | Eine zur Laufzeit erkannte Verletzung (Index, Überlauf im Debug, `fail` …), die nicht gefangen werden kann und das Programm nach dem Aufräumen beendet (Kapitel 9) |
| Besitz, Borrow, Loan | Wer ein Objekt freigibt, wer es vorübergehend nutzt und welche Zugriffe dabei gesperrt sind (Kapitel 5) |
| beobachtbares Verhalten | Ausgaben, geschriebene Dateien, gelesene Eingabe, Exit-Code und ob das Programm endet (Kapitel 12, R-12.4) |

## 5. Konformität

**R-1.4 (Konformes Programm).** Ein Programm ist konform, wenn es gegen keine Regel der Spezifikation verstößt. Für ein konformes Programm legt die Spezifikation das beobachtbare Verhalten fest, bis auf die zulässigen Unterschiede zwischen den Modi (R-12.5).

**R-1.5 (Konforme Implementierung).** Eine Implementierung ist konform, wenn sie
- jedes konforme Programm übersetzt und mit dem festgelegten Verhalten ausführt,
- jedes nicht konforme Programm mit mindestens einem Fehler ablehnt und dabei die Regelnummer nennt,
- die verlangten Warnungen meldet und
- die Konformitäts-Testsuite besteht (Kapitel 14, R-14.7).

**R-1.6 (Grenzen der Implementierung).** Eine Implementierung darf Grenzen haben (Größe des Speichers, Tiefe der Aufrufe), muss aber die Mindestwerte aus Kapitel 12 erfüllen. Überschreitet ein Programm eine Grenze, ist das ein Programmfehler und kein undefiniertes Verhalten.

## 6. Stand

Die Kapitel 4 bis 14 liegen als Entwurf vor. Die Grammatik ist ausführbar und getestet, Kapitel 5 ist als Kernkalkül mit ausführbarem Modell und Zufallstests geprüft. Es gibt noch keine Implementierung; die Konformitätstests sind deshalb bisher nur auf Syntax geprüft. Die erste Implementierung entsteht in Rust („R++ Implementierungsplan“). Offen ist außerdem der maschinelle Beweis von Satz 5.1 (Speichersicherheit).
