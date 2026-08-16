package com.morrow.host;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;

/**
 * Accumulates game events during a tick and provides a single native
 * buffer for batch FFM dispatch.
 *
 * <p>Writes directly into an off-heap {@link MemorySegment} allocated from
 * a per-tick {@link Arena} (design.md §5.1: per-tick confined arena) —
 * no Java-heap ByteBuffer round trip, no {@code Arena.global()} growth.
 * The arena is closed in {@link #reset()}, which the host calls once per
 * tick after dispatch, so freed memory is returned promptly.
 *
 * <p>Binary format:
 * <pre>
 * u32le: total_events
 * for each:
 *   u16le: event_type (0=tick, 1=join, 2=leave, 3=chat,
 *                      4=break, 5=place, 6=death)
 *   u16le: field1_len
 *   u16le: field2_len
 *   field1 bytes
 *   field2 bytes (may be empty)
 * </pre>
 * Tick events stuff the 8-byte tick number after the header (field1/2
 * are empty) — 14 bytes total.
 */
public class EventBuffer {
    private static final int INITIAL_CAPACITY = 4096;

    private Arena arena;      // per-tick confined arena, closed on reset()
    private MemorySegment seg;
    private int pos;          // write cursor (bytes written)
    private int count;        // events in the current batch

    public EventBuffer() {
        reset();
    }

    public void tick(long tick) {
        ensure(14);
        seg.set(ValueLayout.JAVA_SHORT_UNALIGNED, pos, (short) 0); pos += 2;
        seg.set(ValueLayout.JAVA_SHORT_UNALIGNED, pos, (short) 0); pos += 2;
        seg.set(ValueLayout.JAVA_SHORT_UNALIGNED, pos, (short) 0); pos += 2;
        seg.set(ValueLayout.JAVA_LONG_UNALIGNED, pos, tick); pos += 8;
        count++;
    }

    public void playerJoin(String name) {
        byte[] b = name.getBytes(StandardCharsets.UTF_8);
        header(1, b.length, 0);
        putBytes(b);
        count++;
    }

    public void playerLeave(String name) {
        byte[] b = name.getBytes(StandardCharsets.UTF_8);
        header(2, b.length, 0);
        putBytes(b);
        count++;
    }

    public void chat(String player, String msg) {
        byte[] p = player.getBytes(StandardCharsets.UTF_8);
        byte[] m = msg.getBytes(StandardCharsets.UTF_8);
        header(3, p.length, m.length);
        putBytes(p);
        putBytes(m);
        count++;
    }

    public void blockBreak(String player, String block) {
        byte[] p = player.getBytes(StandardCharsets.UTF_8);
        byte[] b = block.getBytes(StandardCharsets.UTF_8);
        header(4, p.length, b.length);
        putBytes(p);
        putBytes(b);
        count++;
    }

    public void blockPlace(String player, String block) {
        byte[] p = player.getBytes(StandardCharsets.UTF_8);
        byte[] b = block.getBytes(StandardCharsets.UTF_8);
        header(5, p.length, b.length);
        putBytes(p);
        putBytes(b);
        count++;
    }

    public void playerDeath(String player) {
        byte[] b = player.getBytes(StandardCharsets.UTF_8);
        header(6, b.length, 0);
        putBytes(b);
        count++;
    }

    /** Finalize the batch: stamp the event count at offset 0. */
    public MemorySegment finish() {
        seg.set(ValueLayout.JAVA_INT_UNALIGNED, 0, count);
        return seg;
    }

    /** Bytes written (excluding the reserved count slot's padding — i.e.
     *  the exact payload length to pass to FFM dispatch). */
    public int size() {
        return pos;
    }

    /**
     * Start a fresh batch: close the previous tick's arena (frees its
     * memory immediately) and allocate a new one. Called by the host
     * after each dispatch.
     */
    public void reset() {
        if (arena != null) {
            arena.close();
        }
        arena = Arena.ofConfined();
        seg = arena.allocate(INITIAL_CAPACITY);
        pos = 4; // reserve u32 count slot
        count = 0;
    }

    public boolean isEmpty() {
        return count == 0;
    }

    private void header(int type, int len1, int len2) {
        ensure(6);
        seg.set(ValueLayout.JAVA_SHORT_UNALIGNED, pos, (short) type); pos += 2;
        seg.set(ValueLayout.JAVA_SHORT_UNALIGNED, pos, (short) len1); pos += 2;
        seg.set(ValueLayout.JAVA_SHORT_UNALIGNED, pos, (short) len2); pos += 2;
    }

    private void putBytes(byte[] b) {
        ensure(b.length);
        MemorySegment.copy(MemorySegment.ofArray(b), 0, seg, pos, b.length);
        pos += b.length;
    }

    /** Grow the segment (within the same arena) when capacity is short.
     *  Confined arenas allow multiple allocations; old segment becomes
     *  garbage and is freed with the arena at reset(). */
    private void ensure(int extra) {
        if (pos + extra > seg.byteSize()) {
            long newCap = Math.max(seg.byteSize() * 2, pos + extra);
            MemorySegment bigger = arena.allocate(newCap);
            MemorySegment.copy(seg, 0, bigger, 0, pos);
            seg = bigger;
        }
    }
}
