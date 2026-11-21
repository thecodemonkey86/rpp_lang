# R++ Konformitätstests (Kapitel 14)

Stand: 21.11.2026, Entwurf 0.2. Dieses Dokument füllt Kapitel 14 des „R++ Spezifikationsgerüsts“. Es legt fest, wie die Konformitäts-Testsuite aufgebaut ist, wie ein Testprogramm sein erwartetes Ergebnis angibt, wann eine Implementierung konform ist und wie die Abdeckung der Regeln geprüft wird.

Alle Festlegungen sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 25 und 26). Jede normative Regel trägt eine Nummer `R-14.n`.

## 0. Ergänzung beim Schließen der Regelabdeckung (angenommen am 21.11.2026)

| Nr. | Vorschlag |
|---|---|
| V-14.1 | Laufzeittests können Kommandozeilenargumente (`// Argumente: …`) und die Standardeingabe (`// Eingabe: "…"`) festlegen (R-14.5). |

## 1. Aufbau

**R-14.1 (Ablage).** Die Testsuite liegt im Verzeichnis `konformitaet/`, je Kapitel in einem Unterverzeichnis `kapNN_thema/` mit bis zu drei Arten von Tests:

| Verzeichnis | Inhalt |
|---|---|
| `akzeptiert/` | Programme, die übersetzt werden müssen, gegebenenfalls mit genau den markierten Warnungen |
| `abgelehnt/` | Programme, die mit genau einem Fehler einer bestimmten Regel in einer bestimmten Zeile abgelehnt werden müssen |
| `laufzeit/` | Programme mit `main`, die ausgeführt werden und eine festgelegte Ausgabe und einen festgelegten Exit-Code liefern müssen |

Ein Test ist eine einzelne Datei `name.rpp`. Tests, die mehrere Dateien oder eine Projektdatei brauchen (vor allem für Kapitel 11), sind ein Verzeichnis `name/` mit `rpp.yaml` (**Projekttest**). Die Kopfzeilen stehen dann als YAML-Kommentare (`# Regel: …`) am Anfang dieser `rpp.yaml`. Ein Projekttest kann weitere Projekte in Unterverzeichnissen enthalten, etwa Bibliotheken, auf die er mit `path` verweist; sie gehören zum selben Test.

**R-14.2 (Namen).** Dateinamen beginnen mit `a` (akzeptiert), `n` (abgelehnt, „negativ“) oder `l` (Laufzeit), gefolgt von einer zweistelligen Nummer und einer kurzen Beschreibung: `n05_return_in_finally.rpp`.

## 2. Kopfzeilen und Markierungen

**R-14.3 (Akzeptierte Programme).** Die erste Zeile ist `// Konformitätstest Kapitel N – muss übersetzt werden.` Optional folgt `// Regel: R-x.y, …`. Erwartet das Programm Warnungen, sagt der Kopf das (`// Erwartet: genau eine Warnung, …`), und jede Zeile mit einer Warnung trägt am Ende `// WARNUNG`. Ein akzeptiertes Programm enthält kein `// FEHLER`.

**R-14.4 (Abgelehnte Programme).** Der Kopf enthält
- `// Regel: R-x.y …` – die erste genannte Regel ist die, deren Fehler erwartet wird, die übrigen erklären den Zusammenhang;
- `// Erwartet: genau ein Fehler, in der mit FEHLER markierten Zeile (Begründung).`

Genau eine Zeile trägt am Ende `// FEHLER`, in YAML-Dateien eines Projekttests `# FEHLER`. Das Programm enthält sonst keinen Fehler; ohne die markierte Zeile wäre es korrekt oder höchstens unvollständig. Der erwartete Fehler darf ein Syntaxfehler oder ein Fehler einer Direktive sein (etwa für `??=` oder `#define`); dann muss der erste gemeldete Fehler in der markierten Zeile liegen.

**R-14.5 (Laufzeittests).** Der Kopf enthält `// Regel: …` und eine oder zwei Zeilen

```
// Erwartet: Ausgabe "…"; Exit-Code n; Fehlerausgabe enthält "…"
// Erwartet (debug): …
// Erwartet (release): …
```

Optional legen zwei weitere Kopfzeilen fest, womit das Programm gestartet wird (V-14.1):

```
// Argumente: eins zwei
// Eingabe: "a\nb\n"
```

`Argumente` sind durch Leerzeichen getrennte Wörter, die `main` in `args` erhält; `Eingabe` ist der vollständige Text der Standardeingabe in der Schreibweise eines String-Literals. Fehlen sie, gibt es keine Argumente und eine leere Eingabe.

Ohne Modus gilt die Erwartung für beide Modi. `Ausgabe` ist der vollständige Text der Standardausgabe; Zeilen werden durch `\n` getrennt, der letzte Zeilenumbruch fehlt. `Fehlerausgabe enthält` prüft nur, ob der Text vorkommt, weil Meldungen sonst je Implementierung abweichen dürfen. Fehlt `Ausgabe`, wird die Standardausgabe nicht geprüft.

**R-14.6 (Regelabdeckung).** Jede Regel `R-x.y` aus den Kapiteln soll in der Kopfzeile mindestens eines Tests genannt sein, wo es sinnvoll ist in einem akzeptierten und einem abgelehnten. Der Testlauf (R-14.8) vergleicht die in den Kapiteldokumenten definierten Regeln mit den in Testköpfen genannten und listet die fehlenden; eine Lücke ist kein Fehlschlag. Die Regeln der Kapitel 1 und 14 betreffen die Spezifikation und die Testsuite selbst und zählen nicht mit. Regeln, die sich mit einem einzelnen Testprogramm nicht prüfen lassen, stehen mit Begründung in Abschnitt 5; sie gelten nicht als Lücke.

## 3. Konformität

**R-14.7 (Konforme Implementierung).** Eine Implementierung ist konform, wenn sie für jeden Test der Suite in jedem Modus, in dem sie arbeitet, das Erwartete tut:
- akzeptierte Programme übersetzt sie, mit genau den markierten Warnungen und keinen weiteren;
- abgelehnte Programme lehnt sie mit einem Fehler der genannten Regel in der markierten Zeile ab;
- Laufzeittests führt sie im Debug- und im Releasemodus aus und liefert Ausgabe, Exit-Code und Fehlerausgabe wie erwartet.

Eine Implementierung, die nur einen Modus anbietet, ist für diesen Modus konform. Die Regelnummer eines Fehlers gibt die Implementierung in ihrer Meldung an; der Wortlaut ist frei.

## 4. Werkzeuge und Stand

**R-14.8 (Testläufe).** Bis es eine Implementierung gibt, prüfen zwei Werkzeuge, was ohne sie prüfbar ist:

| Werkzeug | Prüft |
|---|---|
| `rpp-grammatik/tools/run_tests.py` | dass alle Testprogramme syntaktisch gültig sind, einschließlich aller Dateien eines Projekttests; dass ein erwarteter Syntaxfehler in der markierten Zeile liegt; die Markierungen `FEHLER` und die Kopfzeilen der Laufzeittests; die Regelabdeckung (R-14.6) |
| `fr-modell/run_tests.py`, `fr-modell/fuzz.py` | die Regeln von Kapitel 5 an der Kernsprache FR++, einschließlich Ausführung und Zufallstests |

Die Semantik der übrigen Kapitel ist erst mit einer Implementierung prüfbar.

**Stand vom 21.11.2026:**

| Kapitel | akzeptiert | abgelehnt | Laufzeit | Regeln mit Test |
|---|---|---|---|---|
| 4 Typen | 8 | 27 | 2 | 47 von 47 |
| 5 Ownership (R++ und FR++) | 6 + 10 | 20 + 24 | – | 24 von 24 |
| 6 Null-Sicherheit | 5 | 17 | 2 | 23 von 23 |
| 7 Klassen und Generics | 6 | 30 | 3 | 28 von 28 |
| 8 Anweisungen | 5 | 22 | 4 | 26 von 28 (+ 2 ohne Test) |
| 9 Exceptions | 4 | 10 | 6 | 19 von 19 |
| 10 Lambdas | 4 | 16 | 1 | 18 von 18 |
| 11 Pakete und Projekte | 4 | 13 | 1 | 21 von 25 (+ 4 ohne Test) |
| 12 Ausführungsmodell | – | – | 10 | 8 von 12 (+ 4 ohne Test) |
| 13 Standardbibliothek | 5 | 7 | 4 | 19 von 20 (+ 1 ohne Test) |
| **gesamt** | | | | **233 von 244**, dazu 11 begründet ohne Test |

Projekttests zählen als ein Test. Jede Regel der Kapitel 4 bis 13 ist in mindestens einem Testkopf genannt oder in Abschnitt 5 begründet. Geprüft sind die Tests bisher nur auf Syntax; ob ihre Erwartungen stimmen, zeigt erst eine Implementierung.

## 5. Regeln ohne eigenen Test

Diese Regeln lassen sich nicht mit einem einzelnen Testprogramm prüfen (R-14.6). Der Testlauf liest die Tabelle und zählt sie nicht als Lücke.

| Regel | Begründung |
|---|---|
| R-8.11 | Verweis auf die Kapitel 4, 6, 7 und 10; geprüft durch deren Tests |
| R-8.28 | Verweis auf Kapitel 9; geprüft durch dessen Tests |
| R-11.19 | Minimal Version Selection braucht Bibliotheken aus einem Verzeichnis; es gibt noch kein Testverzeichnis |
| R-11.20 | Prüfsummen in `rpp.lock` gibt es nur für Bibliotheken aus einem Verzeichnis (wie R-11.19) |
| R-11.21 | regelt das Verzeichnis, nicht die Implementierung |
| R-11.22 | die Zwischenrepräsentation ist nicht beobachtbar (R-12.4) |
| R-12.1 | Begriffsbestimmung |
| R-12.2 | durch R-14.7 abgedeckt: jeder Test gilt für beide Modi |
| R-12.3 | betrifft die Entwicklung der Implementierung, nicht ein Programm |
| R-12.12 | Version 1 hat keine Threads |
| R-13.2 | betrifft die Versionierung der Bibliothek, nicht ein Programm |
