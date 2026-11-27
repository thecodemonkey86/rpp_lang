# R++ Grammatik und Testlauf

[English](README.md)

Ausführbare Grammatik der Sprache R++ mit Testprogrammen.

## Inhalt

| Pfad | Zweck |
|---|---|
| `RPP.g4` | Grammatik im ANTLR4-Format (normativ) |
| `tools/preprocess.py` | Bedingte Kompilierung: löst `#if` / `#else` / `#endif` vor dem Parsen auf |
| `tools/run_tests.py` | Testlauf mit dem ANTLR-Parser |
| `tools/difftest.py` | Differenztest: mutierte Programme müssen von ANTLR und dem Rust-Parser `rppc` gleich beurteilt werden |
| `tests/pos/*.rpp` | Beispielprogramme, die akzeptiert werden müssen |
| `tests/neg/*.rpp` | Programme, die abgelehnt werden müssen (erste Zeile `// Name: …`) |
| `tests/parse_probes/*.rpp` | Programme, deren erste Anweisung als eine bestimmte Regel gelesen werden muss (`// Lesart: …`) |
| `../conformance/**/*.rpp` | Konformitätstests der Spezifikation; hier wird nur die Syntax geprüft, auch bei den semantisch abzulehnenden Programmen |
| `gen/` | Aus `RPP.g4` erzeugter Python-Parser |
| `test-report.txt` | Ausgabe des letzten Testlaufs |
| `examples/hello/rpp.yaml` | Beispiel: Programm mit Abhängigkeit |
| `examples/mathlib/rpp.yaml` | Beispiel: Bibliothek |

## Tests ausführen

Voraussetzung: Python 3 und das Paket `antlr4-python3-runtime` (Version 4.13.2).

```
pip install antlr4-python3-runtime==4.13.2
python3 tools/run_tests.py
```

Dieselben Fälle prüft der Rust-Parser der ersten Implementierung: `cd ../rppc; cargo run -- syntaxtest ..`.

## Parser neu erzeugen

Nach einer Änderung an `RPP.g4` muss `gen/` neu erzeugt werden. Mit Node.js:

```
npm install antlr-ng
npx antlr-ng -Dlanguage=Python3 -o gen RPP.g4
```

Alternativ mit dem offiziellen ANTLR-Werkzeug (Java):

```
java -jar antlr-4.13.2-complete.jar -Dlanguage=Python3 -o gen RPP.g4
```

Der Parser in `gen/` wurde mit antlr-ng 1.0.10 erzeugt. Mit dem Java-Werkzeug ist diese Grammatik nicht getestet.
