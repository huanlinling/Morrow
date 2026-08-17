package com.morrow.host;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;

/**
 * Event wire-format parity test — pins the type codes written by
 * {@link EventBuffer} to the canonical table in docs/02-abi-design.md
 * §事件类型码. The Rust parser side is pinned to the same table by
 * {@code runtime-rs/tests/mod_loader_integration.rs}. Change neither
 * side without updating the spec — this test fails on drift.
 */
public class EventBufferCodeTest {

    public static void main(String[] args) {
        System.out.println("==> Event Code Parity: Java EventBuffer vs spec");

        EventBuffer buf = new EventBuffer();
        buf.tick(42L);
        buf.playerJoin("alice");
        buf.playerLeave("dave");
        buf.chat("bob", "hi");
        buf.blockBreak("carol", "stone");
        buf.blockPlace("carol", "dirt");
        buf.playerDeath("erin");

        MemorySegment seg = buf.finish();

        int count = seg.get(ValueLayout.JAVA_INT_UNALIGNED, 0);
        check(count == 7, "count must be 7, got " + count);

        // Walk the wire format, asserting each event's type code and
        // the tick event's 8-byte payload.
        int[] expectedCodes = {0, 1, 2, 3, 4, 5, 6};
        int pos = 4;
        for (int code : expectedCodes) {
            int type = seg.get(ValueLayout.JAVA_SHORT_UNALIGNED, pos);
            check(type == code, "event code " + code + " written as " + type);
            int len1 = seg.get(ValueLayout.JAVA_SHORT_UNALIGNED, pos + 2);
            int len2 = seg.get(ValueLayout.JAVA_SHORT_UNALIGNED, pos + 4);
            if (code == 0) {
                check(len1 == 0 && len2 == 0, "tick event fields must be empty");
                long tick = seg.get(ValueLayout.JAVA_LONG_UNALIGNED, pos + 6);
                check(tick == 42L, "tick payload must be 42, got " + tick);
                pos += 6 + 8;
            } else {
                pos += 6 + len1 + len2;
            }
        }

        System.out.println("    ✅ Event codes match docs/02-abi-design.md");
    }

    private static void check(boolean ok, String what) {
        if (!ok) {
            System.err.println("    ❌ FAILED: " + what);
            System.exit(1);
        }
    }
}
