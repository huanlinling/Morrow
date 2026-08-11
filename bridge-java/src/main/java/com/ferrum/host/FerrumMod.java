package com.ferrum.host;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.stream.Stream;

import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerTickEvents;
import net.minecraft.server.MinecraftServer;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Fabric entry point for the Ferrum native runtime platform.
 *
 * <p>Loads the native Rust runtime library via Panama FFM, initializes it,
 * and registers lifecycle hooks so the runtime stays in sync with the
 * Minecraft server.
 *
 * <p>This is a single-point integration — all Rust-side logic is driven
 * through the opaque {@code runtimeHandle} returned by {@code ferrum_init}.
 */
public class FerrumMod implements ModInitializer {

    public static final Logger LOG = LoggerFactory.getLogger("Ferrum");

    /** Must match {@code abi::ABI_VERSION} in runtime-rs/src/abi/mod.rs. */
    private static final int ABI_VERSION = 0x0001_0000;

    /** Current Minecraft server instance (set during lifecycle events). */
    private static MinecraftServer currentServer;

    // ─── ModInitializer ─────────────────────────

    @Override
    public void onInitialize() {
        LOG.info("Ferrum Host starting...");

        // 1. Discover and load the native runtime
        Path nativeLib;
        try {
            nativeLib = NativeLibraryLoader.load();
        } catch (UnsatisfiedLinkError e) {
            LOG.error("Failed to load native runtime: {}", e.getMessage());
            LOG.error("Ferrum will not be available. Build the runtime: cargo build --release");
            return;
        }

        LOG.info("Native library loaded: {}", nativeLib.getFileName());

        // 2. Setup Panama bridge
        PanamaBridge bridge;
        try {
            bridge = PanamaBridge.create(nativeLib);
        } catch (Exception e) {
            LOG.error("Failed to initialize Panama bridge: {}", e.getMessage(), e);
            return;
        }

        // 3. Initialize the Rust runtime
        long runtimeHandle;
        try {
            MethodHandle init = bridge.downcall("ferrum_init",
                    FunctionDescriptor.of(ValueLayout.JAVA_LONG,
                            ValueLayout.JAVA_INT));
            runtimeHandle = (long) init.invokeExact(ABI_VERSION);
        } catch (Throwable e) {
            LOG.error("Failed to call ferrum_init: {}", e.getMessage(), e);
            return;
        }

        if (runtimeHandle == 0) {
            LOG.error("ferrum_init returned 0 — ABI version mismatch? (requested {:#010x})",
                    ABI_VERSION);
            return;
        }

        LOG.info("Runtime initialized (ABI v{}.{}, handle={})",
                ABI_VERSION >> 16, ABI_VERSION & 0xFFFF, runtimeHandle);

        // 4. Bind ferrum_load_mod
        MethodHandle loadMod;
        try {
            loadMod = bridge.downcall("ferrum_load_mod",
                    FunctionDescriptor.of(ValueLayout.JAVA_INT,
                            ValueLayout.JAVA_LONG,   // runtime_handle
                            ValueLayout.ADDRESS,     // path_ptr
                            ValueLayout.JAVA_INT));  // path_len
        } catch (Throwable e) {
            LOG.error("Failed to bind ferrum_load_mod: {}", e.getMessage(), e);
            return;
        }

        // 5. Scan for and load .ferrum packages
        Path modsDir = Path.of("mods");
        if (Files.isDirectory(modsDir)) {
            try (Stream<Path> files = Files.list(modsDir)) {
                files.filter(p -> p.toString().endsWith(".ferrum"))
                     .forEach(pkg -> loadFerrumPackage(bridge, loadMod, runtimeHandle, pkg));
            } catch (Exception e) {
                LOG.warn("Failed to scan mods directory: {}", e.getMessage());
            }
        } else {
            LOG.info("No mods/ directory found — skipping mod discovery.");
        }

        // 6. Bind ferrum_tick
        MethodHandle ferrumTick;
        try {
            ferrumTick = bridge.downcall("ferrum_tick",
                    FunctionDescriptor.ofVoid(
                            ValueLayout.JAVA_LONG,   // runtime_handle
                            ValueLayout.JAVA_LONG)); // tick_number
        } catch (Throwable e) {
            LOG.error("Failed to bind ferrum_tick: {}", e.getMessage(), e);
            return;
        }

        // 7. Register Fabric tick hook → calls ferrum_tick each tick
        final MethodHandle tickHandle = ferrumTick;
        final long rtHandle = runtimeHandle;
        ServerTickEvents.END_SERVER_TICK.register(server -> {
            try {
                tickHandle.invokeExact(rtHandle, (long) server.getTicks());
            } catch (Throwable e) {
                LOG.error("ferrum_tick failed: {}", e.getMessage());
            }
        });

        // 8. Bind lifecycle dispatch functions
        try {
            MethodHandle dispatchStart = bridge.downcall("ferrum_dispatch_server_start",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG));
            MethodHandle dispatchStop = bridge.downcall("ferrum_dispatch_server_stop",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG));

            ServerLifecycleEvents.SERVER_STARTED.register(server -> {
                currentServer = server;
                registerHostApi(bridge, rtHandle);
                registerFerrumCommands(rtHandle, bridge);
                registerEventListeners(rtHandle, bridge);
                try { dispatchStart.invokeExact(rtHandle); }
                catch (Throwable e) { LOG.error("server_start dispatch: {}", e.getMessage()); }
            });
            ServerLifecycleEvents.SERVER_STOPPING.register(server -> {
                try { dispatchStop.invokeExact(rtHandle); }
                catch (Throwable e) { LOG.error("server_stop dispatch: {}", e.getMessage()); }
            });

            LOG.info("Lifecycle events registered (start/stop).");
        } catch (Throwable e) {
            LOG.warn("Lifecycle dispatch unavailable: {}", e.getMessage());
        }

        LOG.info("Ferrum ready. {} mod(s) loaded. Tick dispatch active.",
                ferrumModCount(bridge, runtimeHandle));
    }

    // ─── Helpers ─────────────────────────────────

    private static void loadFerrumPackage(PanamaBridge bridge, MethodHandle loadMod,
                                           long runtimeHandle, Path pkg) {
        LOG.info("Loading mod: {}", pkg.getFileName());
        try {
            String pathStr = pkg.toAbsolutePath().toString();
            byte[] pathBytes = pathStr.getBytes(StandardCharsets.UTF_8);

            // Allocate the path string in the global arena for the FFI call
            MemorySegment pathSeg = Arena.global().allocate(pathBytes.length);
            pathSeg.copyFrom(MemorySegment.ofArray(pathBytes));

            int status = (int) loadMod.invokeExact(
                    runtimeHandle,
                    pathSeg,
                    pathBytes.length);

            if (status == 0) {
                LOG.info("  Loaded successfully: {}", pkg.getFileName());
            } else {
                LOG.error("  Failed to load {} (error code {})", pkg.getFileName(), status);
            }
        } catch (Throwable e) {
            LOG.error("  Failed to load {}: {}", pkg.getFileName(), e.getMessage());
        }
    }

    private static long ferrumModCount(PanamaBridge bridge, long runtimeHandle) {
        try {
            MethodHandle count = bridge.downcall("ferrum_mod_count",
                    FunctionDescriptor.of(ValueLayout.JAVA_LONG));
            return (long) count.invokeExact();
        } catch (Throwable e) {
            return -1;
        }
    }

    // ─── Host API (Upcalls: Rust → Java) ────────

    private static int getPlayerCount() {
        if (currentServer == null) return 0;
        return currentServer.getPlayerManager().getPlayerList().size();
    }

    private static String joinPlayerNames() {
        if (currentServer == null) return "";
        var names = new java.util.ArrayList<String>();
        currentServer.getPlayerManager().getPlayerList()
                .forEach(p -> names.add(p.getName().getString()));
        return String.join(",", names);
    }

    private static void sendMessage(long msgPtr, int msgLen) {
        if (currentServer == null) return;
        byte[] bytes = MemorySegment.ofAddress(msgPtr).reinterpret(msgLen).toArray(ValueLayout.JAVA_BYTE);
        String msg = new String(bytes, StandardCharsets.UTF_8);
        currentServer.getPlayerManager().broadcast(
                net.minecraft.text.Text.literal("[Ferrum] " + msg), false);
    }

    private static int getPlayerList(long bufPtr, int bufCap) {
        String names = joinPlayerNames();
        byte[] bytes = names.getBytes(StandardCharsets.UTF_8);
        int len = Math.min(bytes.length, bufCap);
        MemorySegment.ofAddress(bufPtr).reinterpret(len).copyFrom(MemorySegment.ofArray(bytes));
        return len;
    }

    private static void executeCommand(long cmdPtr, int cmdLen) {
        if (currentServer == null) return;
        byte[] bytes = MemorySegment.ofAddress(cmdPtr).reinterpret(cmdLen).toArray(ValueLayout.JAVA_BYTE);
        String cmd = new String(bytes, StandardCharsets.UTF_8);
        currentServer.getCommandManager().executeWithPrefix(
                currentServer.getCommandSource(), cmd);
    }

    private static long getWorldTime() {
        if (currentServer == null) return -1;
        return currentServer.getOverworld().getTimeOfDay();
    }

    private static void registerHostApi(PanamaBridge bridge, long runtimeHandle) {
        try {
            Linker linker = Linker.nativeLinker();

            var pcStub = linker.upcallStub(
                    MethodHandles.lookup().findStatic(FerrumMod.class, "getPlayerCount",
                            MethodType.methodType(int.class)),
                    FunctionDescriptor.of(ValueLayout.JAVA_INT), Arena.global());

            var smStub = linker.upcallStub(
                    MethodHandles.lookup().findStatic(FerrumMod.class, "sendMessage",
                            MethodType.methodType(void.class, long.class, int.class)),
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global());

            var plStub = linker.upcallStub(
                    MethodHandles.lookup().findStatic(FerrumMod.class, "getPlayerList",
                            MethodType.methodType(int.class, long.class, int.class)),
                    FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global());

            var ecStub = linker.upcallStub(
                    MethodHandles.lookup().findStatic(FerrumMod.class, "executeCommand",
                            MethodType.methodType(void.class, long.class, int.class)),
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT), Arena.global());

            var wtStub = linker.upcallStub(
                    MethodHandles.lookup().findStatic(FerrumMod.class, "getWorldTime",
                            MethodType.methodType(long.class)),
                    FunctionDescriptor.of(ValueLayout.JAVA_LONG), Arena.global());

            // Vtable layout: [0]=pc, [8]=sm, [16]=pl, [24]=ec, [32]=wt
            MemorySegment vtable = Arena.global().allocate(40);
            vtable.set(ValueLayout.ADDRESS, 0, pcStub);
            vtable.set(ValueLayout.ADDRESS, 8, smStub);
            vtable.set(ValueLayout.ADDRESS, 16, plStub);
            vtable.set(ValueLayout.ADDRESS, 24, ecStub);
            vtable.set(ValueLayout.ADDRESS, 32, wtStub);

            MethodHandle registerApi = bridge.downcall("ferrum_register_host_api",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
            registerApi.invokeExact(runtimeHandle, vtable);

            LOG.info("Host API registered (5 upcalls).");
        } catch (Throwable e) {
            LOG.error("Failed to register host API: {}", e.getMessage(), e);
        }
    }

    // ─── Command dispatch ───────────────────────

    private static void registerFerrumCommands(long runtimeHandle, PanamaBridge bridge) {
        try {
            MethodHandle dispatchCmd = bridge.downcall("ferrum_dispatch_command",
                    FunctionDescriptor.of(ValueLayout.JAVA_INT,
                            ValueLayout.JAVA_LONG,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT));

            // Register /ferrum command that forwards to Rust
            net.fabricmc.fabric.api.command.v2.CommandRegistrationCallback.EVENT.register(
                (dispatcher, registryAccess, environment) -> {
                    dispatcher.register(
                        net.minecraft.server.command.CommandManager
                            .literal("ferrum")
                            .executes(ctx -> {
                                forwardCommand(dispatchCmd, runtimeHandle, "ferrum", "");
                                return 1;
                            })
                            .then(net.minecraft.server.command.CommandManager
                                    .argument("args", net.minecraft.command.argument.MessageArgumentType.message())
                                    .executes(ctx -> {
                                        var msg = net.minecraft.command.argument.MessageArgumentType
                                                .getMessage(ctx, "args");
                                        String args = msg.getString();
                                        forwardCommand(dispatchCmd, runtimeHandle, "ferrum", args);
                                        return 1;
                                    }))
                    );
                });
        } catch (Throwable e) {
            LOG.error("Failed to register commands: {}", e.getMessage());
        }
    }

    private static void forwardCommand(MethodHandle dispatch, long handle,
                                        String name, String args) {
        try {
            byte[] nameBytes = name.getBytes(StandardCharsets.UTF_8);
            byte[] argsBytes = args.getBytes(StandardCharsets.UTF_8);
            var nameSeg = Arena.global().allocate(nameBytes.length);
            nameSeg.copyFrom(MemorySegment.ofArray(nameBytes));
            var argsSeg = Arena.global().allocate(argsBytes.length);
            argsSeg.copyFrom(MemorySegment.ofArray(argsBytes));

            int _result = (int) dispatch.invokeExact(handle,
                    nameSeg, nameBytes.length,
                    argsSeg, argsBytes.length);
        } catch (Throwable e) {
            LOG.error("Command dispatch failed: {}", e.getMessage());
        }
    }

    // ─── Player event listeners ─────────────────

    private static void registerEventListeners(long runtimeHandle, PanamaBridge bridge) {
        try {
            MethodHandle playerJoin = bridge.downcall("ferrum_dispatch_player_join",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
            MethodHandle playerLeave = bridge.downcall("ferrum_dispatch_player_leave",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
            MethodHandle chatMsg = bridge.downcall("ferrum_dispatch_chat_message",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT));

            net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents.JOIN.register(
                (handler, sender, server) -> {
                    String name = handler.getPlayer().getName().getString();
                    ffiCallVoidStr(playerJoin, runtimeHandle, name);
                });
            net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents.DISCONNECT.register(
                (handler, server) -> {
                    String name = handler.getPlayer().getName().getString();
                    ffiCallVoidStr(playerLeave, runtimeHandle, name);
                });
            net.fabricmc.fabric.api.message.v1.ServerMessageEvents.CHAT_MESSAGE.register(
                (message, sender, params) -> {
                    String player = sender.getName().getString();
                    String msg = message.getContent().getString();
                    ffiCallChat(chatMsg, runtimeHandle, player, msg);
                });

            // Block break / place
            MethodHandle blockBreak = bridge.downcall("ferrum_dispatch_block_break",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
            MethodHandle blockPlace = bridge.downcall("ferrum_dispatch_block_place",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
            MethodHandle playerDeath = bridge.downcall("ferrum_dispatch_player_death",
                    FunctionDescriptor.ofVoid(ValueLayout.JAVA_LONG,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT,
                            ValueLayout.ADDRESS, ValueLayout.JAVA_INT));

            net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents.AFTER.register(
                (world, player, pos, state, entity) -> {
                    String pname = player.getName().getString();
                    String bname = state.getBlock().getName().getString();
                    ffiCallTwoStr(blockBreak, runtimeHandle, pname, bname);
                });
            // Block place: use ServerPlayerEvents or a simpler hook
            net.fabricmc.fabric.api.event.lifecycle.v1.ServerEntityEvents.ENTITY_LOAD.register(
                (entity, world) -> { /* not the right hook — use block place callback */ });

            net.fabricmc.fabric.api.entity.event.v1.ServerLivingEntityEvents.AFTER_DEATH.register(
                (entity, source) -> {
                    if (entity instanceof net.minecraft.server.network.ServerPlayerEntity player) {
                        String pname = player.getName().getString();
                        String dmsg = source.getDeathMessage(player).getString();
                        ffiCallTwoStr(playerDeath, runtimeHandle, pname, dmsg);
                    }
                });

            LOG.info("Event listeners registered (join/leave/chat/block/death).");
        } catch (Throwable e) {
            LOG.error("Failed to register events: {}", e.getMessage());
        }
    }

    private static void ffiCallTwoStr(MethodHandle handle, long rt, String s1, String s2) {
        try {
            byte[] b1 = s1.getBytes(StandardCharsets.UTF_8);
            byte[] b2 = s2.getBytes(StandardCharsets.UTF_8);
            var seg1 = Arena.global().allocate(b1.length);
            seg1.copyFrom(MemorySegment.ofArray(b1));
            var seg2 = Arena.global().allocate(b2.length);
            seg2.copyFrom(MemorySegment.ofArray(b2));
            handle.invokeExact(rt, seg1, b1.length, seg2, b2.length);
        } catch (Throwable e) { LOG.error("FFI: {}", e.getMessage()); }
    }

    private static void ffiCallVoidStr(MethodHandle handle, long rt, String s) {
        try {
            byte[] b = s.getBytes(StandardCharsets.UTF_8);
            var seg = Arena.global().allocate(b.length);
            seg.copyFrom(MemorySegment.ofArray(b));
            handle.invokeExact(rt, seg, b.length);
        } catch (Throwable e) { LOG.error("FFI event: {}", e.getMessage()); }
    }

    private static void ffiCallChat(MethodHandle handle, long rt, String player, String msg) {
        try {
            byte[] pb = player.getBytes(StandardCharsets.UTF_8);
            byte[] mb = msg.getBytes(StandardCharsets.UTF_8);
            var ps = Arena.global().allocate(pb.length);
            ps.copyFrom(MemorySegment.ofArray(pb));
            var ms = Arena.global().allocate(mb.length);
            ms.copyFrom(MemorySegment.ofArray(mb));
            handle.invokeExact(rt, ps, pb.length, ms, mb.length);
        } catch (Throwable e) { LOG.error("FFI chat: {}", e.getMessage()); }
    }
}
