# R++ Pakete und Projekte (Kapitel 11)

Stand: 21.11.2026, Entwurf 0.1. Dieses Dokument füllt Kapitel 11 des „R++ Spezifikationsgerüsts“. Es regelt Quelldateien und Pakete, Sichtbarkeit über Paket- und Projektgrenzen, die Projektdatei `rpp.yaml`, Abhängigkeiten und das zentrale Verzeichnis sowie die bedingte Kompilierung. Die lexikalische Form der Direktiven steht im Gerüst (Kapitel 2), die Sichtbarkeit innerhalb von Klassen in Kapitel 7.

Alle Festlegungen sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 13, 13a, 13b und 25). Jede normative Regel trägt eine Nummer `R-11.n`. Die Konformitätstests liegen in `konformitaet/kap11_projekte/`.

## 1. Quelldateien und Pakete

**R-11.1 (Quelldateien).** Eine Quelldatei hat die Endung `.rpp` und ist in UTF-8 kodiert. Sie beginnt mit einer optionalen `package`-Zeile, gefolgt von `using`-Zeilen und Typdeklarationen (Klassen, Mixins, Enums) sowie höchstens einer `main`-Funktion. Headerdateien gibt es nicht.

**R-11.2 (Paket und Verzeichnis).** Eine Datei im Verzeichnis `a/b` relativ zu einem Quellordner beginnt mit `package a.b;`. Eine Datei direkt im Quellordner hat keine `package`-Zeile und gehört zum **Wurzelpaket** des Projekts. Bibliotheken haben kein Wurzelpaket (R-11.18).

**R-11.3 (Paketnamen).** Paketnamen bestehen aus Bezeichnern, getrennt durch `.`; kein Teil darf ein Schlüsselwort sein (`package my.package;` ist ein Fehler).

**R-11.4 (Typen in Dateien).** Eine Datei darf beliebig viele Typen enthalten, und ihr Name ist frei; nur das Verzeichnis muss zum Paket passen. Innerhalb eines Pakets sind Typnamen eindeutig, auch über Dateien hinweg.

**R-11.5 (Zyklen).** Pakete eines Projekts dürfen sich gegenseitig verwenden, auch im Kreis. Der Compiler liest alle Dateien eines Projekts, bevor er Namen auflöst; die Reihenfolge der Dateien spielt keine Rolle.

## 2. Namen und using

**R-11.6 (Vollständige Namen).** Jeder Typ hat einen vollständigen Namen aus Paket und Typname (`rpp.collections.Map`). Ein vollständiger Name darf überall stehen, wo ein Typ erwartet wird.

**R-11.7 (using).** `using a.b.C;` macht den Typ `C` aus dem Paket `a.b` in der Datei unter dem Namen `C` verfügbar. `using X = a.b.C;` macht ihn unter dem Namen `X` verfügbar. `using` nennt immer einen Typ, nie ein ganzes Paket; Platzhalter wie `using a.b.*;` gibt es nicht. Ein `using` auf einen nicht vorhandenen oder nicht sichtbaren Typ ist ein Fehler. `using` gilt nur in der Datei, in der es steht.

**R-11.8 (Namensauflösung).** Ein einfacher Name wird in dieser Reihenfolge gesucht; der erste Treffer gilt:
1. lokale Variablen und Parameter,
2. Member der Klasse und ihrer Basisklassen, einschließlich importierter Mixin-Methoden,
3. Typ-Parameter,
4. Typen des eigenen Pakets,
5. Namen aus `using`-Zeilen der Datei,
6. Typen aus `rpp.core` (Kapitel 13, V-13.1).

Passen auf derselben Stufe 5 zwei Namen (zwei `using`-Zeilen mit demselben Namen), ist das ein Fehler, sobald der Name verwendet wird. Ein `using`-Alias, der einen Typ des eigenen Pakets verdeckt, ist ein Fehler.

## 3. Sichtbarkeit über Projektgrenzen

**R-11.9 (Innerhalb eines Projekts).** Jeder Typ ist im ganzen eigenen Projekt sichtbar, in allen Paketen. Ein „paketprivat“ gibt es nicht; innerhalb von Klassen gelten die Sichtbarkeits-Labels (Kapitel 7, R-7.2).

**R-11.10 (export).** `export` vor `class`, `mixin` oder `enum` macht einen Typ in anderen Projekten nutzbar. Dort sichtbar sind seine `public`-Member und, für Unterklassen, seine `protected`-Member. Member werden nicht einzeln exportiert.

**R-11.11 (Vollständige Schnittstelle).** Die Signaturen der in anderen Projekten sichtbaren Member eines exportierten Typs (Feldtypen, Parameter- und Rückgabetypen, Basisklasse, importierte Mixins, Constraints) verwenden nur exportierte Typen, Typen aus `rpp.*` und eingebaute Typen; sonst ist das ein Fehler.

**R-11.12 (Nur direkte Abhängigkeiten).** Ein Projekt kann nur Typen aus Bibliotheken verwenden, die in seiner `rpp.yaml` stehen. Typen aus den Abhängigkeiten einer Bibliothek sind nur nutzbar, wenn das Projekt diese selbst einträgt. Die Bibliotheken unter `rpp` sind ohne Eintrag verfügbar (V-13.1).

## 4. Die Projektdatei rpp.yaml

**R-11.13 (Ort und Einträge).** Jedes Projekt hat im Projektverzeichnis eine Datei `rpp.yaml` mit diesen Einträgen:

| Eintrag | Pflicht | Bedeutung |
|---|---|---|
| `name` | ja | Name des Projekts; bei Bibliotheken im Verzeichnis `besitzer.name` (R-11.17) |
| `version` | ja | Version nach Semantic Versioning (`1.2.0`) |
| `type` | ja | `executable` oder `library` |
| `main` | bei `executable` | Datei mit `main`, relativ zum Projektverzeichnis |
| `sources` | ja | Liste der Quellordner |
| `dependencies` | nein | Liste von Abhängigkeiten (R-11.15) |
| `registry` | nein | Adresse eines eigenen oder gespiegelten Verzeichnisses statt des zentralen |
| `symbols` | nein | eigene Symbole für die bedingte Kompilierung, je Modus: `debug: [ … ]`, `release: [ … ]` |

Unbekannte Einträge sind ein Fehler. Die Datei verwendet nur die YAML-Formen, die für diese Einträge nötig sind (Zuordnungen, Listen, einfache Werte und Kommentare).

**R-11.14 (main).** Ein Projekt vom Typ `executable` hat genau eine Funktion `i32 main(string[] args)`, und zwar in der Datei, die `main` nennt. Eine Bibliothek hat keine `main`.

**R-11.15 (Abhängigkeiten).** Eine Abhängigkeit ist entweder lokal oder aus dem Verzeichnis:

```yaml
dependencies:
  - name: mathlib            # lokal
    path: ../mathlib
  - name: acme.json          # aus dem Verzeichnis, Mindestversion
    version: "1.4"
```

**R-11.16 (Zyklenfreiheit).** Der Graph der Abhängigkeiten zwischen Projekten ist zyklenfrei; ein Zyklus ist ein Fehler.

**R-11.17 (Bibliotheksnamen).** Eine Bibliothek im Verzeichnis heißt `besitzer.name`; der Besitzer `rpp` ist reserviert. Eine lokale Bibliothek darf einen beliebigen Namen tragen, der kein Schlüsselwort enthält; vor dem Veröffentlichen braucht sie einen Namen mit Besitzer.

**R-11.18 (Pakete einer Bibliothek).** Alle Pakete einer Bibliothek beginnen mit ihrem Namen (`acme.json.parser`). Zwei Bibliotheken können deshalb nie dasselbe Paket enthalten.

## 5. Versionen und zentrales Verzeichnis

**R-11.19 (Auflösung).** Für jede Bibliothek wird die höchste Mindestversion gewählt, die irgendwo im Abhängigkeitsgraph verlangt wird (Minimal Version Selection). Jede Bibliothek gibt es pro Programm nur in einer Version. Verlangen Abhängigkeiten verschiedene Hauptversionen derselben Bibliothek, ist das ein Fehler.

**R-11.20 (Sperrdatei).** `rpp.lock` im Projektverzeichnis hält für jede Bibliothek die gewählte Version und eine Prüfsumme ihres Inhalts fest. Stimmt die Prüfsumme beim Laden nicht, ist das ein Fehler.

**R-11.21 (Veröffentlichen).** Veröffentlicht wird Quelltext. Das Verzeichnis lehnt eine Version ab, wenn sich die exportierte Schnittstelle (R-11.10) gegenüber der Vorversion inkompatibel ändert, ohne dass die Hauptversion steigt, oder wenn neue Teile ohne neue Nebenversion hinzukommen. Veröffentlichte Versionen sind unveränderlich und werden höchstens als zurückgezogen markiert.

**R-11.22 (Zwischenrepräsentation).** Aus dem Quelltext einer Bibliothek entsteht die Zwischenrepräsentation je Modus und Betriebssystem. Das Verzeichnis baut die sechs Standardkombinationen vor; bei eigenen Symbolen oder fehlender Variante baut der Compiler selbst. Generische Typen einer Bibliothek werden aus der Zwischenrepräsentation für neue Typargumente erzeugt.

## 6. Bedingte Kompilierung

**R-11.23 (Direktiven).** Eine Zeile, deren erstes Zeichen nach Leerraum `#` ist, ist eine Direktive: `#if B`, `#elif B`, `#else`, `#endif`. Bedingungen `B` bestehen aus Symbolen, `!`, `&&`, `||` und Klammern; `&&` bindet stärker als `||`. Jede andere Direktive, auch `#define`, ist ein Fehler. Nach einer Direktive darf ein Zeilenkommentar stehen (`#if debug // nur beim Testen`).

**R-11.24 (Symbole).** Gesetzt ist genau eines von `debug` und `release`, genau eines von `windows`, `linux` und `macos` sowie die eigenen Symbole aus `symbols` für den gewählten Modus. Symbole im Code zu definieren ist nicht möglich. Ein nicht gesetztes Symbol ist falsch.

**R-11.25 (Auswertung).** Direktiven werden vor dem Parsen ausgewertet. Die Zeilen inaktiver Zweige werden übersprungen und weder syntaktisch noch semantisch geprüft. `#if` und `#endif` müssen sich innerhalb einer Datei paaren.

## 7. Beispiel

```
hello/
├── rpp.yaml
├── rpp.lock
└── src/
    ├── main.rpp              // Wurzelpaket, enthält main
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

## 8. Konformitätstests

Die Testprogramme liegen in `konformitaet/kap11_projekte/`. Weil Kapitel 11 vor allem Projekte und Dateien betrifft, bestehen viele Tests aus mehreren Dateien; bisher gibt es nur Einzeldatei-Tests zur Namensauflösung und zu `using`.

| Regeln | Akzeptiert | Abgelehnt |
|---|---|---|
| R-11.4, R-11.7, R-11.8 | `a01_using.rpp` | `n01_using_konflikt.rpp`, `n02_alias_verdeckt.rpp` |
| R-11.11 | `a02_export.rpp` | `n03_export_mit_internem_typ.rpp` |
| R-11.14 | – | `n04_zwei_main.rpp` |
