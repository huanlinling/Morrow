package com.morrow.agent;

import java.net.URL;
import java.net.URLClassLoader;

/**
 * Parent-first classloader EXCEPT for default-package classes, which it
 * tries to define itself before delegating.
 *
 * <p>Why this exists: the vanilla server jar is signed by Mojang and most
 * of its classes live in the DEFAULT package. {@code ServerApiVanilla}
 * must also be default-package (javac forbids a named package from
 * referencing the obfuscated default-package types), but defining it in
 * the game loader fails the per-package signer check — the game loader's
 * default package already holds signed classes. In this child loader the
 * default package contains only our unsigned classes, so the check
 * passes. Every named type (game classes, {@code ServerApi}) still
 * delegates parent-first, keeping a single class identity with the game
 * loader — the woven mixin code runs inside {@code MinecraftServer} and
 * casts to the game loader's {@code ServerApi}.
 */
public final class ChildFirstLoader extends URLClassLoader {

    public ChildFirstLoader(URL[] urls, ClassLoader parent) {
        super(urls, parent);
    }

    @Override
    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        synchronized (getClassLoadingLock(name)) {
            Class<?> c = findLoadedClass(name);
            if (c == null && name.indexOf('.') < 0) {
                // Default-package type: child-first. Our URLs contain only
                // ServerApiVanilla (the obfuscated compile stubs never ship
                // in the agent jar), so game classes like "alk" miss here
                // and fall through to the parent (game loader) below.
                try {
                    c = findClass(name);
                } catch (ClassNotFoundException ignore) {
                    // not ours — delegate
                }
            }
            if (c == null) {
                c = super.loadClass(name, false);
            }
            if (resolve) {
                resolveClass(c);
            }
            return c;
        }
    }
}
