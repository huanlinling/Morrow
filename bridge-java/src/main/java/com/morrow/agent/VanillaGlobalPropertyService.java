package com.morrow.agent;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

import org.spongepowered.asm.service.IGlobalPropertyService;
import org.spongepowered.asm.service.IPropertyKey;

/**
 * In-memory {@link IGlobalPropertyService} for the vanilla agent mode.
 * Mixin's own service file only lists LaunchWrapper/ModLauncher stores;
 * this plain map takes their place. Shipped only in the agent jar (see
 * build.gradle {@code agentJar} / {@code resources-agent}) so Fabric dev
 * mode keeps using the loader's own store.
 */
public class VanillaGlobalPropertyService implements IGlobalPropertyService {

    /** Key equality is by name, so re-resolving the same name hits the
     *  same entry regardless of key object identity. */
    private record PropertyKey(String name) implements IPropertyKey {
    }

    private final Map<IPropertyKey, Object> properties = new ConcurrentHashMap<>();

    @Override
    public IPropertyKey resolveKey(String name) {
        return new PropertyKey(name);
    }

    @Override
    @SuppressWarnings("unchecked")
    public <T> T getProperty(IPropertyKey key) {
        return (T) properties.get(key);
    }

    @Override
    public void setProperty(IPropertyKey key, Object value) {
        properties.put(key, value);
    }

    @Override
    public <T> T getProperty(IPropertyKey key, T defaultValue) {
        T value = getProperty(key);
        return value != null ? value : defaultValue;
    }

    @Override
    public String getPropertyString(IPropertyKey key, String defaultValue) {
        Object value = getProperty(key);
        return value != null ? String.valueOf(value) : defaultValue;
    }
}
