package com.ferrum.host;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.stream.Stream;

import net.fabricmc.api.ModInitializer;
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
}
