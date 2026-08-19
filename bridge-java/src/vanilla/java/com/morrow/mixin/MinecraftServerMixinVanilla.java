package com.morrow.mixin;

import com.morrow.host.ServerApi;
import com.morrow.agent.ChildFirstLoader;

import net.minecraft.server.MinecraftServer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.net.URL;

import com.morrow.host.MorrowMod;

/**
 * Agent-mode twin of {@link MinecraftServerMixin}: targets the
 * OBSCURFATED production names of the vanilla server jar.
 *
 * <p>Dev mode (Fabric/loom) runs {@link MinecraftServerMixin} whose
 * {@code method = "loadWorld"} etc. are yarn names, remapped by the
 * loader. The standalone agent sees Mojang's production jar where those
 * members are obfuscated — no loader remap layer exists there.
 *
 * <p>Obfuscated names are from Mojang's {@code server_mappings.txt}
 * for MC 1.20.1:
 * <pre>
 *   loadWorld  (yarn) / loadLevel  (official) -> n_()
 *   tick       (yarn) / tickServer (official) -> a(Ljava/util/function/BooleanSupplier;)V
 *   shutdown   (yarn) / stopServer (official) -> t()
 * </pre>
 * Bump these when the target Minecraft version changes.
 */
@Mixin(MinecraftServer.class)
public abstract class MinecraftServerMixinVanilla {

    private static long tickCounter;

    @Inject(method = "n_()V", at = @At("RETURN"))
    private void onLoadWorld(CallbackInfo ci) {
        MorrowMod.init(newApi(this));
    }

    /**
     * The obfuscated-side adapter lives in the DEFAULT package (javac
     * forbids named packages from referencing default-package classes,
     * and the obfuscated game classes are all default-package), so this
     * named-package mixin reaches it via reflection. The game loader
     * cannot define it either — the default package is signed by Mojang —
     * so a {@link ChildFirstLoader} defines it in a child of the game
     * loader instead.
     */
    private static ServerApi newApi(Object server) {
        try {
            URL jar = MorrowMod.class.getProtectionDomain()
                    .getCodeSource().getLocation();
            ClassLoader cl = new ChildFirstLoader(new URL[]{jar},
                    server.getClass().getClassLoader());
            return (ServerApi) cl.loadClass("ServerApiVanilla")
                    .getConstructor(Object.class).newInstance(server);
        } catch (Throwable e) {
            throw new RuntimeException("ServerApiVanilla unavailable", e);
        }
    }

    @Inject(method = "a(Ljava/util/function/BooleanSupplier;)V", at = @At("HEAD"))
    private void onTickStart(CallbackInfo ci) {
        MorrowMod.onTick(tickCounter++);
    }

    @Inject(method = "a(Ljava/util/function/BooleanSupplier;)V", at = @At("RETURN"))
    private void onTickEnd(CallbackInfo ci) {
        MorrowMod.flushBatch(); // 1 FFM call per tick max
    }

    @Inject(method = "t()V", at = @At("HEAD"))
    private void onShutdown(CallbackInfo ci) {
        MorrowMod.onShutdown();
    }
}
