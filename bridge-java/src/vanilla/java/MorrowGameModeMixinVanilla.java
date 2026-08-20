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
 * Break is captured in two injections: at HEAD the block state still
 * exists, so the name is stashed in a ThreadLocal; at RETURN the
 * destruction has already run (reading the state there always yields
 * air) but the return value tells us whether the break succeeded.
 *
 * All handler params are {@code @Coerce Object}: the obfuscated param
 * types (gu, cmm, ...) are not visible to every loader that reads this
 * class's method signatures (the mixin transform-time loader lacks game
 * classes and fails eager resolution), and Object is assignable from
 * any of them. Casts inside the body resolve lazily at execution time,
 * where the game loader is present.
 * Block name: jb.f (BuiltInRegistries.BLOCK).b(block) -> acq; toString
 * gives "minecraft:stone".
 */
@Mixin(targets = "aih")
public abstract class MorrowGameModeMixinVanilla {

    private static final ThreadLocal<String> BREAK_BLOCK = new ThreadLocal<>();

    // CallbackInfoReturnable even at HEAD: the target returns boolean.
    @Inject(method = "a(Lgu;)Z", at = @At("HEAD"))
    private void morrow$onBreakHead(@Coerce Object pos, CallbackInfoReturnable<Boolean> cir) {
        aih self = (aih) (Object) this;
        BREAK_BLOCK.set(block(self.c, (gu) pos));
    }

    @Inject(method = "a(Lgu;)Z", at = @At("RETURN"))
    private void morrow$onBreak(@Coerce Object pos, CallbackInfoReturnable<Boolean> cir) {
        String block = BREAK_BLOCK.get();
        BREAK_BLOCK.remove();
        if (!cir.getReturnValue() || block == null) return;
        aih self = (aih) (Object) this;
        com.morrow.host.MorrowMod.onBlockBreak(name(self.d), block);
    }

    @Inject(method = "a(Laig;Lcmm;Lcfz;Lbdw;Leee;)Lbdx;", at = @At("RETURN"))
    private void morrow$onPlace(@Coerce Object player, @Coerce Object level, @Coerce Object stack,
                                @Coerce Object hand, @Coerce Object hit, CallbackInfoReturnable cir) {
        if (!((bdx) cir.getReturnValue()).a()) return; // consumesAction
        // Report the PLACED block (clicked pos + face), not the clicked one.
        com.morrow.host.MorrowMod.onBlockPlace(name((aig) player),
                block((cmm) level, ((eee) hit).a().a(((eee) hit).b())));
    }

    private static String name(Object p) {
        return ((beb) p).Z().getString();
    }

    private static String block(Object level, Object pos) {
        return jb.f.b(((cmm) level).a_((gu) pos).b()).toString();
    }
}
