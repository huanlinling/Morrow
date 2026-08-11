package com.morrow.host;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;

/**
 * Accumulates game events during a tick and provides a single
 * binary buffer for batch FFM dispatch.
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
 */
public class EventBuffer {
    private static final int INITIAL_CAPACITY = 4096;
    private final ByteBuffer buf = ByteBuffer.allocate(INITIAL_CAPACITY)
            .order(ByteOrder.LITTLE_ENDIAN);
    private int count;

    public EventBuffer() {
        buf.position(4); // reserve u32 count
    }

    public void tick(long tick) {
        ensure(12);
        buf.putShort((short) 0); // type
        buf.putShort((short) 0); // field1_len
        buf.putShort((short) 0); // field2_len
        buf.putLong(tick);        // tick number stuffed after header
        count++;
    }

    public void playerJoin(String name) {
        byte[] b = name.getBytes(StandardCharsets.UTF_8);
        ensure(6 + b.length);
        buf.putShort((short) 1);
        buf.putShort((short) b.length);
        buf.putShort((short) 0);
        buf.put(b);
        count++;
    }

    public void playerLeave(String name) {
        byte[] b = name.getBytes(StandardCharsets.UTF_8);
        ensure(6 + b.length);
        buf.putShort((short) 2);
        buf.putShort((short) b.length);
        buf.putShort((short) 0);
        buf.put(b);
        count++;
    }

    public void chat(String player, String msg) {
        byte[] p = player.getBytes(StandardCharsets.UTF_8);
        byte[] m = msg.getBytes(StandardCharsets.UTF_8);
        ensure(6 + p.length + m.length);
        buf.putShort((short) 3);
        buf.putShort((short) p.length);
        buf.putShort((short) m.length);
        buf.put(p);
        buf.put(m);
        count++;
    }

    public void blockBreak(String player, String block) {
        byte[] p = player.getBytes(StandardCharsets.UTF_8);
        byte[] b = block.getBytes(StandardCharsets.UTF_8);
        ensure(6 + p.length + b.length);
        buf.putShort((short) 4);
        buf.putShort((short) p.length);
        buf.putShort((short) b.length);
        buf.put(p);
        buf.put(b);
        count++;
    }

    public void blockPlace(String player, String block) {
        byte[] p = player.getBytes(StandardCharsets.UTF_8);
        byte[] b = block.getBytes(StandardCharsets.UTF_8);
        ensure(6 + p.length + b.length);
        buf.putShort((short) 5);
        buf.putShort((short) p.length);
        buf.putShort((short) b.length);
        buf.put(p);
        buf.put(b);
        count++;
    }

    public void playerDeath(String player) {
        byte[] b = player.getBytes(StandardCharsets.UTF_8);
        ensure(6 + b.length);
        buf.putShort((short) 6);
        buf.putShort((short) b.length);
        buf.putShort((short) 0);
        buf.put(b);
        count++;
    }

    /** Finalize and return the buffer. Call once per tick. */
    public ByteBuffer finish() {
        buf.putInt(0, count); // write count at position 0
        buf.flip();
        return buf;
    }

    public boolean isEmpty() { return count == 0; }

    private void ensure(int extra) {
        if (buf.remaining() < extra) {
            // Grow: double capacity
            int newCap = buf.capacity() * 2;
            ByteBuffer bigger = ByteBuffer.allocate(newCap).order(ByteOrder.LITTLE_ENDIAN);
            buf.flip();
            bigger.put(buf);
            buf.clear();
            // Copy back
            bigger.flip();
            buf.clear();
            buf.put(bigger);
        }
    }
}
