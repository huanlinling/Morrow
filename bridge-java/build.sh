#!/bin/bash
# Morrow Java Bridge — Build & Test
# Requires: JDK 21 (Panama FFM is preview), Rust stable
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$SCRIPT_DIR/out"

# JAVA_HOME for JNI headers (M7 7.1); derive from javac when unset
JAVA_HOME=${JAVA_HOME:-$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")")}

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
    src/main/java/com/morrow/host/EventBuffer.java \
    src/test/java/com/morrow/host/M0_AddTest.java \
    src/test/java/com/morrow/host/M1_LifecycleTest.java \
    src/test/java/com/morrow/host/EventBufferCodeTest.java \
    src/test/java/com/morrow/host/JniBench.java \
    src/test/java/com/morrow/host/Benchmark.java
echo "    Java compile OK."

echo ""
echo "==> Step 2b: Compile JNI baseline (libjnibench.so)..."
gcc -shared -fPIC \
    -I"$JAVA_HOME/include" -I"$JAVA_HOME/include/linux" \
    -o "$OUT_DIR/libjnibench.so" \
    src/test/native/jni_bench.c
echo "    JNI compile OK."

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
echo "==> Step 5: Event Code Parity Test..."
java --enable-preview --enable-native-access=ALL-UNNAMED \
    -cp "$OUT_DIR" \
    com.morrow.host.EventBufferCodeTest

echo ""
echo "==> Step 6: Performance Benchmarks..."
# Use a smaller iteration count for CI, larger for manual runs
BENCH_ITERS=${FERUM_BENCH_ITERS:-100000}
java --enable-preview --enable-native-access=ALL-UNNAMED \
    -cp "$OUT_DIR" \
    com.morrow.host.Benchmark

echo ""
echo "==> Step 7: JNI vs Panama Comparison (M7 7.1)..."
java --enable-preview --enable-native-access=ALL-UNNAMED \
    -Djava.library.path="$OUT_DIR" \
    -cp "$OUT_DIR" \
    com.morrow.host.JniBench

echo ""
echo "==> Step 8: Agent premain smoke (standalone Mixin bootstrap)..."
AGENT_JAR="build/libs/morrow-host-0.1.0-agent.jar"
if [ ! -f "$AGENT_JAR" ]; then
    ./gradlew --no-daemon -q agentJar
fi
AGENT_OUT=$(java --enable-preview -javaagent:"$AGENT_JAR" -version 2>&1)
if ! echo "$AGENT_OUT" | grep -q "Mixin initialized"; then
    echo "    ❌ FAILED: premain did not initialize Mixin:"; echo "$AGENT_OUT"
    exit 1
fi
if echo "$AGENT_OUT" | grep -q "ServiceNotAvailableError"; then
    echo "    ❌ FAILED: no mixin host service selected:"; echo "$AGENT_OUT"
    exit 1
fi
echo "    ✅ Agent premain boots Mixin standalone (no Fabric)."

echo ""
echo "==> All tests passed."
