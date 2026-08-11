package com.morrow.host;

import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;

/**
 * Milestone 0 regression test.
 *
 * Calls {@code add(2, 3)} via Panama FFM and verifies the result is 5.
 */
public class M0_AddTest {

    public static void main(String[] args) throws Throwable {
        System.out.println("==> M0 Regression: add(2, 3) = ?");

        PanamaBridge bridge = PanamaBridge.create(
                PanamaBridge.findNativeLibrary());

        MethodHandle add = bridge.downcall("add",
                FunctionDescriptor.of(ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT));

        int result = (int) add.invokeExact(2, 3);
        System.out.println("    2 + 3 = " + result);

        if (result == 5) {
            System.out.println("    ✅ M0 PASSED");
        } else {
            System.err.println("    ❌ M0 FAILED: expected 5, got " + result);
            System.exit(1);
        }
    }
}
