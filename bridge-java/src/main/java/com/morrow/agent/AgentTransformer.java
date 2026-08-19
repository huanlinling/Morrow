package com.morrow.agent;

import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;

import org.spongepowered.asm.mixin.MixinEnvironment;
import org.spongepowered.asm.mixin.Mixins;
import org.spongepowered.asm.mixin.transformer.IMixinTransformer;

/**
 * Bridges the javaagent's {@link ClassFileTransformer} to Mixin's
 * transformer. Registered in premain after {@code MixinBootstrap.init()}
 * completes; Minecraft classes load after main starts, so nothing is
 * missed. The mixin transformer arrives via
 * {@link MixinServiceVanilla#offer}.
 *
 * <p>Two agent-mode specifics live here:
 * <ul>
 * <li>The mixin config is registered on the first transform of
 * {@code net.minecraft.server.Main} — the first class loaded by the
 * vanilla bundler's own classloader. Registering it in premain leaves
 * every @Mixin target "not found": the game classes live in jars the
 * bundler extracts at runtime, invisible to premain's classpath.</li>
 * <li>Class names are passed to the transformer in dot form
 * (LaunchWrapper convention); the agent hands us slash form.</li>
 * </ul>
 */
final class AgentTransformer implements ClassFileTransformer {

    /** Set by {@link MixinServiceVanilla#offer}. */
    static volatile IMixinTransformer transformer;

    /** Set by {@link MorrowAgent#premain}. */
    static volatile Instrumentation instrumentation;

    /**
     * Classloader of the class currently being transformed — the class
     * provider prefers this over the thread context loader for game
     * classes while falling back to the system loader for the agent
     * jar's own classes.
     */
    static volatile ClassLoader currentLoader;

    private static volatile boolean configRegistered;

    @Override
    public byte[] transform(ClassLoader loader, String name, Class<?> beingRedefined,
                            ProtectionDomain protectionDomain, byte[] classfileBuffer) {
        // JDK internals load during JVM bootstrap; touching anything here
        // can cause ClassCircularityError. Return before any work.
        if (name.startsWith("java/") || name.startsWith("jdk/")
                || name.startsWith("sun/") || name.startsWith("javax/")) {
            return null;
        }
        if (name.startsWith("net/minecraft/")) {
            currentLoader = loader;
            // One-time: make the host classes visible to the game loader.
            // The vanilla bundler creates the game classloader as
            // URLClassLoader(urls, systemLoader.getParent()) — parent is
            // the PLATFORM loader, so game classes can never see the app
            // classpath (agent jar included). Injected mixin calls into
            // com.morrow.host.* would fail with NoClassDefFoundError when
            // they execute inside MinecraftServer. Adding the agent jar to
            // the game loader's own URLs fixes it; requires
            // --add-opens java.base/java.net=ALL-UNNAMED on the launch line.
            HostLink.install(loader);
        }
        if (name.equals("net/minecraft/server/Main") && !configRegistered) {
            configRegistered = true;
            Mixins.addConfiguration("morrow.mixins.json");
        }
        IMixinTransformer t = transformer;
        if (t == null) {
            return null;
        }
        // Never touch Mixin's own classes or our host classes.
        if (name.startsWith("org/spongepowered/") || name.startsWith("com/morrow/")) {
            return null;
        }
        try {
            // The DEFAULT-phase environment: mixin's getCurrentEnvironment
            // caches the PREINIT-phase instance for direct (non-abstract)
            // service implementations, and configs never prepare against it.
            return t.transformClass(
                    MixinEnvironment.getDefaultEnvironment(),
                    name.replace('/', '.'), classfileBuffer);
        } catch (Throwable e) {
            // The JVM swallows transformer exceptions silently.
            System.out.println("[Morrow] transform failed for " + name + ": " + e);
            return null;
        }
    }
}
