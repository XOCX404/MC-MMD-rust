package com.shiroha.mmdskin.ui.selector.adapter;

import com.shiroha.mmdskin.bridge.runtime.NativeScenePort;
import com.shiroha.mmdskin.config.ModelConfigData;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class DefaultModelSettingsTailOptionsTest {
    @Test
    void appliesEnabledDefaultsToEveryLoadedInstanceWithTheSameModelName() {
        ModelConfigData config = new ModelConfigData();
        List<String> calls = new ArrayList<>();
        NativeScenePort port = new RecordingScenePort(calls);
        List<DefaultModelSettingsRuntimeGateway.TailPhysicsTarget> models = List.of(
                new DefaultModelSettingsRuntimeGateway.TailPhysicsTarget("same", 5L),
                new DefaultModelSettingsRuntimeGateway.TailPhysicsTarget("same", 13L));

        DefaultModelSettingsRuntimeGateway.applyTailOptionsToMatchingModels("same", config, models, port);

        assertEquals(List.of("tail:5:true:true", "tail:13:true:true"), calls);
    }

    @Test
    void appliesSavedTailOptionsToEveryLoadedInstanceWithTheSameModelName() {
        ModelConfigData config = new ModelConfigData();
        config.tailIdleLiftEnabled = true;
        config.tailMovementBoostEnabled = true;
        List<String> calls = new ArrayList<>();
        NativeScenePort port = new RecordingScenePort(calls);
        List<DefaultModelSettingsRuntimeGateway.TailPhysicsTarget> models = List.of(
                new DefaultModelSettingsRuntimeGateway.TailPhysicsTarget("same", 5L),
                new DefaultModelSettingsRuntimeGateway.TailPhysicsTarget("other", 8L),
                new DefaultModelSettingsRuntimeGateway.TailPhysicsTarget("same", 13L));

        DefaultModelSettingsRuntimeGateway.applyTailOptionsToMatchingModels("same", config, models, port);

        assertEquals(List.of("tail:5:true:true", "tail:13:true:true"), calls);
    }

    @Test
    void preservesExplicitlyDisabledOptionsWhenApplyingToLoadedInstances() {
        ModelConfigData config = new ModelConfigData();
        config.tailIdleLiftEnabled = false;
        config.tailMovementBoostEnabled = false;
        List<String> calls = new ArrayList<>();
        NativeScenePort port = new RecordingScenePort(calls);
        List<DefaultModelSettingsRuntimeGateway.TailPhysicsTarget> models = List.of(
                new DefaultModelSettingsRuntimeGateway.TailPhysicsTarget("same", 21L));

        DefaultModelSettingsRuntimeGateway.applyTailOptionsToMatchingModels("same", config, models, port);

        assertEquals(List.of("tail:21:false:false"), calls);
    }

    private static final class RecordingScenePort implements NativeScenePort {
        private final List<String> calls;

        private RecordingScenePort(List<String> calls) {
            this.calls = calls;
        }

        @Override public void setHeadAngle(long handle, float x, float y, float z, boolean world) { }
        @Override public void setModelPositionAndYaw(long handle, float x, float y, float z, float yaw) { }
        @Override public void setAutoBlinkEnabled(long handle, boolean enabled) { }
        @Override public void setEyeTrackingEnabled(long handle, boolean enabled) { }
        @Override public void setEyeMaxAngle(long handle, float angle) { }
        @Override public void setEyeAngle(long handle, float x, float y) { }
        @Override public void setTailPhysicsOptions(long handle, boolean idleLift, boolean movementBoost) {
            calls.add("tail:" + handle + ":" + idleLift + ":" + movementBoost);
        }
    }
}
