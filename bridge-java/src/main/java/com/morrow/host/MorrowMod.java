package com.morrow.host;

import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;

/**
 * Morrow native runtime platform — no Fabric API, no Minecraft types.
 *
 * <p>Initialized via Mixin hook (MinecraftServerMixin /
 * MinecraftServerMixinVanilla), not ModInitializer. The mixin passes the
 * game-facing {@link ServerApi} adapter — kept out of this class so it
 * links in a fully obfuscated production jar (agent mode).
 */
public class MorrowMod {

    /**
     * Minimal logger. The vanilla server ships slf4j only as a nested
     * library (META-INF/libraries/), invisible to the agent classloader —
     * System.out lands in the server log in both agent and Fabric modes.
     */
    private static void log(String level, String msg) {
        System.out.println("[Morrow][" + level + "] " + msg);
    }

    private static PanamaBridge bridge;
    private static long runtimeHandle;
    private static ServerApi api;
    private static boolean initialized;

    // ─── Entry point (called from Mixin) ──────────

    /** Called by the mixin once the server is ready. */
    public static void init(ServerApi gameApi) {
        if (initialized) return;
        api = gameApi;
        log("INFO", "Morrow loading...");

        // 1. Native runtime
        Path nativeLib;
        try { nativeLib = NativeLibraryLoader.load(); }
        catch (UnsatisfiedLinkError e) {
            log("ERROR", "Native runtime not found: " + e.getMessage()); return; }
        log("INFO", "Native: " + nativeLib.getFileName());

        // 2. Panama bridge
        try { bridge = PanamaBridge.create(nativeLib); }
        catch (Exception e) { log("ERROR", "Panama: " + e.getMessage()); return; }

        // 3. Init runtime
        try {
            var init = bridge.downcall("morrow_init",
                    FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT));
            runtimeHandle = (long) init.invokeExact(PanamaBridge.ABI_VERSION);
        } catch (Throwable e) { log("ERROR", "morrow_init: " + e.getMessage()); return; }
        if (runtimeHandle == 0) { log("ERROR", "ABI mismatch"); return; }
        log("INFO", "Runtime handle=" + runtimeHandle);

        // 4. Load .morrow packages
        MethodHandle loadMod;
        try {
            loadMod = bridge.downcall("morrow_load_mod",
                    FunctionDescriptor.of(ValueLayout.JAVA_INT,
                            ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
        } catch (Throwable e) { log("ERROR", "load_mod: " + e.getMessage()); return; }

        Path modsDir = Path.of("mods");
        if (Files.isDirectory(modsDir)) {
            try {
                var pkgs = Files.list(modsDir).filter(p -> p.toString().endsWith(".morrow")).toList();
                var failed = new ArrayList<Path>();
                for (var p : pkgs) {
                    if (!loadPackage(bridge, loadMod, p)) failed.add(p);
                }
                for (var p : failed) { loadPackage(bridge, loadMod, p); }
            } catch (Exception e) { log("WARN", "mod scan: " + e.getMessage()); }
        }

        // 5. Batch dispatch (replaces individual tick/event calls)
        try {
            dispatchBatch = bridge.downcall("morrow_dispatch_batch",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
        } catch (Throwable e) { log("ERROR", "batch: " + e.getMessage()); }

        // 6. Dispatch server start
        try {
            var start = bridge.downcall("morrow_dispatch_server_start",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG));
            start.invokeExact(runtimeHandle);
        } catch (Throwable e) { log("WARN", "start: " + e.getMessage()); }

        // 7. Host API upcalls
        registerHostApi();

        initialized = true;
        log("INFO", "Morrow ready. " + modCount() + " mod(s).");
    }

    private static MethodHandle dispatchBatch;
    private static final EventBuffer eventBuffer = new EventBuffer();

    // ─── Tick (called from Mixin) ─────────────────

    /** Buffer a tick event, flush at end of tick. */
    public static void onTick(long tick) {
        eventBuffer.tick(tick);
    }

    // ─── Game events (called from Mixin injection points) ───
    // Capture lives in the mixins; these just forward to the batch buffer.

    public static void onPlayerJoin(String name) { eventBuffer.playerJoin(name); }
    public static void onPlayerLeave(String name) { eventBuffer.playerLeave(name); }
    public static void onChat(String player, String msg) { eventBuffer.chat(player, msg); }
    public static void onBlockBreak(String player, String block) { eventBuffer.blockBreak(player, block); }
    public static void onBlockPlace(String player, String block) { eventBuffer.blockPlace(player, block); }
    public static void onPlayerDeath(String player) { eventBuffer.playerDeath(player); }

    /** Flush accumulated events to Rust in one FFM call. */
    public static void flushBatch() {
        if (dispatchBatch == null) return;
        // Hold the EventBuffer monitor across finish → dispatch → reset:
        // a Netty-thread append between finish and the downcall would
        // write into the segment Rust is reading (torn batch); one after
        // reset would miss the batch entirely.
        synchronized (eventBuffer) {
            if (eventBuffer.isEmpty()) return;
            try {
                // EventBuffer owns a per-tick arena — the segment is
                // native memory, no Java-heap copy, no Arena.global() growth.
                var seg = eventBuffer.finish();
                dispatchBatch.invokeExact(runtimeHandle, seg, eventBuffer.size());
            } catch (Throwable e) { log("ERROR", "batch: " + e.getMessage()); }
            finally { eventBuffer.reset(); } // close arena + start the next tick clean
        }
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
    // Game-facing upcalls (getPlayerCount, sendMessage, ...) live in the
    // ServerApi adapter; the vtable binds them as virtual method handles
    // with the adapter instance as receiver. logMessage is game-free and
    // stays here as a static.

    private static void logMessage(int level, long ptr, int len) {
        byte[] b = MemorySegment.ofAddress(ptr).reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
        String msg = new String(b, StandardCharsets.UTF_8);
        switch (level) { case 3 -> log("ERROR", msg); case 2 -> log("WARN", msg); case 1 -> log("INFO", msg); default -> log("DEBUG", msg); }
    }

    private static void registerHostApi() {
        try {
            var lookup = MethodHandles.lookup(); var linker = Linker.nativeLinker();
            var vtable = Arena.global().allocate(56);
            vtable.set(ValueLayout.ADDRESS, 0, linker.upcallStub(
                    lookup.findVirtual(ServerApi.class, "getPlayerCount", MethodType.methodType(int.class)).bindTo(api),
                    FunctionDescriptor.of(ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 8, linker.upcallStub(
                    lookup.findVirtual(ServerApi.class, "sendMessage", MethodType.methodType(void.class, long.class, int.class)).bindTo(api),
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 16, linker.upcallStub(
                    lookup.findVirtual(ServerApi.class, "getPlayerList", MethodType.methodType(int.class, long.class, int.class)).bindTo(api),
                    FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 24, linker.upcallStub(
                    lookup.findVirtual(ServerApi.class, "executeCommand", MethodType.methodType(void.class, long.class, int.class)).bindTo(api),
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 32, linker.upcallStub(
                    lookup.findVirtual(ServerApi.class, "getWorldTime", MethodType.methodType(long.class)).bindTo(api),
                    FunctionDescriptor.of(ValueLayout.JAVA_LONG), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 40, linker.upcallStub(
                    lookup.findStatic(MorrowMod.class, "logMessage", MethodType.methodType(void.class, int.class, long.class, int.class)),
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            vtable.set(ValueLayout.ADDRESS, 48, linker.upcallStub(
                    lookup.findVirtual(ServerApi.class, "getWorldSnapshot", MethodType.methodType(int.class, long.class, int.class)).bindTo(api),
                    FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global()));
            var ra = bridge.downcall("morrow_register_host_api", FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
            ra.invokeExact(runtimeHandle, vtable);
        } catch (Throwable e) { log("ERROR", "Host API: " + e.getMessage()); }
    }
}
