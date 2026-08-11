#!/bin/bash
# Morrow Java Bridge — Build & Test
# Requires: JDK 21 (Panama FFM is preview), Rust stable
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$SCRIPT_DIR/out"

echo "==> Step 1: Build Rust runtime..."
cd "$PROJECT_ROOT"
cargo build --release
echo "    Rust build OK."

echo ""
echo "==> Step 2: Compile Java sources..."
cd "$SCRIPT_DIR"
rm -rf "$OUT_DIR"
javac --release 21 --enable-preview \
    -d "$OUT_DIR" \
    src/main/java/com/morrow/host/PanamaBridge.java \
    src/test/java/com/morrow/host/M0_AddTest.java \
    src/test/java/com/morrow/host/M1_LifecycleTest.java \
    src/test/java/com/morrow/host/Benchmark.java
echo "    Java compile OK."

echo ""
echo "==> Step 3: M0 Regression Test..."
java --enable-preview --enable-native-access=ALL-UNNAMED \
    -cp "$OUT_DIR" \
    com.morrow.host.M0_AddTest

echo ""
echo "==> Step 4: M1 Lifecycle Test..."
java --enable-preview --enable-native-access=ALL-UNNAMED \
    -cp "$OUT_DIR" \
    com.morrow.host.M1_LifecycleTest

echo ""
echo "==> Step 5: Performance Benchmarks..."
# Use a smaller iteration count for CI, larger for manual runs
BENCH_ITERS=${FERUM_BENCH_ITERS:-100000}
java --enable-preview --enable-native-access=ALL-UNNAMED \
    -cp "$OUT_DIR" \
    com.morrow.host.Benchmark

echo ""
echo "==> All tests passed."
