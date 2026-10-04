package com.shiroha.mmdskin.model.runtime;

import com.shiroha.mmdskin.config.ModelConfigData;
import com.shiroha.mmdskin.model.port.ModelRuntimeAccessPort;
import com.mojang.blaze3d.vertex.PoseStack;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.world.entity.Entity;
import org.joml.Vector3f;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class ModelRepositoryTailOptionsTest {
    @Test
    void newModelUsesEnabledDefaultTailOptions() {
        List<String> calls = new ArrayList<>();
        ModelRepository repository = new ModelRepository(new RuntimeAccess(calls));

        repository.initializePhysicsOptions(new FakeModelInstance(39L, calls), new ModelConfigData());

        assertEquals(List.of("reset", "tail:39:true:true"), calls);
    }

    @Test
    void newModelPhysicsInitializationResetsThenAppliesSavedTailOptions() {
        List<String> calls = new ArrayList<>();
        ModelRepository repository = new ModelRepository(new RuntimeAccess(calls));
        ModelConfigData config = new ModelConfigData();
        config.tailIdleLiftEnabled = true;
        config.tailMovementBoostEnabled = false;
        FakeModelInstance model = new FakeModelInstance(41L, calls);

        repository.initializePhysicsOptions(model, config);

        assertEquals(List.of("reset", "tail:41:true:false"), calls);
    }

    private static final class RuntimeAccess implements ModelRuntimeAccessPort {
        private final List<String> calls;

        private RuntimeAccess(List<String> calls) {
            this.calls = calls;
        }

        @Override public boolean isRenderingShadows() { return false; }
        @Override public ModelInstance createModelFromHandle(long handle, String dir, boolean pmd) { return null; }
        @Override public long loadPmxModel(String file, String dir, long layers) { return 0; }
        @Override public long loadPmdModel(String file, String dir, long layers) { return 0; }
        @Override public long loadVrmModel(String file, String dir, long layers) { return 0; }
        @Override public int getMaterialCount(long handle) { return 0; }
        @Override public String getMaterialTexturePath(long handle, int index) { return null; }
        @Override public void setMaterialVisible(long handle, int index, boolean visible) { }
        @Override public void preloadTexture(String path) { }
        @Override public void clearPreloadedTextures() { }
        @Override public void tickTextures() { }
        @Override public void deleteModel(long handle) { }

        @Override
        public void setTailPhysicsOptions(long handle, boolean idleLift, boolean movementBoost) {
            calls.add("tail:" + handle + ":" + idleLift + ":" + movementBoost);
        }
    }

    private static final class FakeModelInstance implements ModelInstance {
        private final long handle;
        private final List<String> calls;

        private FakeModelInstance(long handle, List<String> calls) {
            this.handle = handle;
            this.calls = calls;
        }

        @Override public void render(Entity entity, float yaw, float pitch, Vector3f translation, float delta,
                                     PoseStack poseStack, int light, com.shiroha.mmdskin.render.scene.RenderScene scene) { }
        @Override public void changeAnim(long animation, long layer) { }
        @Override public void transitionAnim(long animation, long layer, float time) { }
        @Override public void resetPhysics() { calls.add("reset"); }
        @Override public long getModelHandle() { return handle; }
        @Override public String getModelDir() { return "test-model"; }
        @Override public boolean setLayerBoneMask(int layer, String root) { return false; }
        @Override public boolean setLayerBoneExclude(int layer, String root) { return false; }
        @Override public void dispose() { }
        @Override public long getRamUsage() { return 0; }
    }
}
