package com.shiroha.mmdskin.compat.maid.runtime;

import com.shiroha.mmdskin.config.UIConstants;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

/** 验证女仆模型有效绑定的优先级。 */
class MaidModelBindingResolverTest {
    @Test
    void localDefaultOverridesRemoteAndAnAbsentLocalChoiceFallsBackToRemote() {
        assertEquals(UIConstants.DEFAULT_MODEL_NAME,
                MaidModelBindingResolver.resolve(UIConstants.DEFAULT_MODEL_NAME, "RemoteModel"));
        assertEquals("RemoteModel", MaidModelBindingResolver.resolve(null, "RemoteModel"));
        assertEquals(UIConstants.DEFAULT_MODEL_NAME, MaidModelBindingResolver.resolve(null, null));
    }
}
