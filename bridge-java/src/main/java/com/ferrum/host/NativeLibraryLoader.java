package com.ferrum.host;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;

/**
 * Platform-aware native library discovery and loading.
 *
 * <p>Search order:
 * <ol>
 *   <li>{@code -Dferrum.native.dir} system property (dev: points at Cargo target/)</li>
 *   <li>JAR resources under {@code natives/<os>-<arch>/} (production)</li>
 *   <li>{@code java.library.path}</li>
 * </ol>
 *
 * <p>After finding the library, it passes the absolute path to
 * {@link PanamaBridge} for loading via Panama FFM.
 */
public final class NativeLibraryLoader {

    private NativeLibraryLoader() { /* static utility */ }

    /** Basename of the runtime library (no prefix/suffix). */
    private static final String LIB_NAME = "ferrum_runtime";

    /**
     * Find and load the native runtime library.
     *
     * @return absolute path to the loaded library
     * @throws UnsatisfiedLinkError if the library cannot be found or loaded
     */
    public static Path load() {
        // 1. Explicit dev override
        String devDir = System.getProperty("ferrum.native.dir");
        if (devDir != null) {
            Path candidate = Path.of(devDir).resolve(mapLibraryName());
            if (Files.exists(candidate)) {
                System.out.println("[Ferrum] Native: " + candidate + " (ferrum.native.dir)");
                return candidate;
            }
        }

        // 2. JAR resources (production packaging)
        String resourcePath = "natives/" + platformDir() + "/" + mapLibraryName();
        try (InputStream is = NativeLibraryLoader.class
                .getClassLoader().getResourceAsStream(resourcePath)) {
            if (is != null) {
                // Extract to a temp file so dlopen can load it
                Path tmp = Files.createTempFile("ferrum_runtime_", "." + platformExtension());
                tmp.toFile().deleteOnExit();
                Files.copy(is, tmp, StandardCopyOption.REPLACE_EXISTING);
                System.out.println("[Ferrum] Native: " + tmp + " (extracted from JAR: " + resourcePath + ")");
                return tmp;
            }
        } catch (IOException e) {
            // Fall through to next strategy
        }

        // 3. java.library.path (system-installed)
        try {
            System.loadLibrary(LIB_NAME);
            // If we get here, System.loadLibrary succeeded.
            // We still need the path for SymbolLookup — scan java.library.path.
            String libPaths = System.getProperty("java.library.path", "");
            for (String dir : libPaths.split(":")) {
                Path candidate = Path.of(dir.trim()).resolve(mapLibraryName());
                if (Files.exists(candidate)) {
                    return candidate;
                }
            }
        } catch (UnsatisfiedLinkError e) {
            // Fall through
        }

        throw new UnsatisfiedLinkError(
                "Cannot find " + mapLibraryName() + ". "
                + "Build it first: cargo build --release. "
                + "Or set -Dferrum.native.dir=<path>/target/release");
    }

    // ─── Platform helpers ───────────────────────

    /** Library filename: "libferrum_runtime.so" on Linux, "ferrum_runtime.dll" on Windows. */
    public static String mapLibraryName() {
        String os = osName();
        if (os.contains("win")) {
            return LIB_NAME + ".dll";
        } else if (os.contains("mac")) {
            return "lib" + LIB_NAME + ".dylib";
        } else {
            return "lib" + LIB_NAME + ".so";
        }
    }

    /** "linux-x86_64", "windows-x86_64", etc. */
    private static String platformDir() {
        String os = osName();
        String arch = System.getProperty("os.arch", "amd64").toLowerCase(Locale.ROOT);

        if (arch.equals("amd64") || arch.equals("x86_64")) {
            arch = "x86_64";
        } else if (arch.equals("aarch64")) {
            arch = "aarch64";
        }

        if (os.contains("win")) {
            return "windows-" + arch;
        } else if (os.contains("mac")) {
            return "macos-" + arch;
        } else {
            return "linux-" + arch;
        }
    }

    private static String platformExtension() {
        if (osName().contains("win")) return "dll";
        if (osName().contains("mac")) return "dylib";
        return "so";
    }

    private static String osName() {
        return System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
    }
}
