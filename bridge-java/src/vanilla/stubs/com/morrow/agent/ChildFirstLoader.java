package com.morrow.agent;

import java.net.URL;
import java.net.URLClassLoader;

/**
 * Compile-time-only stand-in for the real ChildFirstLoader, which the
 * named-package MinecraftServerMixinVanilla instantiates to reach the
 * default-package ServerApiVanilla. See the MorrowMod stub for why.
 */
public class ChildFirstLoader extends URLClassLoader {
    public ChildFirstLoader(URL[] urls, ClassLoader parent) {
        super(urls, parent);
    }
}
