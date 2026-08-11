package com.morrow.host;

import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;

/**
 * Milestone 1: Minimal Runtime lifecycle test.
 *
 * <p>Runs init → shutdown 10 times, verifying no handles leak.
 * Also exercises negative cases: wrong ABI version, unknown handle,
 * and double shutdown.
 */
public class M1_LifecycleTest {

    private static final int CYCLES = 10;

    public static void main(String[] args) throws Throwable {
        System.out.println("==> M1 Lifecycle Test: init → shutdown × " + CYCLES);

        PanamaBridge bridge = PanamaBridge.create(
                PanamaBridge.findNativeLibrary());

        MethodHandle morrow_init = bridge.downcall("morrow_init",
                FunctionDescriptor.of(ValueLayout.JAVA_LONG,
                        ValueLayout.JAVA_INT));

        MethodHandle morrow_shutdown = bridge.downcall("morrow_shutdown",
                FunctionDescriptor.of(ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_LONG));

        MethodHandle morrow_handle_count = bridge.downcall("morrow_handle_count",
                FunctionDescriptor.of(ValueLayout.JAVA_LONG));

        // ── Positive: init → shutdown × CYCLES ──

        for (int i = 1; i <= CYCLES; i++) {
            long handle = (long) morrow_init.invokeExact(PanamaBridge.ABI_VERSION);
            check(handle != 0, "Iteration " + i + ": morrow_init returned 0");

            int status = (int) morrow_shutdown.invokeExact(handle);
            check(status == PanamaBridge.RESULT_OK,
                    "Iteration " + i + ": shutdown returned " + status);

            long live = (long) morrow_handle_count.invokeExact();
            check(live == 0,
                    "Iteration " + i + ": " + live + " live handles (expected 0)");

            System.out.printf("    Iteration %2d: handle=%d → shutdown → OK (live=%d)%n",
                    i, handle, live);
        }

        // ── Negative: wrong ABI version ──

        long bad = (long) morrow_init.invokeExact(0x0002_0000); // major 2 ≠ major 1
        check(bad == 0, "Wrong ABI major must be rejected");

        // ── Negative: unknown handle ──

        int s1 = (int) morrow_shutdown.invokeExact(0xDEAD_BEEFL);
        check(s1 == PanamaBridge.RESULT_ERR_INVALID_HANDLE,
                "Unknown handle must return INVALID_HANDLE");

        // ── Negative: double shutdown ──

        long h = (long) morrow_init.invokeExact(PanamaBridge.ABI_VERSION);
        check(h != 0, "Init before double-shutdown test failed");
        check((int) morrow_shutdown.invokeExact(h) == PanamaBridge.RESULT_OK,
                "First shutdown failed");
        int s2 = (int) morrow_shutdown.invokeExact(h);
        check(s2 == PanamaBridge.RESULT_ERR_INVALID_HANDLE,
                "Double shutdown must be rejected (got " + s2 + ")");

        // ── Final leak check ──

        long live = (long) morrow_handle_count.invokeExact();
        check(live == 0, "Final: " + live + " live handles (expected 0)");

        System.out.println("    ✅ M1 PASSED — " + CYCLES
                + " cycles, no leaks, all edge cases handled.");
        System.out.println("[Morrow] Milestone 1 complete! Runtime is alive.");
    }

    private static void check(boolean condition, String message) {
        if (!condition) {
            System.err.println("    ❌ " + message);
            System.exit(1);
        }
    }
}
