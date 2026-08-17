package com.morrow.host;

import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.ValueLayout;

/**
 * M7 7.1: JNI vs Panama downcall latency, measured in the same process.
 *
 * <p>The JNI function lives in {@code src/test/native/jni_bench.c},
 * compiled by build.sh into {@code libjnibench.so} on
 * {@code java.library.path}. Both paths call the same 2-int add:
 * JNI through the classic JNI machinery, Panama through
 * {@link PanamaBridge} (which wraps {@code add} in the runtime lib).
 */
public class JniBench {

    static {
        System.loadLibrary("jnibench");
    }

    private static native int add(int a, int b);

    public static void main(String[] args) throws Throwable {
        System.out.println("==> M7 7.1: JNI vs Panama downcall latency");

        final int iters = 1_000_000;

        // ── JNI ──
        int sum = 0;
        for (int i = 0; i < 10_000; i++) sum += add(2, 3);
        long start = System.nanoTime();
        for (int i = 0; i < iters; i++) sum += add(2, 3);
        long jniNs = System.nanoTime() - start;

        // ── Panama (same function shape, different library) ──
        PanamaBridge bridge = PanamaBridge.create(PanamaBridge.findNativeLibrary());
        var panamaAdd = bridge.downcall("add",
                FunctionDescriptor.of(ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT, ValueLayout.JAVA_INT));
        for (int i = 0; i < 10_000; i++) sum += (int) panamaAdd.invokeExact(2, 3);
        start = System.nanoTime();
        for (int i = 0; i < iters; i++) sum += (int) panamaAdd.invokeExact(2, 3);
        long panamaNs = System.nanoTime() - start;

        if (sum == 0) System.out.println("unreachable");

        double jni = jniNs / (double) iters;
        double panama = panamaNs / (double) iters;
        System.out.printf("  JNI:    %.1f ns/call%n", jni);
        System.out.printf("  Panama: %.1f ns/call (%.2fx faster)%n", panama, jni / panama);
    }
}
