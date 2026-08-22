package com.morrow.agent;

import org.spongepowered.asm.launch.platform.container.ContainerHandleVirtual;
import org.spongepowered.asm.launch.platform.container.IContainerHandle;

/**
 * Client-side mixin service: {@link MixinServiceVanilla} with the side
 * reported as CLIENT so client-only mixins resolve.
 */
public class MixinServiceClient extends MixinServiceVanilla {

    @Override
    public IContainerHandle getPrimaryContainer() {
        return new ContainerHandleVirtual("morrow-agent")
                .setAttribute("mixin.env.side", "CLIENT");
    }

    @Override
    public String getSideName() {
        return "CLIENT";
    }
}
