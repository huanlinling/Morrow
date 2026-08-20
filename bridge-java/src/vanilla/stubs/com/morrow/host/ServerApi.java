package com.morrow.host;

/**
 * Compile-time-only stand-in for the host ServerApi interface, mirroring
 * the real one in src/main/java (same signatures, same vtable order).
 * See MorrowMod stub for why the mixins compile against stubs.
 */
public interface ServerApi {
    int getPlayerCount();
    void sendMessage(long ptr, int len);
    int getPlayerList(long buf, int cap);
    void executeCommand(long ptr, int len);
    long getWorldTime();
    int getWorldSnapshot(long buf, int cap);
}
