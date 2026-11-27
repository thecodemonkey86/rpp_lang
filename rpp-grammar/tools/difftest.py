"""Differenztest: mutierte Programme müssen von ANTLR (RPP.g4) und rppc gleich beurteilt werden.

Aufruf (aus dem Projektverzeichnis r++):  py rpp-grammar/tools/difftest.py [ANZAHL] [SEED]
Vorher rppc bauen:  cd rppc; cargo build
Mutiert werden die positiven Grammatiktests und die Konformitätstests (ohne Direktiven):
ein Token löschen, verdoppeln, vertauschen, ersetzen oder einfügen. Gemeldet wird jeder
Mutant, den nur einer der beiden Parser annimmt, und wie oft die erste Fehlerzeile übereinstimmt.
"""
import pathlib
import random
import re
import shutil
import tempfile
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")
R = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(R / "rpp-grammar" / "tools"))
import run_tests as rt  # noqa: E402

N = int(sys.argv[1]) if len(sys.argv) > 1 else 600
SEED = int(sys.argv[2]) if len(sys.argv) > 2 else 1
random.seed(SEED)
OUT = pathlib.Path(tempfile.mkdtemp(prefix="rpp-mutanten-"))

TOKEN = re.compile(r'"(?:[^"\\\n]|\\.)*"|\'(?:[^\'\\\n]|\\.)\'|//[^\n]*|[A-Za-z_][A-Za-z0-9_]*|\d[\d_]*(?:\.\d[\d_]*)?|\?\.|\?\?|::|\.\.|\+\+|--|&&|\|\||==|!=|<=|>=|\+=|-=|=>|\S')
POOL = ["(", ")", "<", ">", "?", ";", ",", "&", "::", ".", "=", "as", "ref", "own", "x", "Foo", "[", "]",
        "{", "}", "i32", "new", "copy", "..", "?.", "??", "-", "!", "++", "=>", "var", "const", "shared", "1", "null"]

sources = sorted((R / "rpp-grammar" / "tests" / "pos").glob("*.rpp")) + sorted((R / "conformance").rglob("*.rpp"))
sources = [s for s in sources if "#" not in s.read_text(encoding="utf-8")]
texts = [s.read_text(encoding="utf-8") for s in sources]

mutants = []
while len(mutants) < N:
    text = random.choice(texts)
    toks = [m for m in TOKEN.finditer(text) if not m.group().startswith("//")]
    i = random.randrange(len(toks))
    t = toks[i]
    op = random.choice(["del", "dup", "swap", "repl", "ins"])
    if op == "del":
        new = text[:t.start()] + text[t.end():]
    elif op == "dup":
        new = text[:t.end()] + " " + t.group() + text[t.end():]
    elif op == "swap" and i + 1 < len(toks):
        u = toks[i + 1]
        new = text[:t.start()] + u.group() + text[t.end():u.start()] + t.group() + text[u.end():]
    elif op == "repl":
        new = text[:t.start()] + random.choice(POOL) + text[t.end():]
    else:
        new = text[:t.start()] + random.choice(POOL) + " " + text[t.start():]
    if new != text:
        mutants.append(new)

paths = []
antlr = {}
for k, m in enumerate(mutants):
    p = OUT / f"m{k:05d}.rpp"
    p.write_text(m, encoding="utf-8")
    paths.append(p)
    _, c = rt.parse(m)
    antlr[p.name] = c.errors[0] if c.errors else None

rust = {}
exe = R / "rppc" / "target" / "debug" / "rppc.exe"
for b in range(0, len(paths), 80):
    batch = [str(p) for p in paths[b:b + 80]]
    res = subprocess.run([str(exe), "parse", *batch], capture_output=True, text=True, encoding="utf-8")
    for line in res.stdout.splitlines():
        m = re.match(r".*[\\/](m\d+\.rpp)(?::(\d+):\d+: (.*)|: ok)$", line)
        if m:
            rust[m.group(1)] = (int(m.group(2)), m.group(3)) if m.group(2) else None

diff = same_line = other_line = rejected = 0
for name, a in antlr.items():
    r = rust.get(name, "missing")
    if (a is None) != (r is None):
        diff += 1
        if diff <= 15:
            print(f"MISMATCH {name}: ANTLR {'ok' if a is None else a!r} | rppc {'ok' if r is None else r}")
    elif a is not None:
        rejected += 1
        line = int(re.search(r"Line (\d+)", a).group(1))
        if line == r[0]:
            same_line += 1
        else:
            other_line += 1
print(f"{N} mutants (seed {SEED}): both accepted {N - rejected - diff}, both rejected {rejected}, "
      f"{diff} mismatches. First error line matched in {same_line} of {rejected}")
shutil.rmtree(OUT, ignore_errors=True)
