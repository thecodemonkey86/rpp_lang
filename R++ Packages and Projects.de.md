# R++ Pakete und Projekte (Kapitel 11)

[English](R%2B%2B%20Packages%20and%20Projects.md)

Stand: 21.11.2026, Entwurf 0.1. Dieses Dokument füllt Kapitel 11 des „R++ Spezifikationsgerüsts“. Es regelt Quelldateien und Pakete, Sichtbarkeit über Paket- und Projektgrenzen, die Projektdatei `rpp.yaml`, Abhängigkeiten und das zentrale Verzeichnis sowie die bedingte Kompilierung. Die lexikalische Form der Direktiven steht im Gerüst (Kapitel 2), die Sichtbarkeit innerhalb von Klassen in Kapitel 7.

Alle Festlegungen sind entschieden (siehe „R++ Designentscheidungen“, Abschnitte 13, 13a, 13b und 25). Jede normative Regel trägt eine Nummer `R-11.n`. Die Konformitätstests liegen in `conformance/ch11_projects/`.

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
| `registry` | nein | Adresse eines eigenen oder gespiegelten Verzeichnisses statt des zentralen; bis es den Dienst gibt, ein Ordner im Dateisystem relativ zum Projektverzeichnis (R-11.26) |
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

## 5a. Das Verzeichnis im Dateisystem

Bis es den Dienst für das zentrale Verzeichnis gibt, ist ein Verzeichnis ein Ordner im Dateisystem (Implementierungsplan, Meilenstein M9). R-11.19 bis R-11.21 gelten dafür unverändert; R-11.26 bis R-11.31 legen fest, was dort offen bleibt.

**R-11.26 (Ort und Aufbau).** Welches Verzeichnis gilt, bestimmt das Programm, also das Projekt, das übersetzt wird: der Eintrag `registry` in seiner `rpp.yaml` (ein Pfad relativ zum Projektverzeichnis), sonst die Umgebungsvariable `RPP_REGISTRY`. Es gilt für alle Abhängigkeiten mit `version`, auch die von Bibliotheken; `registry` in einer Bibliothek bleibt ohne Wirkung. Die Version `v` der Bibliothek `besitzer.name` liegt im Ordner `besitzer.name/v` als Projekt mit `rpp.yaml`; `name` und `version` darin müssen dazu passen. Eine Datei `besitzer.name/v.withdrawn` markiert die Version als zurückgezogen. Fehlen das Verzeichnis, die Bibliothek oder die Version, ist das ein Fehler an der Abhängigkeit.

**R-11.27 (Versionen und Namen).** Eine Version hat die Form `MAJOR.MINOR.PATCH` aus Dezimalzahlen ohne führende Nullen mit höchstens neun Ziffern; das gilt für `version` in jeder `rpp.yaml`. Eine Mindestversion in `dependencies` darf hintere Teile weglassen: `"1.4"` steht für `1.4.0`, `"1"` für `1.0.0`. Versionen werden Teil für Teil als Zahlen verglichen; die Hauptversion ist der erste Teil. Eine Abhängigkeit mit `version` nennt eine Bibliothek `besitzer.name` aus genau zwei ASCII-Bezeichnern, die keine Schlüsselwörter sind (R-11.17). Eine Bibliothek im Verzeichnis hat keine Abhängigkeiten mit `path`.

**R-11.28 (Auswahl).** Zu einer Anforderung (Bibliothek, Mindestversion `m`) gehört die kleinste nicht zurückgezogene Version `≥ m` mit derselben Hauptversion wie `m`. Ausgehend von den Anforderungen des Programms und seiner lokalen Abhängigkeiten werden die Anforderungen jeder so erreichten Version verfolgt, in der Reihenfolge der Einträge (Breitensuche); auch eine Version, die eine höhere überholt, trägt ihre Anforderungen bei. Gewählt wird je Bibliothek die höchste erreichte Version (R-11.19). Nennt eine Anforderung eine andere Hauptversion als die erste Anforderung derselben Bibliothek, ist sie der Fehler. Eine Bibliothek kann nicht zugleich lokal und aus dem Verzeichnis eingebunden sein, und Zyklen sind auch im Verzeichnis ein Fehler (R-11.16). Sichtbar sind die Typen der gewählten Bibliotheken nur über direkte Einträge (R-11.12).

**R-11.29 (Prüfsumme).** Die Prüfsumme einer Bibliothek wird über ihren Ordner gebildet. Für `rpp.yaml` und jede Datei mit der Endung `.rpp` darunter entsteht eine Zeile: die SHA-256-Prüfsumme des Inhalts als 64 Hexziffern in Kleinbuchstaben, zwei Leerzeichen, der Pfad relativ zum Ordner mit `/` und ein LF. Dabei zählt jedes CR LF im Inhalt als LF; so hängt die Prüfsumme nicht von der Zeilenumwandlung beim Auschecken ab. Die Zeilen werden nach dem Pfad sortiert (Byte-Reihenfolge von UTF-8) und aneinandergehängt. Die Prüfsumme ist `sha256:` gefolgt von der SHA-256-Prüfsumme dieses Texts. Andere Dateien zählen nicht, weil der Compiler sie nicht liest.

**R-11.30 (rpp.lock).** Die Sperrdatei liegt neben der `rpp.yaml` des Programms:

```yaml
libraries:
  - name: acme.json
    version: 1.4.0
    checksum: sha256:…
```

Der Befehl `lock` (`rppc lock PROJEKT`, `rppcc lock PROJEKT`) wählt nach R-11.28, ohne eine vorhandene Sperrdatei zu lesen, und schreibt sie neu, die Einträge nach Namen sortiert; dieselbe Eingabe ergibt dieselbe Datei. Gibt es beim Übersetzen eine `rpp.lock`, gehört zu jeder Anforderung die gesperrte Version, auch wenn sie inzwischen zurückgezogen ist (R-11.21); sie muss die Mindestversion erfüllen und dieselbe Hauptversion haben. Fehler sind dann: eine Anforderung ohne Eintrag oder mit einem Eintrag, der sie nicht erfüllt; ein Eintrag, den keine Anforderung nennt; eine Prüfsumme, die nicht zum Ordner passt (R-11.20); eine Sperrdatei, die nicht diese Form hat. Ohne `rpp.lock` wird nach R-11.28 gewählt. Sperrdateien von Bibliotheken werden nicht gelesen.

**R-11.31 (Veröffentlichen und Zurückziehen).** `rppcc publish [--registry VERZEICHNIS] PROJEKT` veröffentlicht eine Bibliothek in das angegebene Verzeichnis, sonst in das aus `registry` des Projekts, sonst in das aus `RPP_REGISTRY`. Verlangt sind der Typ `library`, ein Name `besitzer.name`, eine gültige Version, keine Abhängigkeit mit `path` und eine fehlerfreie Prüfung; eine schon vorhandene Version ist ein Fehler. Die exportierte Schnittstelle (exportierte Typen mit ihren `public`- und `protected`-Teilen, R-11.10) wird mit der nächstniedrigeren veröffentlichten Version verglichen. Entfallen oder ändern sich Teile, muss die Hauptversion steigen; kommen nur Teile hinzu, mindestens die Nebenversion (R-11.21). Kopiert werden `rpp.yaml` und die Quelldateien, die der Compiler lädt. `rppcc withdraw [--registry VERZEICHNIS] NAME VERSION` legt die Markierung aus R-11.26 an.

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

Die Testprogramme liegen in `conformance/ch11_projects/`. Weil Kapitel 11 vor allem Projekte und Dateien betrifft, bestehen viele Tests aus mehreren Dateien. Die Tests zum Verzeichnis (Abschnitt 5a) bringen ihr Verzeichnis als Ordner `registry/` im Testverzeichnis mit.

| Regeln | Akzeptiert | Abgelehnt |
|---|---|---|
| R-11.4, R-11.7, R-11.8 | `a01_using.rpp` | `n01_using_conflict.rpp`, `n02_alias_shadowed.rpp` |
| R-11.11 | `a02_export.rpp` | `n03_export_with_internal_type.rpp` |
| R-11.14 | – | `n04_two_main_functions.rpp` |
| R-11.19, R-11.20, R-11.26, R-11.28 | `l02_registry` (Laufzeit: die höchste Mindestversion gewinnt, mit `rpp.lock`) | `n14_major_versions`, `n16_no_matching_version` |
| R-11.21, R-11.28, R-11.30 | `l03_withdrawn`, `l04_locked_withdrawn` (Laufzeit) | `n18_stale_lock_file`, `n19_unneeded_entry` |
| R-11.20, R-11.29 | – | `n17_checksum` |
| R-11.26 | – | `n15_unknown_library` |
| R-11.27 | – | `n20_incomplete_version`, `n21_path_in_registry` |
| R-11.16 und R-11.12 im Verzeichnis | – | `n22_cycle_in_registry`, `n23_direct_dependencies_only` |

`publish` und `withdraw` (R-11.31) sind Befehle, keine Programme; sie prüft `.github/scripts/check.py registry`: Veröffentlichen mit den Regeln für Haupt- und Nebenversion, unveränderliche Versionen, Zurückziehen, eine reproduzierbare `rpp.lock`, mit der eine zurückgezogene Version weiter baut, und das Erkennen einer veränderten Bibliothek.
