package com.morrow.agent;

import org.spongepowered.asm.service.IMixinServiceBootstrap;
import org.spongepowered.asm.service.ServiceNotAvailableError;

/**
 * Registers {@link MixinServiceVanilla} with Mixin's service discovery
 * (META-INF/services). Only claims the host role when no other mod
 * platform is in charge — in Fabric dev mode (loom runServer) the
 * loader's Knot service must win, and on Forge the ModLauncher service
 * must win.
 */
public class MixinServiceVanillaBootstrap implements IMixinServiceBootstrap {

    @Override
    public String getName() {
        return "Vanilla";
    }

    @Override
    public String getServiceClassName() {
        return "com.morrow.agent.MixinServiceVanilla";
    }

    @Override
    public void bootstrap() {
        try {
            Class.forName("net.fabricmc.loader.api.FabricLoader", false,
                    MixinServiceVanillaBootstrap.class.getClassLoader());
            throw new ServiceNotAvailableError("Fabric Loader is present");
        } catch (ClassNotFoundException e) {
            // No Fabric (or Quilt) — proceed.
        }
        try {
            Class.forName("cpw.mods.modlauncher.Launcher", false,
                    MixinServiceVanillaBootstrap.class.getClassLoader());
            throw new ServiceNotAvailableError("ModLauncher (Forge) is present");
        } catch (ClassNotFoundException e) {
            // No Forge — this is a plain vanilla server. Claim the role.
        }
    }
}
