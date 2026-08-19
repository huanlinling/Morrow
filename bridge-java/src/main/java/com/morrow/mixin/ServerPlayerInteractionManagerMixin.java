package com.morrow.mixin;

import com.morrow.host.MorrowMod;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.server.network.ServerPlayerInteractionManager;
import net.minecraft.server.world.ServerWorld;
import net.minecraft.util.ActionResult;
import net.minecraft.util.Hand;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.math.BlockPos;
import net.minecraft.world.World;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(ServerPlayerInteractionManager.class)
public abstract class ServerPlayerInteractionManagerMixin {

    @Shadow @Final protected ServerPlayerEntity player;
    @Shadow protected ServerWorld world;

    @Inject(method = "tryBreakBlock", at = @At("RETURN"))
    private void morrow$onBreak(BlockPos pos, CallbackInfoReturnable<Boolean> cir) {
        if (!cir.getReturnValue()) return;
        MorrowMod.onBlockBreak(player.getName().getString(), blockName(world, pos));
    }

    @Inject(method = "interactBlock", at = @At("RETURN"))
    private void morrow$onPlace(ServerPlayerEntity player, World world, ItemStack stack, Hand hand,
                                BlockHitResult hit, CallbackInfoReturnable<ActionResult> cir) {
        if (!cir.getReturnValue().isAccepted()) return;
        MorrowMod.onBlockPlace(player.getName().getString(), blockName(world, hit.getBlockPos()));
    }

    private static String blockName(World world, BlockPos pos) {
        return Registries.BLOCK.getId(world.getBlockState(pos).getBlock()).toString();
    }
}
