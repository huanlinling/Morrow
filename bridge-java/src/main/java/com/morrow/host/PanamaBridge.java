package com.morrow.host;

import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Panama FFM bridge — manages the native linker, symbol lookup,
 * and downcall MethodHandles for calling into libmorrow_runtime.
 *
 * <p>Usage:
 * <pre>{@code
 *   PanamaBridge bridge = PanamaBridge.create(findNativeLibrary());
 *   int result = (int) bridge.downcall("add",
 *       FunctionDescriptor.of(ValueLayout.JAVA_INT,
 *                             ValueLayout.JAVA_INT,
 *                             ValueLayout.JAVA_INT))
 *       .invokeExact(2, 3);
 * }</pre>
 */
public class PanamaBridge {

    /** Must match {@code abi::ABI_VERSION} in runtime-rs/src/abi/mod.rs. */
    public static final int ABI_VERSION = 0x0001_0000; // v1.0

    // Error codes — mirrors runtime-rs/src/abi/mod.rs (add more when Java checks them)
    public static final int RESULT_OK = 0;
    public static final int RESULT_ERR_INVALID_HANDLE = 3;

    private final Linker linker;
    private final SymbolLookup lookup;
    private final Path libraryPath;

    private PanamaBridge(Linker linker, SymbolLookup lookup, Path libraryPath) {
        this.linker = linker;
        this.lookup = lookup;
        this.libraryPath = libraryPath;
    }

    /**
     * Create a PanamaBridge for the given native library.
     *
     * @param libPath absolute path to the native library (.so / .dll)
     * @return initialized PanamaBridge
     * @throws IllegalArgumentException if the library cannot be loaded
     */
    public static PanamaBridge create(Path libPath) {
        if (!Files.exists(libPath)) {
            throw new IllegalArgumentException("Native library not found: " + libPath);
        }

        System.out.println("[Morrow] Loading: " + libPath);

        Linker linker = Linker.nativeLinker();
        SymbolLookup lookup = SymbolLookup.libraryLookup(libPath, Arena.global());

        return new PanamaBridge(linker, lookup, libPath);
    }

    /**
     * Look up a symbol and create a downcall MethodHandle.
     *
     * @param name exported symbol name (e.g. "add", "morrow_init")
     * @param desc function descriptor matching the C signature
     * @return MethodHandle ready for invokeExact
     * @throws UnsatisfiedLinkError if the symbol is not found
     */
    public MethodHandle downcall(String name, FunctionDescriptor desc) {
        MemorySegment symbol = lookup.find(name)
                .orElseThrow(() -> new UnsatisfiedLinkError(
                        "Symbol '" + name + "' not found in " + libraryPath));

        System.out.println("[Morrow] Found symbol: " + name);
        return linker.downcallHandle(symbol, desc);
    }

    /**
     * Find the native library by searching common locations relative to
     * the current working directory.
     *
     * <p>Typical layout (Cargo workspace): {@code target/release/libmorrow_runtime.so}.
     *
     * @return absolute path to the native library
     * @throws IllegalStateException if the library cannot be found
     */
    public static Path findNativeLibrary() {
        String[] candidates = {
                "../../target/release/libmorrow_runtime.so",
                "../target/release/libmorrow_runtime.so",
                "target/release/libmorrow_runtime.so",
        };

        Path cwd = Path.of("").toAbsolutePath();

        for (String candidate : candidates) {
            Path resolved = cwd.resolve(candidate).normalize();
            if (Files.exists(resolved)) {
                return resolved;
            }
        }

        throw new IllegalStateException(
                "Cannot find libmorrow_runtime.so. "
                + "Build it first: cd runtime-rs && cargo build --release");
    }

    /** @return path to the loaded native library */
    public Path libraryPath() {
        return libraryPath;
    }
}
