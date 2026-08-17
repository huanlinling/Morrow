package com.morrow.agent;

import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;

import org.spongepowered.asm.mixin.transformer.IMixinTransformer;

/**
 * Bridges the javaagent's {@link ClassFileTransformer} to Mixin's
 * transformer. Registered in premain before {@code MixinBootstrap.init()}
 * so Minecraft classes are transformed on first load; the mixin
 * transformer arrives later via {@link MixinServiceVanilla#offer}, so
 * until then this is a no-op.
 */
final class AgentTransformer implements ClassFileTransformer {

    /** Set by {@link MixinServiceVanilla#offer}. */
    static volatile IMixinTransformer transformer;

    /** Set by {@link MorrowAgent#premain}. */
    static volatile Instrumentation instrumentation;

    @Override
    public byte[] transform(ClassLoader loader, String name, Class<?> beingRedefined,
                            ProtectionDomain protectionDomain, byte[] classfileBuffer) {
        IMixinTransformer t = transformer;
        if (t == null) {
            return null;
        }
        // Never touch Mixin's own classes or our host classes.
        if (name.startsWith("org/spongepowered/") || name.startsWith("com/morrow/")) {
            return null;
        }
        // Mixin's transformer expects the dot-form name (LaunchWrapper
        // convention); the agent hands us slash-form.
        return t.transformClassBytes(name.replace('/', '.'), name, classfileBuffer);
    }
}
