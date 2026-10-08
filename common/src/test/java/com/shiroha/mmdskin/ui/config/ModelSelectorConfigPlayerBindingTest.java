package com.shiroha.mmdskin.ui.config;

import com.shiroha.mmdskin.config.UIConstants;
import com.shiroha.mmdskin.player.sync.PlayerModelSyncService;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

class ModelSelectorConfigPlayerBindingTest {
    @TempDir
    Path directory;

    @AfterEach
    void clearBroadcaster() {
        PlayerModelSyncService.setNetworkBroadcaster(null);
    }

    @Test
    void switchingBindingKeyPersistsOnlyTheSelectedKeyImmediately() throws Exception {
        Path file = directory.resolve("model_selector.json");
        UUID playerUuid = UUID.fromString("b7746b98-a27b-4c7a-b01c-3e4dc3f9d611");
        Files.writeString(file, """
            {"playerModels":{"b7746b98-a27b-4c7a-b01c-3e4dc3f9d611":"OldUuid","Alice":"OldName"},"quickModelSlots":{}}
            """);
        ModelSelectorConfig config = new ModelSelectorConfig(file.toFile());

        config.setPlayerModelBinding(playerUuid, "Alice", true, "UuidModel", null, null);
        config.setPlayerModelBinding(playerUuid, "Alice", false, "NameModel", null, null);
        assertEquals("NameModel", config.getPlayerModelByUuidOrName(playerUuid, "Alice"));

        ModelSelectorConfig afterNameBinding = new ModelSelectorConfig(file.toFile());
        assertNull(afterNameBinding.getRawModel(playerUuid.toString()));
        assertEquals("NameModel", afterNameBinding.getRawModel("Alice"));
        assertEquals("NameModel", afterNameBinding.getPlayerModelByUuidOrName(playerUuid, "Alice"));

        afterNameBinding.setPlayerModelBinding(playerUuid, "Alice", true,
                UIConstants.DEFAULT_MODEL_NAME, null, null);
        ModelSelectorConfig afterReset = new ModelSelectorConfig(file.toFile());
        assertNull(afterReset.getRawModel(playerUuid.toString()));
        assertNull(afterReset.getRawModel("Alice"));
    }

    @Test
    void localBindingsBroadcastForUuidOrNameAndBlankResetUsesDefault() {
        UUID localUuid = UUID.fromString("b7746b98-a27b-4c7a-b01c-3e4dc3f9d611");
        UUID remoteUuid = UUID.fromString("67ce71f7-3299-4e1a-8041-5d335598af42");
        List<String> broadcasts = new ArrayList<>();
        PlayerModelSyncService.setNetworkBroadcaster((uuid, model) -> broadcasts.add(uuid + ":" + model));
        ModelSelectorConfig config = new ModelSelectorConfig(directory.resolve("sync.json").toFile());

        config.setPlayerModelBinding(remoteUuid, "Local", true, "UuidRemote", localUuid, "Local");
        config.setPlayerModelBinding(localUuid, "Local", true, "UuidLocal", localUuid, "Local");
        config.setPlayerModelBinding(localUuid, "Local", false, "NameLocal", localUuid, "Local");
        assertEquals("NameLocal", config.getPlayerModelByUuidOrName(localUuid, "Local"));

        config.setPlayerModelBinding(localUuid, "Local", true, "UuidPriority", localUuid, "Local");
        config.setPlayerModelBinding(null, "Local", false, "NameOnlyLocal", localUuid, "Local");
        config.setPlayerModelBinding(null, "Local", false, "   ", localUuid, "Local");
        config.setPlayerModelBinding(null, "Elsewhere", false, "NameOnlyRemote", localUuid, "Local");

        assertEquals(List.of(
                localUuid + ":UuidLocal",
                localUuid + ":NameLocal",
                localUuid + ":UuidPriority",
                localUuid + ":UuidPriority",
                localUuid + ":UuidPriority"), broadcasts);
        assertEquals("UuidPriority", config.getPlayerModelByUuidOrName(localUuid, "Local"));
    }

    @Test
    void blankNameOnlyResetBroadcastsDefaultWhenNoUuidFallbackExists() {
        UUID localUuid = UUID.fromString("b7746b98-a27b-4c7a-b01c-3e4dc3f9d611");
        List<String> broadcasts = new ArrayList<>();
        PlayerModelSyncService.setNetworkBroadcaster((uuid, model) -> broadcasts.add(uuid + ":" + model));
        ModelSelectorConfig config = new ModelSelectorConfig(directory.resolve("blank-reset.json").toFile());

        config.setPlayerModelBinding(null, "Local", false, "NameOnly", localUuid, "Local");
        config.setPlayerModelBinding(null, "Local", false, "   ", localUuid, "Local");

        assertEquals(List.of(localUuid + ":NameOnly", localUuid + ":" + UIConstants.DEFAULT_MODEL_NAME), broadcasts);
        assertEquals(UIConstants.DEFAULT_MODEL_NAME, config.getPlayerModelByUuidOrName(localUuid, "Local"));
    }
}
