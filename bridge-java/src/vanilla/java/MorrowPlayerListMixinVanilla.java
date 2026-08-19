import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Join/leave capture for the vanilla production jar (obfuscated twin of
 * PlayerManagerMixin). Targets alk (PlayerList):
 *   placeNewPlayer(Connection, ServerPlayer) -> a(sd, aig)
 *   remove(ServerPlayer)                     -> c(aig)
 * Player name via beb.Z() (Nameable.getName), same as ServerApiVanilla.
 * Default package: javac forbids named packages referencing these
 * obfuscated default-package types. Bump names per MC version.
 */
@Mixin(targets = "alk")
public abstract class MorrowPlayerListMixinVanilla {

    @Inject(method = "a(Lsd;Laig;)V", at = @At("RETURN"))
    private void morrow$onJoin(sd conn, aig player, CallbackInfo ci) {
        com.morrow.host.MorrowMod.onPlayerJoin(((beb) player).Z().getString());
    }

    @Inject(method = "c(Laig;)V", at = @At("HEAD"))
    private void morrow$onLeave(aig player, CallbackInfo ci) {
        com.morrow.host.MorrowMod.onPlayerLeave(((beb) player).Z().getString());
    }
}
