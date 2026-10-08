package com.shiroha.mmdskin.config;

/** 集中定义问题诊断日志位。 */
public final class IssueDiagnosticOptions {
    public static final int MODEL_DEFORMATION = 1;
    public static final int SIDE_PHYSICS = 1 << 1;
    public static final int COLLISION_MODE = 1 << 2;
    public static final int OUTLINE_PAPER_DOLL = 1 << 3;

    private IssueDiagnosticOptions() {}

    public static int from(boolean modelDeformation, boolean sidePhysics,
                           boolean collisionMode, boolean outlinePaperDoll) {
        return (modelDeformation ? MODEL_DEFORMATION : 0)
                | (sidePhysics ? SIDE_PHYSICS : 0)
                | (collisionMode ? COLLISION_MODE : 0)
                | (outlinePaperDoll ? OUTLINE_PAPER_DOLL : 0);
    }
}
