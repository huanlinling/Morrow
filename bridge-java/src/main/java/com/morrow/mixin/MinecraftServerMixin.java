package com.morrow.mixin;

import com.morrow.host.MorrowMod;
import net.minecraft.server.MinecraftServer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Hooks into MinecraftServer to drive Morrow.
 *
 * <p>No Fabric API needed — these Mixin injections are applied by Fabric
 * Loader's Mixin processor, but Morrow itself doesn't use Fabric's mod API.
 */
@Mixin(MinecraftServer.class)
public abstract class MinecraftServerMixin {

    private static long tickCounter;

    /** Hook: world loaded → Morrow init. */
    @Inject(method = "loadWorld", at = @At("RETURN"))
    private void onLoadWorld(CallbackInfo ci) {
        MorrowMod.init((MinecraftServer) (Object) this);
    }

    /** Hook: each tick → Rust dispatch. */
    @Inject(method = "tick", at = @At("RETURN"))
    private void onTick(CallbackInfo ci) {
        if (tickCounter % 20 == 0) { // only dispatch every 20 ticks? No — every tick
            MorrowMod.onTick(tickCounter);
        }
        tickCounter++;
    }

    /** Hook: shutdown → Rust teardown. */
    @Inject(method = "shutdown", at = @At("HEAD"))
    private void onShutdown(CallbackInfo ci) {
        MorrowMod.onShutdown();
    }
}
