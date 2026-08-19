import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Chat capture for the vanilla production jar. Targets aiy
 * (ServerGamePacketListenerImpl):
 *   handleChat(ServerboundChatPacket) -> a(zi);  zi.a() = message()
 *   field b = player (aig)
 */
@Mixin(targets = "aiy")
public abstract class MorrowChatMixinVanilla {

    @Inject(method = "a(Lzi;)V", at = @At("HEAD"))
    private void morrow$onChat(zi packet, CallbackInfo ci) {
        aig player = ((aiy) (Object) this).b;
        com.morrow.host.MorrowMod.onChat(((beb) player).Z().getString(), packet.a());
    }
}
