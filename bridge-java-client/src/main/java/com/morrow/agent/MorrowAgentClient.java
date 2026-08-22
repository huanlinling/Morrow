package com.morrow.agent;

import java.lang.instrument.Instrumentation;

import org.spongepowered.asm.launch.MixinBootstrap;
import org.spongepowered.asm.mixin.MixinEnvironment;
import org.spongepowered.asm.mixin.Mixins;

/**
 * Client twin of {@link MorrowAgent}: attaches Mixin standalone to a
 * vanilla 1.20.1 CLIENT JVM. Same technique as the server agent, but
 * {@link MixinEnvironment.Side#CLIENT} and the client entry point as the
 * config-registration trigger.
 *
 * <p>Usage: {@code java -javaagent:morrow-client-1.20.1-1.0.1-agent.jar
 * --add-opens java.base/java.net=ALL-UNNAMED <client-launcher>}
 */
public class MorrowAgentClient {

    public static void premain(String args, Instrumentation inst) {
        System.out.println("[Morrow] Client agent loaded.");

        MixinBootstrap.init();
        MixinEnvironment.getDefaultEnvironment()
                .setSide(MixinEnvironment.Side.CLIENT);
        Mixins.getConfigs();

        AgentTransformer.configTrigger = "net/minecraft/client/main/Main";
        AgentTransformer.instrumentation = inst;
        inst.addTransformer(new AgentTransformer(), true);
    }

    /** Attach at runtime (for debugging). */
    public static void agentmain(String args, Instrumentation inst) {
        premain(args, inst);
    }
}
