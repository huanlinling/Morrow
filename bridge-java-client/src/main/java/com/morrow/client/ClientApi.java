package com.morrow.client;

import com.morrow.host.ServerApi;

/**
 * Client-side host API for the skeleton: every upcall is a no-op. The
 * server's vtable contract (getPlayerCount, sendMessage, ...) has no
 * client meaning yet — real client semantics (send chat to the server,
 * read the local player's world) land in a later pass.
 */
public class ClientApi implements ServerApi {

    @Override public int getPlayerCount() { return 0; }

    @Override public void sendMessage(long ptr, int len) { }

    @Override public int getPlayerList(long buf, int cap) { return 0; }

    @Override public void executeCommand(long ptr, int len) { }

    @Override public long getWorldTime() { return 0L; }

    @Override public int getWorldSnapshot(long buf, int cap) { return 0; }
}
