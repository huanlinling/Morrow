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
 * <p>The agent bootstraps Mixin directly. Minecraft classes are
 * transformed as they load — no Fabric Loader or Fabric API needed.
 */
public class MorrowAgent {

    public static void premain(String args, Instrumentation inst) {
        System.out.println("[Morrow] Agent loaded.");

        // Bootstrap Mixin standalone
        MixinBootstrap.init();
        Mixins.addConfiguration("morrow.mixins.json");
        MixinEnvironment.getDefaultEnvironment()
                .setSide(MixinEnvironment.Side.SERVER);

        System.out.println("[Morrow] Mixin initialized. Waiting for Minecraft...");
    }

    /** Attach at runtime (for debugging). */
    public static void agentmain(String args, Instrumentation inst) {
        premain(args, inst);
    }
}
