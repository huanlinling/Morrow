package com.morrow.mixin;

import com.morrow.host.MorrowMod;
import net.minecraft.network.ClientConnection;
import net.minecraft.server.PlayerManager;
import net.minecraft.server.network.ServerPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(PlayerManager.class)
public abstract class PlayerManagerMixin {

    @Inject(method = "onPlayerConnect", at = @At("RETURN"))
    private void morrow$onJoin(ClientConnection conn, ServerPlayerEntity player, CallbackInfo ci) {
        MorrowMod.onPlayerJoin(player.getName().getString());
    }

    @Inject(method = "remove", at = @At("HEAD"))
    private void morrow$onLeave(ServerPlayerEntity player, CallbackInfo ci) {
        MorrowMod.onPlayerLeave(player.getName().getString());
    }
}
