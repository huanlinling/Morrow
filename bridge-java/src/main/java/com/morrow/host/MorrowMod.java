package com.morrow.host;

import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;

import net.minecraft.server.MinecraftServer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Morrow native runtime platform — no Fabric API required.
 *
 * <p>Initialized via Mixin hook (MinecraftServerMixin), not ModInitializer.
 * Drop {@code morrow.jar} in mods/ and Rust mods (.morrow) in mods/.
 */
public class MorrowMod {

    public static final Logger LOG = LoggerFactory.getLogger("Morrow");
    private static final int ABI_VERSION = 0x0001_0000;

    private static PanamaBridge bridge;
    private static long runtimeHandle;
    private static MinecraftServer server;
    private static boolean initialized;

    // ─── Entry point (called from Mixin) ──────────

    /** Called by MinecraftServerMixin when the server is ready. */
    public static void init(MinecraftServer mcServer) {
        if (initialized) return;
        server = mcServer;
        LOG.info("Morrow loading...");

        // 1. Native runtime
        Path nativeLib;
        try { nativeLib = NativeLibraryLoader.load(); }
        catch (UnsatisfiedLinkError e) {
            LOG.error("Native runtime not found: {}", e.getMessage()); return; }
        LOG.info("Native: {}", nativeLib.getFileName());

        // 2. Panama bridge
        try { bridge = PanamaBridge.create(nativeLib); }
        catch (Exception e) { LOG.error("Panama: {}", e.getMessage()); return; }

        // 3. Init runtime
        try {
            var init = bridge.downcall("morrow_init",
                    FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT));
            runtimeHandle = (long) init.invokeExact(ABI_VERSION);
        } catch (Throwable e) { LOG.error("morrow_init: {}", e.getMessage()); return; }
        if (runtimeHandle == 0) { LOG.error("ABI mismatch"); return; }
        LOG.info("Runtime handle={}", runtimeHandle);

        // 4. Load .morrow packages
        MethodHandle loadMod;
        try {
            loadMod = bridge.downcall("morrow_load_mod",
                    FunctionDescriptor.of(ValueLayout.JAVA_INT,
                            ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
        } catch (Throwable e) { LOG.error("load_mod: {}", e.getMessage()); return; }

        Path modsDir = Path.of("mods");
        if (Files.isDirectory(modsDir)) {
            try {
                var pkgs = Files.list(modsDir).filter(p -> p.toString().endsWith(".morrow")).toList();
                var failed = new ArrayList<Path>();
                for (var p : pkgs) {
                    if (!loadPackage(bridge, loadMod, p)) failed.add(p);
                }
                for (var p : failed) { loadPackage(bridge, loadMod, p); }
            } catch (Exception e) { LOG.warn("mod scan: {}", e.getMessage()); }
        }

        // 5. Batch dispatch (replaces individual tick/event calls)
        try {
            dispatchBatch = bridge.downcall("morrow_dispatch_batch",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
        } catch (Throwable e) { LOG.error("batch: {}", e.getMessage()); }

        // 6. Dispatch server start
        try {
            var start = bridge.downcall("morrow_dispatch_server_start",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG));
            start.invokeExact(runtimeHandle);
        } catch (Throwable e) { LOG.warn("start: {}", e.getMessage()); }

        // 7. Host API upcalls
        registerHostApi();

        initialized = true;
        LOG.info("Morrow ready. {} mod(s).", modCount());
    }

    private static MethodHandle dispatchBatch;
    private static final EventBuffer eventBuffer = new EventBuffer();

    // ─── Tick (called from Mixin) ─────────────────

    /** Buffer a tick event, flush at end of tick. */
    public static void onTick(long tick) {
        eventBuffer.tick(tick);
    }

    /** Flush accumulated events to Rust in one FFM call. */
    public static void flushBatch() {
        if (dispatchBatch == null || eventBuffer.isEmpty()) return;
        try {
            // EventBuffer owns a per-tick confined arena — the segment is
            // native memory, no Java-heap copy, no Arena.global() growth.
            var seg = eventBuffer.finish();
            dispatchBatch.invokeExact(runtimeHandle, seg, eventBuffer.size());
        } catch (Throwable e) { LOG.error("batch: {}", e.getMessage()); }
        finally { eventBuffer.reset(); } // close arena + start the next tick clean
    }

    // ─── Shutdown (called from Mixin) ────────────

    public static void onShutdown() {
        if (!initialized) return;
        try {
            var stop = bridge.downcall("morrow_dispatch_server_stop",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG));
            stop.invokeExact(runtimeHandle);
        } catch (Throwable e) { /* ignore */ }
        try {
            var s = bridge.downcall("morrow_shutdown",
                    FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG));
            s.invokeExact(runtimeHandle);
        } catch (Throwable e) { /* ignore */ }
        initialized = false;
    }

    // ─── Helpers ─────────────────────────────────

    private static boolean loadPackage(PanamaBridge b, MethodHandle lm, Path pkg) {
        try {
            String s = pkg.toAbsolutePath().toString();
            byte[] bytes = s.getBytes(StandardCharsets.UTF_8);
            var seg = Arena.global().allocate(bytes.length);
            seg.copyFrom(MemorySegment.ofArray(bytes));
            return 0 == (int) lm.invokeExact(runtimeHandle, seg, bytes.length);
        } catch (Throwable e) { return false; }
    }

    private static long modCount() {
        try {
            var c = bridge.downcall("morrow_mod_count",
                    FunctionDescriptor.of(ValueLayout.JAVA_LONG));
            return (long) c.invokeExact();
        } catch (Throwable e) { return -1; }
    }

    // ─── Host API upcalls ────────────────────────

    private static int getPlayerCount() { return server.getPlayerManager().getPlayerList().size(); }
    private static String joinNames() {
        var names = new ArrayList<String>();
        server.getPlayerManager().getPlayerList().forEach(p -> names.add(p.getName().getString()));
        return String.join(",", names);
    }
    private static void sendMessage(long ptr, int len) {
        byte[] b = MemorySegment.ofAddress(ptr).reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
        server.getPlayerManager().broadcast(net.minecraft.text.Text.literal("[Morrow] " + new String(b, StandardCharsets.UTF_8)), false);
    }
    private static int getPlayerList(long buf, int cap) {
        String names = joinNames(); byte[] b = names.getBytes(StandardCharsets.UTF_8);
        int n = Math.min(b.length, cap);
        MemorySegment.ofAddress(buf).reinterpret(n).copyFrom(MemorySegment.ofArray(b));
        return n;
    }
    private static void executeCommand(long ptr, int len) {
        byte[] b = MemorySegment.ofAddress(ptr).reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
        server.getCommandManager().executeWithPrefix(server.getCommandSource(), new String(b, StandardCharsets.UTF_8));
    }
    private static long getWorldTime() { return server.getOverworld().getTimeOfDay(); }

    /** Fill buffer with world snapshot (players + time), return bytes written. */
    private static int getWorldSnapshot(long bufPtr, int bufCap) {
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

    private static void logMessage(int level, long ptr, int len) {
        byte[] b = MemorySegment.ofAddress(ptr).reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
        String msg = new String(b, StandardCharsets.UTF_8);
        switch (level) { case 3 -> LOG.error(msg); case 2 -> LOG.warn(msg); case 1 -> LOG.info(msg); default -> LOG.debug(msg); }
    }

    private static void registerHostApi() {
        try {
            var lookup = MethodHandles.lookup(); var linker = Linker.nativeLinker();
            var vtable = Arena.global().allocate(56);
            vtable.set(ValueLayout.ADDRESS, 0, linker.upcallStub(lookup.findStatic(MorrowMod.class, "getPlayerCount", MethodType.methodType(int.class)), FunctionDescriptor.of(ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 8, linker.upcallStub(lookup.findStatic(MorrowMod.class, "sendMessage", MethodType.methodType(void.class, long.class, int.class)), FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 16, linker.upcallStub(lookup.findStatic(MorrowMod.class, "getPlayerList", MethodType.methodType(int.class, long.class, int.class)), FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 24, linker.upcallStub(lookup.findStatic(MorrowMod.class, "executeCommand", MethodType.methodType(void.class, long.class, int.class)), FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 32, linker.upcallStub(lookup.findStatic(MorrowMod.class, "getWorldTime", MethodType.methodType(long.class)), FunctionDescriptor.of(ValueLayout.JAVA_LONG), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 40, linker.upcallStub(lookup.findStatic(MorrowMod.class, "logMessage", MethodType.methodType(void.class, int.class, long.class, int.class)), FunctionDescriptor.ofVoid(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 48, linker.upcallStub(lookup.findStatic(MorrowMod.class, "getWorldSnapshot", MethodType.methodType(int.class, long.class, int.class)), FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            var ra = bridge.downcall("morrow_register_host_api", FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
            ra.invokeExact(runtimeHandle, vtable);
        } catch (Throwable e) { LOG.error("Host API: {}", e.getMessage()); }
    }
}
