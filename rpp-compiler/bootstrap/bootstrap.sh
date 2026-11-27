#!/bin/sh
# Baut rppcc ohne Rust aus dem eingecheckten C-Startpunkt (Meilenstein M8).
#
#   rpp-compiler/bootstrap/bootstrap.sh [--check-seed]
#
# 1. Stufe 0: bootstrap/rppcc.c mit der C-Laufzeit übersetzen.
# 2. Stufe 0 übersetzt den Compiler aus dem Quelltext (C-Code gen1, Stufe 1).
# 3. Stufe 1 übersetzt ihn noch einmal (gen2). Fixpunkt: gen1 und gen2 sind gleich.
# Mit --check-seed muss außerdem gen1 gleich dem Startpunkt sein (er ist aktuell).
# Ergebnis: rpp-compiler/build/rppcc. Der C-Compiler ist $CC, sonst cc.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
compiler=$(dirname "$here")
root=$(dirname "$compiler")
runtime="$compiler/runtime"
build="$compiler/build"
cc=${CC:-cc}
flags="-std=c17 -O2 -D_POSIX_C_SOURCE=200809L -pthread"

check_seed=0
if [ "${1:-}" = "--check-seed" ]; then
    check_seed=1
fi

mkdir -p "$build/stage0" "$build/stage1"

echo "Stage 0: building the seed with $cc"
# shellcheck disable=SC2086
"$cc" $flags -I "$runtime" "$here/rppcc.c" "$runtime/rpp_rt.c" -lm -ldl -o "$build/stage0/rppcc"

echo "Stage 1: the seed compiles rpp-compiler"
"$build/stage0/rppcc" c --release "$root/rppc/std" "$compiler" "$build/gen1.c"
# shellcheck disable=SC2086
"$cc" $flags -I "$runtime" "$build/gen1.c" "$runtime/rpp_rt.c" -lm -ldl -o "$build/stage1/rppcc"

echo "Stage 2: stage 1 compiles rpp-compiler again"
"$build/stage1/rppcc" c --release "$root/rppc/std" "$compiler" "$build/gen2.c"

if ! cmp -s "$build/gen1.c" "$build/gen2.c"; then
    echo "No fixpoint: stage 1 and stage 2 generated different C code" >&2
    exit 1
fi
echo "Fixpoint reached: stage 1 and stage 2 generate the same C code."

if ! cmp -s "$here/rppcc.c" "$build/gen1.c"; then
    if [ "$check_seed" = 1 ]; then
        echo "The seed bootstrap/rppcc.c is out of date; refresh it with" >&2
        echo "  rpp-compiler/build/rppcc c --release rppc/std rpp-compiler rpp-compiler/bootstrap/rppcc.c" >&2
        cp "$build/stage1/rppcc" "$build/rppcc"
        exit 1
    fi
    echo "Note: the seed differs from the current compiler (refresh bootstrap/rppcc.c)."
else
    echo "The seed is up to date."
fi

cp "$build/stage1/rppcc" "$build/rppcc"
echo "Built $build/rppcc"
