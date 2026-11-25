"""Check the English command-line interface and the native compiler."""

import argparse
import hashlib
import re
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WINDOWS = sys.platform == "win32"
EXE_SUFFIX = ".exe" if WINDOWS else ""
COMPILER = ROOT / "rpp-compiler"
RUNTIME = COMPILER / "runtime"
STD = ROOT / "rppc" / "std"
CONFORMANCE = ROOT / "konformitaet"
ARGUMENTS = CONFORMANCE / "kap12_ausfuehrung/laufzeit/l08_argumente.rpp"
DIVISION = CONFORMANCE / "kap04_typen/laufzeit/l02_division_durch_null.rpp"
NULL_ERROR = CONFORMANCE / "kap06_null/abgelehnt/n08_ersatz_nicht_nullbar.rpp"
JSON_RUNTIME = CONFORMANCE / "kap13_bibliothek/laufzeit"


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
        ("json-tree", "l41_json_baum.rpp", "JSON tree OK\n"),
        ("json-errors", "l42_json_fehler.rpp", "JSON errors OK\n"),
        ("json-depth", "l43_json_tiefe.rpp", "JSON depth OK\n"),
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


if __name__ == "__main__":
    checks = {"cli": check_cli, "native": check_native, "bootstrap": check_bootstrap}
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("check", choices=checks)
    checks[parser.parse_args().check]()
