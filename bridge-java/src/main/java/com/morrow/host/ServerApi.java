package com.morrow.host;

/**
 * Game-facing half of the host API — the upcall targets the native
 * runtime invokes. Implemented per environment: {@link ServerApiFabric}
 * (yarn names, dev) and ServerApiVanilla (obfuscated names, agent mode).
 * Deliberately free of Minecraft types so MorrowMod stays loadable in a
 * fully obfuscated production jar; the adapters do the game access.
 */
public interface ServerApi {

    int getPlayerCount();

    /** UTF-8 bytes at {@code ptr}; broadcast as "[Morrow] …" to all players. */
    void sendMessage(long ptr, int len);

    /** Fill {@code buf} with comma-joined player names, return bytes written. */
    int getPlayerList(long buf, int cap);

    /** Execute a command as the server console. */
    void executeCommand(long ptr, int len);

    long getWorldTime();

    /** Fill {@code buf} with the world snapshot (players + time). */
    int getWorldSnapshot(long buf, int cap);
}
