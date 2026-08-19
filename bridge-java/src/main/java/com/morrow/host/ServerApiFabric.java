package com.morrow.host;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;

import net.minecraft.server.MinecraftServer;
import net.minecraft.text.Text;

/**
 * Host API against dev/loom names (yarn). Production agent mode uses
 * {@code ServerApiVanilla} (obfuscated names) instead — same interface,
 * same vtable order.
 */
public class ServerApiFabric implements ServerApi {

    private final MinecraftServer server;

    public ServerApiFabric(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public int getPlayerCount() {
        return server.getPlayerManager().getPlayerList().size();
    }

    @Override
    public void sendMessage(long ptr, int len) {
        byte[] b = MemorySegment.ofAddress(ptr).reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
        server.getPlayerManager().broadcast(
                Text.literal("[Morrow] " + new String(b, StandardCharsets.UTF_8)), false);
    }

    @Override
    public int getPlayerList(long buf, int cap) {
        String names = joinNames();
        byte[] b = names.getBytes(StandardCharsets.UTF_8);
        int n = Math.min(b.length, cap);
        MemorySegment.ofAddress(buf).reinterpret(n).copyFrom(MemorySegment.ofArray(b));
        return n;
    }

    @Override
    public void executeCommand(long ptr, int len) {
        byte[] b = MemorySegment.ofAddress(ptr).reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
        server.getCommandManager().executeWithPrefix(
                server.getCommandSource(), new String(b, StandardCharsets.UTF_8));
    }

    @Override
    public long getWorldTime() {
        return server.getOverworld().getTimeOfDay();
    }

    @Override
    public int getWorldSnapshot(long bufPtr, int bufCap) {
        var players = server.getPlayerManager().getPlayerList();
        var buf = MemorySegment.ofAddress(bufPtr).reinterpret(bufCap);
        int pos = 0;
        // u32: player count
        buf.set(ValueLayout.JAVA_INT_UNALIGNED, pos, players.size()); pos += 4;
        // u64: world time
        buf.set(ValueLayout.JAVA_LONG_UNALIGNED, pos, getWorldTime()); pos += 8;
        for (var p : players) {
            byte[] name = p.getName().getString().getBytes(StandardCharsets.UTF_8);
            if (pos + 2 + name.length > bufCap) break;
            buf.set(ValueLayout.JAVA_SHORT_UNALIGNED, pos, (short) name.length); pos += 2;
            for (byte b : name) { buf.set(ValueLayout.JAVA_BYTE, pos++, b); }
        }
        return pos;
    }

    private String joinNames() {
        var names = new java.util.ArrayList<String>();
        server.getPlayerManager().getPlayerList().forEach(p -> names.add(p.getName().getString()));
        return String.join(",", names);
    }
}
