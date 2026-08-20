package com.morrow.host;

/**
 * Compile-time-only stand-in for the real MorrowMod (which lives in the
 * main source set, compiled at 21 with preview flags — javac at release
 * 17 refuses to read preview-flagged classes). Signatures match
 * MorrowMod exactly; the real class wins at runtime because the stubs
 * never enter the agent jar.
 */
public final class MorrowMod {
    public static void init(ServerApi gameApi) {}
    public static void onTick(long tick) {}
    public static void onPlayerJoin(String name) {}
    public static void onPlayerLeave(String name) {}
    public static void onChat(String player, String msg) {}
    public static void onBlockBreak(String player, String block) {}
    public static void onBlockPlace(String player, String block) {}
    public static void onPlayerDeath(String player) {}
    public static void flushBatch() {}
    public static void onShutdown() {}
}
