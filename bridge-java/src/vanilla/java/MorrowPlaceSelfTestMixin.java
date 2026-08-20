import java.util.function.BooleanSupplier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Place-event self-test: drives a real {@code useItemOn} call on the
 * server thread once the first player is online, bypassing vanilla's
 * packet-layer validation (which rejected the headless fake client) but
 * covering Morrow's whole pipeline: interactBlock → place handler →
 * EventBuffer → Rust dispatch → mod callbacks.
 *
 * <p>Off by default; enable with {@code -Dmorrow.selftest.place=true}.
 * One-shot per JVM. Prints the InteractionResult for diagnosis.
 *
 * <p>Obfuscated references (1.20.1, javap-verified):
 *   ServerPlayer.gameMode            aig.e        (public final aih)
 *   MinecraftServer.getLevel(OVERWORLD) a(acp) -> aif
 *   BuiltInRegistries.BLOCK          jb.f         (gz registry)
 *   Registry.get(ResourceLocation)   gz.a(acq) -> T
 *   Direction.UP                     ha.b
 *   ItemStack(ItemLike)              cfz(cml)
 *   BlockHitResult                   eee(eei, ha, gu, boolean)
 *   InteractionHand.MAIN_HAND        bdw.a
 */
@Mixin(targets = "net.minecraft.server.MinecraftServer")
public abstract class MorrowPlaceSelfTestMixin {

    private static final boolean ENABLED =
            Boolean.getBoolean("morrow.selftest.place");
    private static boolean ran;

    @Inject(method = "a(Ljava/util/function/BooleanSupplier;)V", at = @At("HEAD"))
    private void morrow$selfTestPlace(BooleanSupplier supplier, CallbackInfo ci) {
        if (!ENABLED || ran) return;
        net.minecraft.server.MinecraftServer self =
                (net.minecraft.server.MinecraftServer) (Object) this;
        java.util.List<?> players = self.ac().t();
        if (players.isEmpty()) return;
        ran = true; // attempt once, with a player present

        try {
            aig player = (aig) players.get(0);
            aih gm = player.e;
            aif level = (aif) self.a(cmm.h);            // OVERWORLD
            cpn dirt = jb.f.a(new acq("minecraft:dirt"));
            cfz stack = new cfz((cml) dirt);
            gu target = new gu(-20, 59, -247);
            eee hit = new eee(new eei(-19.5, 60.0, -246.5), ha.b, target, false);
            bdx result = gm.a(player, level, stack, bdw.a, hit); // useItemOn
            System.out.println("[Morrow] place self-test: " + result
                    + " consumed=" + result.a());
        } catch (Throwable t) {
            System.out.println("[Morrow] place self-test FAILED: " + t);
            t.printStackTrace();
        }
    }
}
