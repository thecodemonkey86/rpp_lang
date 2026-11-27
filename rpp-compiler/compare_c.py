"""Vergleich des C-Codegenerators (rpp-compiler, Befehl `c`) mit dem Interpreter von rppc.

Aufruf (aus diesem Verzeichnis):  py -X utf8 compare_c.py [-v] [--gcc] [--fr N] [--only-fr] [--exe RPPCC] [--cflags "…"] [PATTERN ...]

Für jeden Laufzeittest der Konformitätstests (conformance/*/runtime) und jede Datei in
tests_c/ wird in beiden Modi (debug, release)
  1. mit `rppc run rpp-compiler -- c [--release] STD PFAD prog.c` C-Code erzeugt,
  2. mit MSVC (Voreinstellung; --gcc: gcc aus C:/Qt/Tools/mingw1310_64) zusammen mit
     runtime/rpp_rt.c übersetzt,
  3. in einem frischen, leeren Arbeitsverzeichnis ausgeführt (Argumente und Eingabe aus dem
     Kopf des Tests, wie `rppc test`) und
  4. mit `rppc run [--release] PFAD` verglichen: Ausgabe, Fehlerausgabe und Exit-Code
     müssen gleich sein.
PATTERN wählt Tests, deren Pfad das Muster enthält. Mit --fr N kommen die handgeschriebenen
FR++-Programme (fr-model/tests) und N Zufallsprogramme aus frpp/gen.py dazu, nach R++
übersetzt wie in fr-model/compare.py; verglichen werden die, die rppc annimmt.
Mit --exe RPPCC erzeugt der übersetzte Compiler den C-Code statt des Interpreters (schneller);
--cflags gibt dem C-Compiler weitere Schalter, etwa "/DRPP_NATCHECK" (bzw. "-DRPP_NATCHECK").
Tests aus Kapiteln mit einem Ordner native/ (Kapitel 15) binden im Releasemodus die C-Bibliotheken
aus native/build, die rppc test bzw. rppcc test vorher gebaut hat; „Erwartet (release): Fehler beim
Bauen“ verlangt, dass das Binden scheitert.
"""

import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
RPPC = ROOT / "rppc" / "target" / "debug" / ("rppc.exe" if sys.platform == "win32" else "rppc")
STD = ROOT / "rppc" / "std"
RUNTIME = HERE / "runtime"
VCVARS = r"C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"
GCC = r"C:\Qt\Tools\mingw1310_64\bin\gcc.exe"


def header_of(path):
    if path.is_dir():
        return path / "rpp.yaml"
    return path


def unquote(s):
    s = s.strip()
    if len(s) >= 2 and s[0] == '"' and s[-1] == '"':
        s = s[1:-1]
    out = []
    i = 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            n = s[i + 1]
            out.append({"n": "\n", "t": "\t"}.get(n, n))
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


def runtime_header(path):
    text = header_of(path).read_text(encoding="utf-8", errors="replace")
    modes = []
    args = []
    stdin = ""
    build_error = set()
    for line in text.splitlines()[:12]:
        l = line.strip().lstrip("/").lstrip("#").strip()
        if l.startswith("Erwartet (debug):"):
            modes.append("debug")
        elif l.startswith("Erwartet (release):"):
            modes.append("release")
            if "Fehler beim Bauen" in l:
                build_error.add("release")
        elif l.startswith("Erwartet:"):
            modes += ["debug", "release"]
        elif l.startswith("Argumente:"):
            args = l[len("Argumente:"):].split()
        elif l.startswith("Eingabe:"):
            stdin = unquote(l[len("Eingabe:"):])
    return sorted(set(modes)), args, stdin, build_error


def native_libs(path):
    """C-Bibliotheken eines Tests aus Kapitel 15: (Ordner, Namen) aus KAPITEL/native/build,
    gebaut von `rppc test` bzw. `rppcc test`. Gebunden wird nur im Releasemodus (R-15.12)."""
    if not path.is_dir():
        return None, []
    build = path.parent.parent / "native" / "build"
    if not (path.parent.parent / "native").is_dir():
        return None, []
    if not build.is_dir():
        raise SystemExit(f"{build} fehlt: zuerst `rppc test` oder `rppcc test` laufen lassen")
    names = sorted(f.stem for f in (path.parent.parent / "native").glob("*.c"))
    return build, names


def run_in_fresh_dir(cmd, stdin):
    work = tempfile.mkdtemp(prefix="rppc-c-")
    try:
        p = subprocess.run(cmd, cwd=work, input=stdin.encode("utf-8"), capture_output=True, timeout=120)
        return p.stdout.decode("utf-8", errors="replace"), p.stderr.decode("utf-8", errors="replace"), p.returncode
    except subprocess.TimeoutExpired:
        return "", "Zeitüberschreitung (120 s)", -1
    finally:
        shutil.rmtree(work, ignore_errors=True)


class Compiler:
    def __init__(self, use_gcc, build, cflags=""):
        self.gcc = use_gcc
        self.cflags = cflags
        self.build = build
        self.rt_obj = None

    def compile(self, c_file, exe, lib_dir=None, libs=()):
        if lib_dir is not None:
            for name in libs:
                shutil.copyfile(lib_dir / (name + ".dll"), exe.parent / (name + ".dll"))
        if self.gcc:
            linked = [f"-L{lib_dir}"] + [f"-l{n}" for n in libs] if lib_dir is not None else []
            cmd = [GCC, "-std=c17", "-O1", "-w", *self.cflags.split(), "-I", str(RUNTIME), str(c_file), str(RUNTIME / "rpp_rt.c"), "-o", str(exe), *linked, "-lm", "-lwinmm", "-Wl,--stack,16777216"]
            p = subprocess.run(cmd, capture_output=True)
            return p.returncode == 0, (p.stdout + p.stderr).decode("utf-8", errors="replace")
        if self.rt_obj is None:
            self.rt_obj = self.build / "rpp_rt.obj"
            bat = self.build / "rt.bat"
            bat.write_text(f'@echo off\ncall "{VCVARS}" >nul\ncl /nologo /std:c17 /O2 /W3 {self.cflags} /c /I "{RUNTIME}" "{RUNTIME / "rpp_rt.c"}" /Fo:"{self.rt_obj}"\n', encoding="cp1252")
            p = subprocess.run(["cmd", "/c", str(bat)], capture_output=True)
            if p.returncode != 0:
                raise SystemExit("Could not compile the C runtime:\n" + (p.stdout + p.stderr).decode("cp1252", errors="replace"))
        obj = exe.with_suffix(".obj")
        linked = "".join(f' "{n}.lib"' for n in libs)
        if lib_dir is not None:
            linked = f' /LIBPATH:"{lib_dir}"' + linked
        bat = self.build / "cc.bat"
        bat.write_text(f'@echo off\ncall "{VCVARS}" >nul\ncl /nologo /std:c17 /O1 /W1 /wd4101 /wd4102 {self.cflags} /I "{RUNTIME}" "{c_file}" "{self.rt_obj}" /Fo:"{obj}" /Fe:"{exe}" /link /STACK:16777216{linked}\n', encoding="cp1252")
        p = subprocess.run(["cmd", "/c", str(bat)], capture_output=True)
        return p.returncode == 0, (p.stdout + p.stderr).decode("cp1252", errors="replace")


def cases(patterns):
    out = []
    for d in sorted((ROOT / "conformance").glob("*/runtime/*")):
        out.append(d)
    extra = HERE / "tests_c"
    if extra.is_dir():
        out += sorted(p for p in extra.iterdir() if p.suffix == ".rpp" or p.is_dir())
    if patterns:
        out = [p for p in out if any(m in str(p).replace("\\", "/") for m in patterns)]
    return out


def fr_programs(tmp, count):
    """Übersetzte FR++-Programme als Dateien, die rppc annimmt."""
    sys.path.insert(0, str(ROOT / "fr-model"))
    from frpp import ParseError, parse
    from frpp.gen import generate
    from frpp.to_rpp import Untranslatable, translate
    progs = []
    for path in sorted((ROOT / "fr-model" / "tests").glob("*/*.fr")):
        try:
            progs.append(parse(path.read_text(encoding="utf-8")))
        except ParseError:
            continue
    for seed in range(count):
        progs.append(generate(seed))
    out = []
    for prog in progs:
        try:
            text, _ = translate(prog)
        except Untranslatable:
            continue
        f = tmp / f"fr_{len(out)}.rpp"
        f.write_bytes(text.encode("utf-8"))
        p = subprocess.run([str(RPPC), "check", str(f)], capture_output=True, input=b"")
        if p.returncode == 0:
            out.append(f)
        else:
            f.unlink()
    return out


def main():
    verbose = "-v" in sys.argv
    use_gcc = "--gcc" in sys.argv
    fr_count = 0
    argv = sys.argv[1:]
    if "--fr" in argv:
        k = argv.index("--fr")
        fr_count = int(argv[k + 1])
        del argv[k:k + 2]
    exe_gen = None
    if "--exe" in argv:
        k = argv.index("--exe")
        exe_gen = str(pathlib.Path(argv[k + 1]).resolve())
        del argv[k:k + 2]
    cflags = ""
    if "--cflags" in argv:
        k = argv.index("--cflags")
        cflags = argv[k + 1]
        del argv[k:k + 2]
    patterns = [a for a in argv if not a.startswith("-")]
    build = pathlib.Path(tempfile.mkdtemp(prefix="rppc-c-build-"))
    comp = Compiler(use_gcc, build, cflags)
    same = diff = 0
    only_fr = "--only-fr" in argv or "--nur-fr" in argv
    todo = [] if only_fr else cases(patterns)
    if fr_count or only_fr:
        frdir = build / "fr"
        frdir.mkdir()
        todo += fr_programs(frdir, fr_count)
    try:
        for path in todo:
            modes, args, stdin, build_error = runtime_header(path)
            lib_dir, libs = native_libs(path)
            if not modes:
                modes = ["debug", "release"]
            try:
                rel = path.relative_to(ROOT).as_posix()
            except ValueError:
                rel = "FR++ " + path.name
            for mode in modes:
                flag = ["--release"] if mode == "release" else []
                c_file = build / "prog.c"
                exe = build / "prog.exe"
                for f in (c_file, exe):
                    if f.exists():
                        f.unlink()
                gen_cmd = [exe_gen] if exe_gen else [str(RPPC), "run", str(HERE), "--"]
                gen = subprocess.run(gen_cmd + ["c"] + flag + [str(STD), str(path.resolve()), str(c_file)], capture_output=True, timeout=3600, input=b"")
                if gen.returncode != 0 or not c_file.exists():
                    diff += 1
                    print(f"MISMATCH {rel} ({mode}): C code generation failed\n{gen.stdout.decode('utf-8', 'replace')}{gen.stderr.decode('utf-8', 'replace')}")
                    continue
                if mode == "release" and lib_dir is not None:
                    ok, msg = comp.compile(c_file, exe, lib_dir, libs)
                else:
                    ok, msg = comp.compile(c_file, exe)
                if mode in build_error:
                    # R-15.12: das Programm darf sich in diesem Modus nicht bauen lassen
                    if ok:
                        diff += 1
                        print(f"MISMATCH {rel} ({mode}): expected a build error")
                    else:
                        same += 1
                        if verbose:
                            print(f"match {rel} ({mode}): build error")
                    continue
                if not ok:
                    diff += 1
                    print(f"MISMATCH {rel} ({mode}): C compilation failed\n{msg[:3000]}")
                    continue
                got = run_in_fresh_dir([str(exe)] + args, stdin)
                want = run_in_fresh_dir([str(RPPC), "run"] + flag + [str(path.resolve()), "--"] + args, stdin)
                if got == want:
                    same += 1
                    if verbose:
                        print(f"match {rel} ({mode})")
                else:
                    diff += 1
                    print(f"MISMATCH {rel} ({mode})")
                    for name, a, b in (("stdout", got[0], want[0]), ("stderr", got[1], want[1]), ("exit code", str(got[2]), str(want[2]))):
                        if a != b:
                            print(f"  {name}:\n    C:      {a[:600]!r}\n    rppc:   {b[:600]!r}")
    finally:
        shutil.rmtree(build, ignore_errors=True)
    print(f"c: {same} matched, {diff} mismatched")
    return 0 if diff == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
