import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Coerce;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Block break/place capture for the vanilla production jar. Targets aih
 * (ServerPlayerGameMode):
 *   destroyBlock(BlockPos) -> a(gu)Z        (field d = player, c = level)
 *   useItemOn(...)         -> a(aig,cmm,cfz,bdw,eee)Lbdx;
 *
 * All handler params are {@code @Coerce Object}: the obfuscated param
 * types (gu, cmm, ...) are not visible to every loader that reads this
 * class's method signatures (the mixin transform-time loader lacks game
 * classes and fails eager resolution — see the aih CNF history), and
 * Object is assignable from any of them. Casts inside the body resolve
 * lazily at execution time, where the game loader is present.
 * Block name: jb.f (BuiltInRegistries.BLOCK).b(block) -> acq; toString
 * gives "minecraft:stone".
 */
@Mixin(targets = "aih")
public abstract class MorrowGameModeMixinVanilla {

    @Inject(method = "a(Lgu;)Z", at = @At("RETURN"))
    private void morrow$onBreak(@Coerce Object pos, CallbackInfoReturnable<Boolean> cir) {
        if (!cir.getReturnValue()) return;
        aih self = (aih) (Object) this;
        com.morrow.host.MorrowMod.onBlockBreak(name(self.d), block(self.c, (gu) pos));
    }

    @Inject(method = "a(Laig;Lcmm;Lcfz;Lbdw;Leee;)Lbdx;", at = @At("RETURN"))
    private void morrow$onPlace(@Coerce Object player, @Coerce Object level, @Coerce Object stack,
                                @Coerce Object hand, @Coerce Object hit, CallbackInfoReturnable cir) {
        if (!((bdx) cir.getReturnValue()).a()) return; // consumesAction
        com.morrow.host.MorrowMod.onBlockPlace(name((aig) player), block((cmm) level, ((eee) hit).a()));
    }

    private static String name(Object p) {
        return ((beb) p).Z().getString();
    }

    private static String block(Object level, Object pos) {
        return jb.f.b(((cmm) level).a_((gu) pos).b()).toString();
    }
}
