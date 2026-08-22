package com.morrow.agent;

/**
 * Service bootstrap for the client: discovers {@link MixinServiceClient}.
 * Same Fabric/Forge-absence guard as the server bootstrap.
 */
public class MixinServiceClientBootstrap extends MixinServiceVanillaBootstrap {

    @Override
    public String getName() {
        return "VanillaClient";
    }

    @Override
    public String getServiceClassName() {
        return "com.morrow.agent.MixinServiceClient";
    }
}
