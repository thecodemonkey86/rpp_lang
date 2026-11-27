# R++ Konformitätstests (Kapitel 14)

[English](R%2B%2B%20Conformance%20Tests.md)

Stand: 22.11.2026, Entwurf 0.4. Dieses Dokument füllt Kapitel 14 des „R++ Spezifikationsgerüsts“. Es legt fest, wie die Konformitäts-Testsuite aufgebaut ist, wie ein Testprogramm sein erwartetes Ergebnis angibt, wann eine Implementierung konform ist und wie die Abdeckung der Regeln geprüft wird.

Alle Festlegungen sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 25 und 26), einschließlich der Klarstellungen V-14.2 und V-14.3. Jede normative Regel trägt eine Nummer `R-14.n`.

## 0. Ergänzung beim Schließen der Regelabdeckung (angenommen am 21.11.2026)

| Nr. | Vorschlag |
|---|---|
| V-14.1 | Laufzeittests können Kommandozeilenargumente (`// Argumente: …`) und die Standardeingabe (`// Eingabe: "…"`) festlegen (R-14.5). |
| V-14.3 | (angenommen am 22.11.2026, aus Meilenstein M5) Jeder Laufzeittest läuft je Modus in einem frischen, leeren Arbeitsverzeichnis. Er darf dort Dateien und Verzeichnisse anlegen, lesen und löschen; relative Pfade beziehen sich darauf. Danach wird das Verzeichnis gelöscht. Außerhalb davon schreibt ein Test nichts. Damit lassen sich `rpp.io` (R-13.19, R-13.20) ohne vorbereitete Dateien prüfen, und Tests stören sich nicht gegenseitig. |
| V-14.2 | (angenommen am 21.11.2026, aus Meilenstein M3) Eine Fehlermeldung darf mehrere Regeln nennen: die verletzte Regel und die Regeln, die begründen, warum sie verletzt ist. Beispiel: `x.run()` nach `x = null;` verletzt R-6.2, begründet durch R-6.11; die Meldung nennt `[R-6.2, R-6.11]`. Ein abgelehnter Test ist erfüllt, wenn eine der genannten Regeln die erwartete ist (R-14.7). Ohne diese Klarstellung müsste die Hauptregel je nach Ursache wechseln, obwohl der Verstoß derselbe ist. |

## 1. Aufbau

**R-14.1 (Ablage).** Die Testsuite liegt im Verzeichnis `conformance/`, je Kapitel in einem Unterverzeichnis `chNN_topic/` mit bis zu drei Arten von Tests:

| Verzeichnis | Inhalt |
|---|---|
| `accepted/` | Programme, die übersetzt werden müssen, gegebenenfalls mit genau den markierten Warnungen |
| `rejected/` | Programme, die mit genau einem Fehler einer bestimmten Regel in einer bestimmten Zeile abgelehnt werden müssen |
| `runtime/` | Programme mit `main`, die ausgeführt werden und eine festgelegte Ausgabe und einen festgelegten Exit-Code liefern müssen |

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

Ein Laufzeittest startet je Modus in einem frischen, leeren Arbeitsverzeichnis, das er beschreiben darf und das danach gelöscht wird (V-14.3).

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
- abgelehnte Programme lehnt sie mit einem Fehler der genannten Regel in der markierten Zeile ab (die Regel darf als Begründung neben der verletzten Regel stehen, V-14.2);
- Laufzeittests führt sie im Debug- und im Releasemodus aus und liefert Ausgabe, Exit-Code und Fehlerausgabe wie erwartet.

Eine Implementierung, die nur einen Modus anbietet, ist für diesen Modus konform. Die Regelnummer eines Fehlers gibt die Implementierung in ihrer Meldung an; der Wortlaut ist frei.

## 4. Werkzeuge und Stand

**R-14.8 (Testläufe).** Diese Werkzeuge prüfen die Testsuite:

| Werkzeug | Prüft |
|---|---|
| `rpp-grammar/tools/run_tests.py` | dass alle Testprogramme syntaktisch gültig sind, einschließlich aller Dateien eines Projekttests; dass ein erwarteter Syntaxfehler in der markierten Zeile liegt; die Markierungen `FEHLER` und die Kopfzeilen der Laufzeittests; die Regelabdeckung (R-14.6) |
| `fr-model/run_tests.py`, `fr-model/fuzz.py` | die Regeln von Kapitel 5 an der Kernsprache FR++, einschließlich Ausführung und Zufallstests |
| `fr-model/compare.py` | dass `rppc` FR++-Programme, übersetzt nach Abschnitt 7 von Kapitel 5, genauso annimmt oder ablehnt wie das Modell (handgeschriebene und Zufallsprogramme) |
| `rppc test` (erste Implementierung, ab Meilenstein M2) | übersetzen oder ablehnen nach R-14.7: akzeptierte und Laufzeittests ohne Fehler und mit Warnungen genau in den `WARNUNG`-Zeilen; abgelehnte nur mit Fehlern in der markierten Zeile, darunter einer der erwarteten Regel (V-14.2). Laufzeittests führt der Interpreter in beiden Modi aus und vergleicht Ausgabe, Exit-Code und Fehlerausgabe (ab M4). |
| `rppcc test` (R++-Compiler in R++, ab Meilenstein M8) | dasselbe wie `rppc test` mit demselben Urteil je Test; Laufzeittests werden in beiden Modi zu nativen Programmen übersetzt (C-Codegenerator und C-Compiler) und in einem frischen, leeren Arbeitsverzeichnis ausgeführt. Damit ist die Implementierung auch für den Releasemodus nach R-14.7 geprüft. |

Stand 26.11.2026 (M9): `rppc test` und `rppcc test` bestehen alle 332 Tests; neu sind 13 Tests zum Verzeichnis im Dateisystem (Kapitel 11, Abschnitt 5a) und `l50_sha256` (R-13.46). Davor, Stand 26.11.2026 (M8): `rppc test` und `rppcc test` bestehen alle 318 Tests, `rppcc test` unter Windows (MSVC) und Linux (gcc) mit nativen Programmen in beiden Modi; neu sind die Laufzeittests für Prozesse, den Programmpfad und `Dir::removeAll` (R-13.43 bis R-13.45). Die Semantik aller Kapitel prüft `rppc test`; Stand 23.11.2026: alle 293 Tests bestehen (übersetzen bzw. ablehnen, Laufzeittests in beiden Modi ausgeführt), darunter neue Tests für `rpp.random` (R-13.23 bis R-13.25), die Vektor-Algorithmen (R-13.26, R-13.27), `rpp.math` (R-13.28, R-13.29), die Text-Hilfen (R-13.30), die Datei-Operationen (R-13.31) sowie `Deque` und `getOrPut` (R-13.32, R-13.33).

**Stand vom 21.11.2026:**

| Kapitel | akzeptiert | abgelehnt | Laufzeit | Regeln mit Test |
|---|---|---|---|---|
| 4 Typen | 8 | 27 | 2 | 47 von 47 |
| 5 Ownership (R++ und FR++) | 7 + 11 | 26 + 25 | 3 | 25 von 25 |
| 6 Null-Sicherheit | 5 | 17 | 2 | 23 von 23 |
| 7 Klassen und Generics | 6 | 30 | 4 | 28 von 28 |
| 8 Anweisungen | 5 | 22 | 4 | 26 von 28 (+ 2 ohne Test) |
| 9 Exceptions | 4 | 10 | 6 | 19 von 19 |
| 10 Lambdas | 4 | 16 | 1 | 18 von 18 |
| 11 Pakete und Projekte | 4 | 13 | 1 | 21 von 25 (+ 4 ohne Test) |
| 12 Ausführungsmodell | – | – | 10 | 8 von 12 (+ 4 ohne Test) |
| 13 Standardbibliothek | 5 | 7 | 9 | 19 von 20 (+ 1 ohne Test) |
| **gesamt** | | | | **234 von 245**, dazu 11 begründet ohne Test |

Projekttests zählen als ein Test. Jede Regel der Kapitel 4 bis 13 ist in mindestens einem Testkopf genannt oder in Abschnitt 5 begründet. Alle Tests sind auf Syntax geprüft und mit der ersten Implementierung auch semantisch (übersetzen oder ablehnen); die Ausgaben der Laufzeittests prüft sie ab Meilenstein M4.

## 5. Regeln ohne eigenen Test

Diese Regeln lassen sich nicht mit einem einzelnen Testprogramm prüfen (R-14.6). Der Testlauf liest die Tabelle und zählt sie nicht als Lücke.

| Regel | Begründung |
|---|---|
| R-8.11 | Verweis auf die Kapitel 4, 6, 7 und 10; geprüft durch deren Tests |
| R-8.28 | Verweis auf Kapitel 9; geprüft durch dessen Tests |
| R-11.31 | `publish` und `withdraw` sind Befehle und keine Programme; geprüft von `.github/scripts/check.py registry` (Kapitel 11, Abschnitt 8) |
| R-11.22 | die Zwischenrepräsentation ist nicht beobachtbar (R-12.4) |
| R-12.1 | Begriffsbestimmung |
| R-12.2 | durch R-14.7 abgedeckt: jeder Test gilt für beide Modi |
| R-12.3 | betrifft die Entwicklung der Implementierung, nicht ein Programm |
| R-12.12 | Zusammenfassung von Kapitel 16; durch dessen Tests abgedeckt |
| R-13.2 | betrifft die Versionierung der Bibliothek, nicht ein Programm |
| R-15.16 | Grenze der Zusagen für C-Code; abgedeckt durch den Vergleich beider Modi in allen Laufzeittests von Kapitel 15 |
| R-15.17 | durch R-14.7 abgedeckt: jeder Laufzeittest von Kapitel 15 gilt für beide Modi |
