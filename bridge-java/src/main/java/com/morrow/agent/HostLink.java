package com.morrow.agent;

import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;

/**
 * Makes the host classes (com.morrow.host.*) loadable from the game
 * classloader. See {@link AgentTransformer} for why the vanilla bundler
 * would otherwise keep them invisible. The agent jar itself is added as a
 * URL of the game loader; the host package then resolves from game code.
 */
final class HostLink {

    private static volatile boolean installed;

    static void install(ClassLoader gameLoader) {
        if (installed || !(gameLoader instanceof URLClassLoader urlLoader)) {
            return;
        }
        try {
            Method addURL = URLClassLoader.class.getDeclaredMethod("addURL", URL.class);
            addURL.setAccessible(true);
            addURL.invoke(urlLoader, HostLink.class.getProtectionDomain()
                    .getCodeSource().getLocation());
            installed = true;
        } catch (Throwable e) {
            // Missing --add-opens java.base/java.net=ALL-UNNAMED shows up here.
            System.out.println("[Morrow] HostLink failed: " + e);
        }
    }
}
