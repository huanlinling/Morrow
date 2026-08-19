import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Player death capture for the vanilla production jar. Targets aig
 * (ServerPlayer): die(DamageSource) -> a(ben).
 */
@Mixin(targets = "aig")
public abstract class MorrowDeathMixinVanilla {

    @Inject(method = "a(Lben;)V", at = @At("HEAD"))
    private void morrow$onDeath(ben source, CallbackInfo ci) {
        com.morrow.host.MorrowMod.onPlayerDeath(((beb) (Object) this).Z().getString());
    }
}
