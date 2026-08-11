package com.ferrum.host;

import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;

/**
 * Performance benchmarks for the Ferrum Panama FFI bridge.
 *
 * <p>Measures:
 * <ol>
 *   <li>Raw downcall latency (add function)</li>
 *   <li>Tick dispatch throughput (N mods, M ticks)</li>
 * </ol>
 *
 * <p>Run with: {@code make test-bridge} then manually:
 * <pre>{@code
 *   java --enable-preview --enable-native-access=ALL-UNNAMED \
 *        -cp bridge-java/out com.ferrum.host.Benchmark
 * }</pre>
 */
public class Benchmark {

    private static final int WARMUP_ITERS = 10_000;
    private static final int MEASURE_ITERS = 1_000_000;

    public static void main(String[] args) throws Throwable {
        PanamaBridge bridge = PanamaBridge.create(
                PanamaBridge.findNativeLibrary());

        System.out.println("=== Ferrum Performance Benchmarks ===\n");

        benchDowncallLatency(bridge);
        benchTickDispatch(bridge);

        System.out.println("=== Done ===");
    }

    // ─── 1. Raw downcall latency ────────────────

    private static void benchDowncallLatency(PanamaBridge bridge) throws Throwable {
        System.out.println("--- Downcall Latency (add) ---");

        MethodHandle add = bridge.downcall("add",
                FunctionDescriptor.of(ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT));

        // Warmup
        int sum = 0;
        for (int i = 0; i < WARMUP_ITERS; i++) {
            sum += (int) add.invokeExact(2, 3);
        }

        // Measure
        long start = System.nanoTime();
        for (int i = 0; i < MEASURE_ITERS; i++) {
            sum += (int) add.invokeExact(2, 3);
        }
        // Prevent JIT from optimizing away the calls
        if (sum == 0) System.out.println("unreachable");
        long elapsed = System.nanoTime() - start;

        double avgNs = (double) elapsed / MEASURE_ITERS;
        System.out.printf("  Iterations: %,d%n", MEASURE_ITERS);
        System.out.printf("  Total time: %.2f ms%n", elapsed / 1_000_000.0);
        System.out.printf("  Avg latency: %.1f ns/call%n", avgNs);

        // Throughput
        double callsPerSec = 1_000_000_000.0 / avgNs;
        System.out.printf("  Throughput:  %,.0f calls/sec%n", callsPerSec);
        System.out.println();
    }

    // ─── 2. Tick dispatch throughput ─────────────

    private static void benchTickDispatch(PanamaBridge bridge) throws Throwable {
        System.out.println("--- Tick Dispatch ---");

        MethodHandle init = bridge.downcall("ferrum_init",
                FunctionDescriptor.of(ValueLayout.JAVA_LONG,
                        ValueLayout.JAVA_INT));
        MethodHandle tick = bridge.downcall("ferrum_tick",
                FunctionDescriptor.ofVoid(
                        ValueLayout.JAVA_LONG,
                        ValueLayout.JAVA_LONG));
        MethodHandle shutdown = bridge.downcall("ferrum_shutdown",
                FunctionDescriptor.of(ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_LONG));

        // Init runtime
        long handle = (long) init.invokeExact(PanamaBridge.ABI_VERSION);
        if (handle == 0) {
            System.err.println("  ERROR: ferrum_init failed");
            return;
        }

        int tickCount = 100_000;

        // Warmup
        for (int i = 0; i < 1000; i++) {
            tick.invokeExact(handle, (long) i);
        }

        // Measure
        long start = System.nanoTime();
        for (int i = 0; i < tickCount; i++) {
            tick.invokeExact(handle, (long) i);
        }
        long elapsed = System.nanoTime() - start;

        double avgUs = (double) elapsed / tickCount / 1000.0;

        System.out.printf("  Ticks:       %,d%n", tickCount);
        System.out.printf("  Total time:  %.2f ms%n", elapsed / 1_000_000.0);
        System.out.printf("  Avg latency: %.2f μs/tick%n", avgUs);
        System.out.printf("  TPS (theoretical): %,.0f ticks/sec%n",
                1_000_000.0 / avgUs);
        System.out.println();

        // Shutdown
        int status = (int) shutdown.invokeExact(handle);
        if (status != 0) System.err.println("  WARN: shutdown returned " + status);
    }
}
