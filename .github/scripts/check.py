"""Check the English command-line interface, the native compiler and the registry."""

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WINDOWS = sys.platform == "win32"
EXE_SUFFIX = ".exe" if WINDOWS else ""
COMPILER = ROOT / "rpp-compiler"
RUNTIME = COMPILER / "runtime"
STD = ROOT / "rppc" / "std"
CONFORMANCE = ROOT / "conformance"
ARGUMENTS = CONFORMANCE / "ch12_execution/runtime/l08_arguments.rpp"
CONSOLE = CONFORMANCE / "ch13_standard_library/runtime/l04_console.rpp"
DIVISION = CONFORMANCE / "ch04_types/runtime/l02_division_by_zero.rpp"
NULL_ERROR = CONFORMANCE / "ch06_null_safety/rejected/n08_fallback_not_nullable.rpp"
JSON_RUNTIME = CONFORMANCE / "ch13_standard_library/runtime"


def rppc(profile):
    return ROOT / "rppc" / "target" / profile / f"rppc{EXE_SUFFIX}"


def run(command, *, cwd=ROOT):
    result = subprocess.run(command, cwd=cwd, input="", encoding="utf-8",
                            capture_output=True, timeout=120)
    return result.returncode, result.stdout, result.stderr


def expect_output(command, exit_code, message, *, stderr=False):
    code, out, err = run(command)
    stream, other = (err, out) if stderr else (out, err)
    if code != exit_code or message not in stream or other:
        raise RuntimeError(f"{command} returned {(code, out, err)!r}, "
                           f"expected exit {exit_code} and {message!r} on "
                           f"{'stderr' if stderr else 'stdout'}")
    return code, out, err


def check_cli():
    rust = [rppc("debug")]
    compiler = [*rust, "run", COMPILER, "--"]
    expect_output([*rust, "--help"], 0, "Usage:\n  rppc check PATH")
    expect_output([*compiler, "--help"], 0, "Usage:\n  rppcc tokens FILE")
    expect_output([*compiler, "diagnose", "--help"], 0, "Show this help")
    expect_output([*compiler, "tokens"], 2, "missing argument FILE", stderr=True)
    expect_output([*compiler, "project", "--unknown"], 2,
                  "unknown option --unknown", stderr=True)
    expect_output([*compiler, "tokens", "file.rpp", "extra"], 2,
                  "unexpected argument extra", stderr=True)

    message = "[R-6.18] The left operand of ?? must be nullable, got string"
    expect_output([*rust, "check", NULL_ERROR], 1, f"Error {message}")
    args = ["diagnose", STD, NULL_ERROR]
    rust_diagnostic = expect_output([*rust, *args], 0, message)
    compiler_diagnostic = expect_output([*compiler, *args], 0, message)
    if rust_diagnostic != compiler_diagnostic:
        raise RuntimeError(f"Compiler diagnostics differ: {rust_diagnostic!r} "
                           f"and {compiler_diagnostic!r}")

    code, out, err = run([*compiler, "diagnose", "--timings", STD, ARGUMENTS])
    timings = r"Project loading: \d+ ms\nDeclaration checking: \d+ ms\nBody checking: \d+ ms\n"
    if code != 0 or out or not re.fullmatch(timings, err):
        raise RuntimeError(f"Unexpected timing output: {(code, out, err)!r}")
    print("English help, argument errors, compiler diagnostics, and timings passed.")
    check_live_stdio(rust[0])


def check_live_stdio(rust):
    pipes = {"stdin": subprocess.PIPE, "stdout": subprocess.PIPE, "stderr": subprocess.PIPE}
    with subprocess.Popen([rust, "run", ARGUMENTS], cwd=ROOT, **pipes) as process:
        try:
            code = process.wait(timeout=15)
            result = (code, process.stdout.read(), process.stderr.read())
            if result != (0, b"0\n", b""):
                raise RuntimeError(f"Unexpected result with stdin open: {result!r}")
        finally:
            if process.poll() is None:
                process.kill()

    with ThreadPoolExecutor(1) as reader, subprocess.Popen([rust, "run", CONSOLE], cwd=ROOT, **pipes) as process:
        try:
            process.stdin.write(b"a\r\n\n")
            process.stdin.flush()
            # The program writes a response without a newline and then tries to read again.
            expected = b"[a][]"
            output = reader.submit(process.stdout.read, len(expected)).result(timeout=15)
            if output != expected:
                raise RuntimeError(f"Unexpected output before EOF: {output!r}")

            process.stdin.write("Grüße".encode("utf-8"))
            process.stdin.close()
            code = process.wait(timeout=15)
            result = (code, process.stdout.read(), process.stderr.read())
            if result != (0, "[Grüße]\n".encode("utf-8"), b"fertig\n"):
                raise RuntimeError(f"Unexpected result after EOF: {result!r}")
        finally:
            if process.poll() is None:
                process.kill()
    print("Live console input, output before EOF, and redirected input passed.")


def generate_c(driver, source, output, *, release=False, timeout=120):
    mode = ["--release"] if release else []
    subprocess.run([*driver, "c", *mode, STD, source, output], cwd=ROOT,
                   input=b"", check=True, timeout=timeout)


def build_native(source, executable):
    if WINDOWS:
        command = [
            "cl.exe", "/nologo", "/std:c17", "/utf-8", "/O1", "/I", RUNTIME,
            source, RUNTIME / "rpp_rt.c", f"/Fe:{executable}",
            "/link", "/STACK:16777216",
        ]
    else:
        command = [
            "gcc", "-std=c17", "-D_POSIX_C_SOURCE=200809L", "-O1", "-pthread",
            "-I", RUNTIME, source, RUNTIME / "rpp_rt.c", "-lm", "-o", executable,
        ]
    subprocess.run(command, cwd=source.parent, check=True, timeout=1200)


def check_native():
    rust = rppc("debug")
    compiler = [rust, "run", COMPILER, "--"]
    cases = [
        ("arguments", ARGUMENTS, True, ["Ada", "two words"], 0, "2\nAda\ntwo words\n", ""),
        ("division", DIVISION, False, [], 2, "vorher\n", "DivisionError: Division by zero"),
    ]
    for name, source, expected in (
        ("json-tree", "l41_json_tree.rpp", "JSON tree OK\n"),
        ("json-errors", "l42_json_errors.rpp", "JSON errors OK\n"),
        ("json-depth", "l43_json_depth.rpp", "JSON depth OK\n"),
    ):
        for mode in ("debug", "release"):
            cases.append((f"{name}-{mode}", JSON_RUNTIME / source, mode == "release", [], 0, expected, ""))
    for name, source, release, args, exit_code, stdout, stderr in cases:
        print(f"Building native {name} test", flush=True)
        with tempfile.TemporaryDirectory(prefix="rpp-native-") as directory:
            work = Path(directory)
            generated = work / f"{name}.c"
            executable = work / f"{name}{EXE_SUFFIX}"
            generate_c(compiler, source, generated, release=release)
            build_native(generated, executable)

            mode = ["--release"] if release else []
            expected = run([rust, "run", *mode, source, "--", *args], cwd=work)
            actual = run([executable, *args], cwd=work)
            if actual != expected:
                raise RuntimeError(f"{name}: native {actual!r} differs from interpreter {expected!r}")
            code, out, err = actual
            if code != exit_code or out != stdout or not err.startswith(stderr) or (stderr == "" and err):
                raise RuntimeError(f"{name}: unexpected native result {actual!r}")
            print(f"Native {name} test passed.")


def check_bootstrap():
    driver = [rppc("release"), "run", COMPILER, "--"]
    with tempfile.TemporaryDirectory(prefix="rpp-bootstrap-") as directory:
        work = Path(directory)
        outputs = []
        for stage in (1, 2, 3):
            print(f"Generating compiler stage {stage}", flush=True)
            generated = work / f"stage{stage}.c"
            generate_c(driver, COMPILER, generated, release=True, timeout=5400)
            outputs.append(generated)
            if stage < 3:
                print(f"Building compiler stage {stage}", flush=True)
                executable = work / f"stage{stage}{EXE_SUFFIX}"
                build_native(generated, executable)
                driver = [executable]

        second, third = (path.read_bytes() for path in outputs[1:])
        if second != third:
            raise RuntimeError("The second and third compiler stages generated different C code.")
        print(f"Self-hosting fixpoint verified at SHA-256 {hashlib.sha256(third).hexdigest()}.")


GEO_V1 = """package acme.geo;

export class Point {
public:
    i32 x;
    constructor(i32 x) {
        this.x = x;
    }
    i32 twice() {
        return x * 2;
    }
private:
    i32 secret() {
        return 7;
    }
}
"""

APP_MAIN = """using acme.geo.Point;

i32 main(string[] args) {
    Point p = new Point(21);
    print(p.twice().toString());
    return 0;
}
"""


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def library_yaml(version, extra=""):
    return f"name: acme.geo\nversion: {version}\ntype: library\nsources:\n  - src\n{extra}"


def check_registry(compiler):
    """publish, withdraw and lock of rppcc with a registry in the file system (M9)."""
    rppcc = [str(compiler)]
    env_backup = os.environ.pop("RPP_REGISTRY", None)
    try:
        with tempfile.TemporaryDirectory(prefix="rpp-registry-") as directory:
            work = Path(directory)
            registry = work / "registry"
            registry.mkdir()
            lib = work / "geo"
            source = lib / "src/acme/geo/Point.rpp"

            def publish(version, text, code, message):
                write(lib / "rpp.yaml", library_yaml(version))
                write(source, text)
                expect_any([*rppcc, "publish", "--registry", registry, lib], code, message)

            publish("1.0.0", GEO_V1, 0, "Published acme.geo 1.0.0")
            if not (registry / "acme.geo/1.0.0/src/acme/geo/Point.rpp").is_file():
                raise RuntimeError("publish did not copy the source file")
            publish("1.0.0", GEO_V1, 1, "already published")
            added = GEO_V1.replace("private:", "    i32 half() {\n        return x / 2;\n    }\nprivate:")
            publish("1.0.1", added, 1, "needs a new minor version (1.1.0")
            publish("1.1.0", added, 0, "Published acme.geo 1.1.0")
            removed = added.replace("    i32 twice() {\n        return x * 2;\n    }\n", "")
            publish("1.2.0", removed, 1, "needs a new major version (2.0.0")
            publish("2.0.0", removed, 0, "Published acme.geo 2.0.0")
            publish("2.0.1", removed.replace("return 7;", "return 8;"), 0, "Published acme.geo 2.0.1")

            write(lib / "rpp.yaml", library_yaml("2.1.0", "dependencies:\n  - name: other\n    path: ../other\n"))
            expect_any([*rppcc, "publish", "--registry", registry, lib], 1, "cannot have path dependencies")
            write(lib / "rpp.yaml", library_yaml("2.1.0").replace("acme.geo", "geo"))
            expect_any([*rppcc, "publish", "--registry", registry, lib], 1, "")

            app = work / "app"
            write(app / "rpp.yaml", "name: app\nversion: 0.1.0\ntype: executable\nmain: src/main.rpp\n"
                  "sources:\n  - src\nregistry: ../registry\ndependencies:\n  - name: acme.geo\n    version: \"1.0\"\n")
            write(app / "src/main.rpp", APP_MAIN)
            expect_any([*rppcc, "lock", app], 0, "rpp.lock: 1 libraries")
            first = (app / "rpp.lock").read_bytes()
            if b"version: 1.0.0" not in first:
                raise RuntimeError(f"lock chose the wrong version:\n{first.decode()}")
            expect_any([*rppcc, "lock", app], 0, "rpp.lock: 1 libraries")
            if (app / "rpp.lock").read_bytes() != first:
                raise RuntimeError("rpp.lock is not reproducible")
            rust = rppc("release")
            if rust.exists():
                code, out, err = run([rust, "lock", "--stdout", app])
                if code != 0 or out.encode("utf-8") != first:
                    raise RuntimeError(f"rppc lock differs from rppcc lock: {(code, out, err)!r}")
            expect_any([*rppcc, "run", app], 0, "42")

            expect_any([*rppcc, "withdraw", "--registry", registry, "acme.geo", "1.0.0"], 0, "Withdrew acme.geo 1.0.0")
            if not (registry / "acme.geo/1.0.0.withdrawn").is_file():
                raise RuntimeError("withdraw did not mark the version")
            expect_any([*rppcc, "run", app], 0, "42")
            code, out, _ = run([*rppcc, "lock", "--stdout", app])
            if code != 0 or "version: 1.1.0" not in out:
                raise RuntimeError(f"lock should skip the withdrawn version: {(code, out)!r}")

            published = registry / "acme.geo/1.0.0/src/acme/geo/Point.rpp"
            published.write_bytes(published.read_bytes().replace(b"x * 2", b"x * 3"))
            expect_any([*rppcc, "build", "-o", work / f"app{EXE_SUFFIX}", app], 1, "the library has changed")
    finally:
        if env_backup is not None:
            os.environ["RPP_REGISTRY"] = env_backup
    print("Registry check passed: publish, withdraw, lock and checksums.")


def expect_any(command, exit_code, message):
    """Exit code and a message on stdout or stderr."""
    code, out, err = run([str(c) for c in command])
    if code != exit_code or message not in out + err:
        raise RuntimeError(f"{command} returned {(code, out, err)!r}, expected exit {exit_code} and {message!r}")


if __name__ == "__main__":
    checks = {"cli": check_cli, "native": check_native, "bootstrap": check_bootstrap, "registry": None}
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("check", choices=checks)
    parser.add_argument("--compiler", default=str(COMPILER / "build" / f"rppcc{EXE_SUFFIX}"),
                        help="rppcc for the registry check")
    options = parser.parse_args()
    if options.check == "registry":
        check_registry(Path(options.compiler))
    else:
        checks[options.check]()
