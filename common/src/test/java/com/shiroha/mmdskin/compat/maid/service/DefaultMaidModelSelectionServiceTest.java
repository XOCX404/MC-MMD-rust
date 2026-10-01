/** 文件职责：验证女仆模型选择服务的绑定与同步行为。 */
package com.shiroha.mmdskin.compat.maid.service;

import com.shiroha.mmdskin.config.UIConstants;
import org.junit.jupiter.api.Test;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicReference;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

class DefaultMaidModelSelectionServiceTest {
    @Test
    void shouldReturnDefaultModelWhenNoBindingExists() {
        DefaultMaidModelSelectionService service = new DefaultMaidModelSelectionService(
                List::of,
                maidUUID -> null,
                (maidUUID, modelName) -> {
                },
                (entityId, modelName) -> {
                });

        assertEquals(UIConstants.DEFAULT_MODEL_NAME, service.getCurrentModel(UUID.randomUUID()));
    }

    @Test
    void shouldBindAndSyncSelectedModel() {
        UUID maidUUID = UUID.randomUUID();
        AtomicReference<String> boundModel = new AtomicReference<>();
        AtomicReference<String> syncedModel = new AtomicReference<>();
        AtomicReference<Integer> syncedEntityId = new AtomicReference<>();
        DefaultMaidModelSelectionService service = new DefaultMaidModelSelectionService(
                () -> List.of(UIConstants.DEFAULT_MODEL_NAME, "Alice"),
                uuid -> UIConstants.DEFAULT_MODEL_NAME,
                (uuid, modelName) -> boundModel.set(uuid + ":" + modelName),
                (entityId, modelName) -> {
                    syncedEntityId.set(entityId);
                    syncedModel.set(modelName);
                });

        service.selectModel(maidUUID, 12, "Alice");

        assertEquals(maidUUID + ":Alice", boundModel.get());
        assertEquals(12, syncedEntityId.get());
        assertEquals("Alice", syncedModel.get());
    }

    @Test
    void shouldPersistExplicitDefaultAndSkipSyncWithoutAValidEntityId() {
        UUID maidUUID = UUID.randomUUID();
        AtomicReference<String> persistedModel = new AtomicReference<>();
        DefaultMaidModelSelectionService service = new DefaultMaidModelSelectionService(
                List::of,
                uuid -> "Alice",
                (uuid, modelName) -> persistedModel.set(modelName),
                (entityId, modelName) -> {
                    throw new AssertionError("实体 ID 不合法时不应发送网络消息");
                });

        service.selectModel(maidUUID, 0, UIConstants.DEFAULT_MODEL_NAME);

        assertEquals(UIConstants.DEFAULT_MODEL_NAME, persistedModel.get());
    }

    @Test
    void localSelectionSurvivesNetworkFailure() {
        UUID maidUUID = UUID.randomUUID();
        AtomicReference<String> persistedModel = new AtomicReference<>();
        DefaultMaidModelSelectionService service = new DefaultMaidModelSelectionService(
                List::of,
                uuid -> null,
                (uuid, modelName) -> persistedModel.set(modelName),
                (entityId, modelName) -> { throw new IllegalStateException("offline"); });

        service.selectModel(maidUUID, 7, "Alice");

        assertEquals("Alice", persistedModel.get());
    }

    @Test
    void saveFailureStopsBeforeNetworkSynchronization() {
        UUID maidUUID = UUID.randomUUID();
        AtomicReference<String> syncedModel = new AtomicReference<>();
        DefaultMaidModelSelectionService service = new DefaultMaidModelSelectionService(
                List::of,
                uuid -> null,
                (uuid, modelName) -> { throw new IllegalStateException("disk full"); },
                (entityId, modelName) -> syncedModel.set(modelName));

        org.junit.jupiter.api.Assertions.assertThrows(IllegalStateException.class,
                () -> service.selectModel(maidUUID, 7, "Alice"));
        assertNull(syncedModel.get());
    }

    @Test
    void staleEntityIdSkipsNetworkSyncAfterSavingLocalPreference() {
        UUID maidUUID = UUID.randomUUID();
        AtomicReference<String> persistedModel = new AtomicReference<>();
        AtomicReference<String> syncedModel = new AtomicReference<>();
        DefaultMaidModelSelectionService service = new DefaultMaidModelSelectionService(
                List::of,
                uuid -> null,
                (uuid, modelName) -> persistedModel.set(modelName),
                (entityId, modelName) -> syncedModel.set(modelName),
                (uuid, entityId) -> false);

        service.selectModel(maidUUID, 42, "Alice");

        assertEquals("Alice", persistedModel.get());
        assertNull(syncedModel.get());
    }
}
