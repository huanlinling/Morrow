package com.morrow.agent;

import java.io.IOException;
import java.io.InputStream;
import java.lang.instrument.Instrumentation;
import java.net.URL;
import java.util.Collection;
import java.util.Collections;
import java.util.List;

import org.objectweb.asm.ClassReader;
import org.objectweb.asm.tree.ClassNode;
import org.spongepowered.asm.launch.platform.container.ContainerHandleVirtual;
import org.spongepowered.asm.launch.platform.container.IContainerHandle;
import org.spongepowered.asm.logging.ILogger;
import org.spongepowered.asm.logging.LoggerAdapterConsole;
import org.spongepowered.asm.mixin.MixinEnvironment;
import org.spongepowered.asm.mixin.transformer.IMixinTransformer;
import org.spongepowered.asm.mixin.transformer.IMixinTransformerFactory;
import org.spongepowered.asm.service.IClassBytecodeProvider;
import org.spongepowered.asm.service.IClassProvider;
import org.spongepowered.asm.service.IClassTracker;
import org.spongepowered.asm.service.IMixinAuditTrail;
import org.spongepowered.asm.service.IMixinInternal;
import org.spongepowered.asm.service.IMixinService;
import org.spongepowered.asm.service.ITransformer;
import org.spongepowered.asm.service.ITransformerProvider;
import org.spongepowered.asm.util.ReEntranceLock;

/**
 * Mixin host service for a plain vanilla server — no Fabric Loader, no
 * Forge ModLauncher. Same shape as Fabric's {@code MixinServiceKnot}:
 * one class implements {@link IMixinService} plus the provider/tracker
 * roles, transformation is driven by the platform itself (here, the
 * javaagent's {@link Instrumentation} via {@link AgentTransformer}),
 * so {@link #getTransformers()} stays empty.
 */
public class MixinServiceVanilla implements IMixinService, IClassProvider,
        IClassBytecodeProvider, ITransformerProvider, IClassTracker {

    private static final ReEntranceLock LOCK = new ReEntranceLock(1);

    @Override
    public String getName() {
        return "Vanilla";
    }

    @Override
    public boolean isValid() {
        return true;
    }

    @Override
    public void prepare() {
        // Obfuscation context is set in MorrowAgent.premain after
        // MixinBootstrap.init() — this method runs during MixinBootstrap's
        // static init, before the environment exists.
    }

    @Override
    public MixinEnvironment.Phase getInitialPhase() {
        return MixinEnvironment.Phase.PREINIT;
    }

    @Override
    public void offer(IMixinInternal internal) {
        if (internal instanceof IMixinTransformerFactory factory) {
            AgentTransformer.transformer = factory.createTransformer();
        }
    }

    @Override
    public void init() {
    }

    @Override
    public void beginPhase() {
    }

    @Override
    public void checkEnv(Object bootSource) {
    }

    @Override
    public ReEntranceLock getReEntranceLock() {
        return LOCK;
    }

    // ── IClassProvider ─────────────────────────────

    @Override
    public IClassProvider getClassProvider() {
        return this;
    }

    @Override
    public URL[] getClassPath() {
        // Mixin scans the provider classpath to locate mixin classes
        // (MixinInfo.loadMixinClass) — an empty array made every mixin
        // "not found" even though Class.forName worked. The agent jar is
        // the only path we need: all Morrow mixin classes live there.
        try {
            return new URL[] { MixinServiceVanilla.class
                    .getProtectionDomain().getCodeSource().getLocation() };
        } catch (Exception e) {
            return new URL[0];
        }
    }

    @Override
    public Class<?> findClass(String name) throws ClassNotFoundException {
        return findClass(name, true);
    }

    @Override
    public Class<?> findClass(String name, boolean initialize) throws ClassNotFoundException {
        return Class.forName(name, initialize, classLoader());
    }

    @Override
    public Class<?> findAgentClass(String name, boolean initialize) throws ClassNotFoundException {
        return Class.forName(name, initialize, MixinServiceVanilla.class.getClassLoader());
    }

    // ── IClassBytecodeProvider ─────────────────────

    @Override
    public IClassBytecodeProvider getBytecodeProvider() {
        return this;
    }

    @Override
    public ClassNode getClassNode(String name) throws ClassNotFoundException, IOException {
        return getClassNode(name, true);
    }

    @Override
    public ClassNode getClassNode(String name, boolean runTransformers)
            throws ClassNotFoundException, IOException {
        // Class names arrive in dot form; resource paths are slash form.
        // Not converting here made every mixin class "not found" during
        // config preparation (the same silent failure behind targets=[]).
        // Lookup order matters: the bundler loader for game classes, the
        // system loader for the agent jar's own mixin classes.
        String resource = name.replace('.', '/') + ".class";
        ClassLoader gameLoader = AgentTransformer.currentLoader;
        InputStream in = gameLoader != null
                ? gameLoader.getResourceAsStream(resource)
                : null;
        if (in == null) {
            in = ClassLoader.getSystemResourceAsStream(resource);
        }
        if (in == null) {
            throw new ClassNotFoundException(name);
        }
        final InputStream stream = in;
        try (stream) {
            ClassNode cn = new ClassNode();
            new ClassReader(stream).accept(cn, runTransformers ? ClassReader.EXPAND_FRAMES : 0);
            return cn;
        }
    }

    // ── ITransformerProvider ───────────────────────
    // Transformation is driven by AgentTransformer via Instrumentation,
    // not by the framework's transformer collection (see class doc).

    @Override
    public ITransformerProvider getTransformerProvider() {
        return this;
    }

    @Override
    public Collection<ITransformer> getTransformers() {
        return Collections.emptyList();
    }

    @Override
    public Collection<ITransformer> getDelegatedTransformers() {
        return Collections.emptyList();
    }

    @Override
    public void addTransformerExclusion(String name) {
    }

    // ── IClassTracker ──────────────────────────────

    @Override
    public IClassTracker getClassTracker() {
        return this;
    }

    @Override
    public void registerInvalidClass(String name) {
    }

    @Override
    public boolean isClassLoaded(String name) {
        Instrumentation inst = AgentTransformer.instrumentation;
        if (inst == null) {
            return false;
        }
        for (Class<?> c : inst.getAllLoadedClasses()) {
            if (c.getName().equals(name)) {
                return true;
            }
        }
        return false;
    }

    @Override
    public String getClassRestrictions(String name) {
        return "";
    }

    // ── Misc IMixinService ─────────────────────────

    @Override
    public IMixinAuditTrail getAuditTrail() {
        return new IMixinAuditTrail() {
            @Override
            public void onApply(String targetName, String mixinName) {
                System.out.println("[Morrow] mixin applied: " + targetName + " <- " + mixinName);
            }

            @Override
            public void onPostProcess(String targetName) {
            }

            @Override
            public void onGenerate(String targetName, String generatorName) {
            }
        };
    }

    @Override
    public Collection<String> getPlatformAgents() {
        return Collections.emptyList();
    }

    @Override
    public IContainerHandle getPrimaryContainer() {
        return new ContainerHandleVirtual("morrow-agent")
                .setAttribute("mixin.env.side", "SERVER");
    }

    @Override
    public Collection<IContainerHandle> getMixinContainers() {
        return List.of(getPrimaryContainer());
    }

    @Override
    public InputStream getResourceAsStream(String name) {
        return ClassLoader.getSystemResourceAsStream(name);
    }

    @Override
    public String getSideName() {
        return "SERVER";
    }

    @Override
    public MixinEnvironment.CompatibilityLevel getMinCompatibilityLevel() {
        return MixinEnvironment.CompatibilityLevel.JAVA_17;
    }

    @Override
    public MixinEnvironment.CompatibilityLevel getMaxCompatibilityLevel() {
        return MixinEnvironment.CompatibilityLevel.MAX_SUPPORTED;
    }

    @Override
    public ILogger getLogger(String name) {
        LoggerAdapterConsole logger = new LoggerAdapterConsole(name);
        // Debug output is off by default in the console adapter; route it
        // to stdout so -Dmixin.debug=true actually reaches the server log.
        logger.setDebugStream(System.out);
        return logger;
    }

    /** App classloader at transform time; system loader as fallback. */
    private static ClassLoader classLoader() {
        ClassLoader cl = AgentTransformer.currentLoader;
        if (cl == null) {
            cl = Thread.currentThread().getContextClassLoader();
        }
        return cl != null ? cl : ClassLoader.getSystemClassLoader();
    }
}
