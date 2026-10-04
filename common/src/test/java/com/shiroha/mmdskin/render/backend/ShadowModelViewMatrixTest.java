package com.shiroha.mmdskin.render.backend;

import org.joml.Matrix4f;
import org.joml.Vector3f;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.assertSame;

class ShadowModelViewMatrixTest {
    private static final float EPSILON = 0.00001f;

    @Test
    void shadowPositionShouldIgnoreMainCameraRotation() {
        // 模拟 Iris 已合成的光源视图、实体位移与模型缩放。
        Matrix4f shadowPose = new Matrix4f().rotateX((float) Math.PI / 2)
                .translate(2, 3, 4).scale(0.5f);
        Matrix4f destination = new Matrix4f();
        Vector3f vertex = new Vector3f(2, 0, 0);

        Matrix4f result = BaseModelInstance.composeModelViewMatrix(
                new Matrix4f().rotateY(1.2f), shadowPose, true, destination);
        assertSame(destination, result);
        assertTrue(result.transformPosition(vertex, new Vector3f())
                .equals(new Vector3f(3, -4, 3), EPSILON));

        Matrix4f turnedCameraResult = BaseModelInstance.composeModelViewMatrix(
                new Matrix4f().rotateY(-0.7f), shadowPose, true, new Matrix4f());
        assertTrue(result.equals(turnedCameraResult, EPSILON));
    }

    @Test
    void worldPositionShouldStillIncludeMainCameraView() {
        Matrix4f camera = new Matrix4f().rotateY((float) Math.PI / 2);
        Matrix4f entity = new Matrix4f().translate(2, 3, 4).scale(0.5f);

        Matrix4f result = BaseModelInstance.composeModelViewMatrix(
                camera, entity, false, new Matrix4f());
        assertTrue(result.transformPosition(new Vector3f(2, 0, 0), new Vector3f())
                .equals(new Vector3f(4, 3, -3), EPSILON));
    }

    @Test
    void switchingBetweenShadowAndWorldShouldNotMutateInputMatrices() {
        Matrix4f camera = new Matrix4f().rotateY(0.6f);
        Matrix4f entity = new Matrix4f().rotateX(0.3f).translate(2, 3, 4);
        Matrix4f originalCamera = new Matrix4f(camera);
        Matrix4f originalEntity = new Matrix4f(entity);
        Matrix4f destination = new Matrix4f();

        BaseModelInstance.composeModelViewMatrix(camera, entity, true, destination);
        BaseModelInstance.composeModelViewMatrix(camera, entity, false, destination);
        BaseModelInstance.composeModelViewMatrix(camera, entity, true, destination);

        assertTrue(camera.equals(originalCamera, EPSILON));
        assertTrue(entity.equals(originalEntity, EPSILON));
        assertTrue(destination.equals(originalEntity, EPSILON));
    }
}
