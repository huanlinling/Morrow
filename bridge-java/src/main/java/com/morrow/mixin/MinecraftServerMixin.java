package com.morrow.mixin;

import com.morrow.host.MorrowMod;
import com.morrow.host.ServerApiFabric;
import net.minecraft.server.MinecraftServer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(MinecraftServer.class)
public abstract class MinecraftServerMixin {

    private static long tickCounter;

    @Inject(method = "loadWorld", at = @At("RETURN"))
    private void onLoadWorld(CallbackInfo ci) {
        MorrowMod.init(new ServerApiFabric((MinecraftServer) (Object) this));
    }

    @Inject(method = "tick", at = @At("HEAD"))
    private void onTickStart(CallbackInfo ci) {
        MorrowMod.onTick(tickCounter++);
    }

    @Inject(method = "tick", at = @At("RETURN"))
    private void onTickEnd(CallbackInfo ci) {
        MorrowMod.flushBatch(); // 1 FFM call per tick max
    }

    @Inject(method = "shutdown", at = @At("HEAD"))
    private void onShutdown(CallbackInfo ci) {
        MorrowMod.onShutdown();
    }
}
