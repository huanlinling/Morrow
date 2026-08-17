package com.morrow.agent;

import java.lang.instrument.Instrumentation;
import org.spongepowered.asm.launch.MixinBootstrap;
import org.spongepowered.asm.mixin.MixinEnvironment;
import org.spongepowered.asm.mixin.Mixins;

/**
 * Java Agent entry point — replaces Fabric Loader.
 *
 * <p>Usage: {@code java -javaagent:morrow.jar -jar server.jar}
 *
 * <p>The agent bootstraps Mixin standalone. Minecraft classes are
 * transformed as they load — no Fabric Loader or Fabric API needed.
 *
 * <p>The mixin config is registered by {@link AgentTransformer} at the
 * first game class load (see that class for why), not here.
 */
public class MorrowAgent {

    public static void premain(String args, Instrumentation inst) {
        System.out.println("[Morrow] Agent loaded.");

        // Bootstrap Mixin standalone — MixinServiceVanilla (discovered via
        // META-INF/services) claims the host role on a plain vanilla server;
        // it yields to Fabric/Forge when either launcher is present.
        MixinBootstrap.init();
        MixinEnvironment.getDefaultEnvironment()
                .setSide(MixinEnvironment.Side.SERVER);
        Mixins.getConfigs();

        System.out.println("[Morrow] Mixin initialized. Waiting for Minecraft...");

        // Register the class transformer after bootstrap completes —
        // registering first made it fire for Mixin's own bootstrap
        // classes and could trip JVM classloading circularity errors.
        // Minecraft classes load after main starts, still in time.
        AgentTransformer.instrumentation = inst;
        inst.addTransformer(new AgentTransformer(), true);
    }

    /** Attach at runtime (for debugging). */
    public static void agentmain(String args, Instrumentation inst) {
        premain(args, inst);
    }
}
