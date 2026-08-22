package com.morrow.client.mixin;

import com.morrow.client.ClientApi;
import com.morrow.host.MorrowMod;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Hooks the obfuscated client tick. {@code MinecraftClient} (mojmap
 * {@code net.minecraft.client.Minecraft}) is obfuscated to the
 * default-package class {@code enn}; its {@code tick()} is {@code s()V}
 * (intermediary {@code method_1574}).
 *
 * <p>ponytail: fires on the per-frame tick (~60 Hz), not the 20 TPS game
 * tick — the TPS gate lives elsewhere in MinecraftClient. Fine for proving
 * injection + load + dispatch; wire the 20 TPS source when real client
 * events land.
 */
@Mixin(targets = "enn")
public abstract class MinecraftClientMixin {

    private static long tickCounter;
    private static boolean initialized;

    @Inject(method = "s()V", at = @At("HEAD"))
    private void onClientTick(CallbackInfo ci) {
        if (!initialized) {
            initialized = true;
            MorrowMod.init(new ClientApi());
        }
        MorrowMod.onTick(tickCounter++);
        MorrowMod.flushBatch();
    }
}
