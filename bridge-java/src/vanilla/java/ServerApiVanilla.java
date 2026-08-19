import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;

/**
 * Host API against Mojang's production jar — the ONLY obfuscated names
 * that survive in 1.20.1's server jar are net.minecraft.{server,util,...}
 * classes; everything else (Text, PlayerList, Component, ...) is renamed
 * to letters. Names below are ground-truthed against the extracted
 * 1.20.1 jar (server_mappings.txt + javap):
 *
 * <pre>
 *   MinecraftServer.getPlayerList()          ac()  -> alk (PlayerList)
 *   MinecraftServer.getCommands()            aC()  -> dt  (Commands)
 *   MinecraftServer.createCommandSourceStack aD()  -> ds  (CommandSourceStack)
 *   MinecraftServer.getLevel(ResourceKey)    a(acp) -> aif (ServerLevel)
 *   Level.OVERWORLD                          cmm.h
 *   Level.getDayTime                         cmm.W()
 *   PlayerList.getPlayers                    alk.t()        -> List<aig>
 *   PlayerList.broadcastSystemMessage        alk.a(sw, bool)
 *   Nameable.getName                         beb.Z()        -> sw
 *   Component.literal(String)                sw.b(String)   -> tj (MutableComponent)
 *   Component.getString                      sw.getString() (survives)
 *   Commands.performPrefixedCommand          dt.a(ds, String)
 * </pre>
 *
 * Bump these when the target Minecraft version changes (same maintenance
 * contract as MinecraftServerMixinVanilla). Compiles against stub classes
 * in src/vanilla/stubs (excluded from the agent jar) whose descriptors
 * match the real jar exactly.
 */
public class ServerApiVanilla implements com.morrow.host.ServerApi {

    private final net.minecraft.server.MinecraftServer server;

    public ServerApiVanilla(Object server) {
        this.server = (net.minecraft.server.MinecraftServer) server;
    }

    @Override
    public int getPlayerCount() {
        return server.ac().t().size();
    }

    @Override
    public void sendMessage(long ptr, int len) {
        byte[] b = MemorySegment.ofAddress(ptr).reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
        server.ac().a(sw.b("[Morrow] " + new String(b, StandardCharsets.UTF_8)), false);
    }

    @Override
    public int getPlayerList(long buf, int cap) {
        String names = joinNames();
        byte[] b = names.getBytes(StandardCharsets.UTF_8);
        int n = Math.min(b.length, cap);
        MemorySegment.ofAddress(buf).reinterpret(n).copyFrom(MemorySegment.ofArray(b));
        return n;
    }

    @Override
    public void executeCommand(long ptr, int len) {
        byte[] b = MemorySegment.ofAddress(ptr).reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
        server.aC().a(server.aD(), new String(b, StandardCharsets.UTF_8));
    }

    @Override
    public long getWorldTime() {
        return ((cmm) server.a(cmm.h)).W();
    }

    @Override
    public int getWorldSnapshot(long bufPtr, int bufCap) {
        var players = server.ac().t();
        var buf = MemorySegment.ofAddress(bufPtr).reinterpret(bufCap);
        int pos = 0;
        // u32: player count
        buf.set(ValueLayout.JAVA_INT_UNALIGNED, pos, players.size()); pos += 4;
        // u64: world time
        buf.set(ValueLayout.JAVA_LONG_UNALIGNED, pos, getWorldTime()); pos += 8;
        for (var p : players) {
            byte[] name = ((beb) p).Z().getString().getBytes(StandardCharsets.UTF_8);
            if (pos + 2 + name.length > bufCap) break;
            buf.set(ValueLayout.JAVA_SHORT_UNALIGNED, pos, (short) name.length); pos += 2;
            for (byte c : name) { buf.set(ValueLayout.JAVA_BYTE, pos++, c); }
        }
        return pos;
    }

    private String joinNames() {
        var names = new java.util.ArrayList<String>();
        for (var p : server.ac().t()) {
            names.add(((beb) p).Z().getString());
        }
        return String.join(",", names);
    }
}
