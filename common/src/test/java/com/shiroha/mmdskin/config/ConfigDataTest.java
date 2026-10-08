package com.shiroha.mmdskin.config;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

class ConfigDataTest {
    @Test
    void missingIssueLogFieldsDefaultToFalse(@TempDir Path configDir) throws Exception {
        Files.createDirectories(configDir);
        Files.writeString(configDir.resolve("config.json"), "{\"debugHudEnabled\":true}");

        ConfigData loaded = ConfigData.load(configDir);

        assertFalse(loaded.debugModelDeformationLog);
        assertFalse(loaded.debugSidePhysicsLog);
        assertFalse(loaded.debugCollisionModeLog);
        assertFalse(loaded.debugOutlinePaperDollLog);
        assertEquals(0, new TestConfig(loaded).getIssueDiagnosticFlags());
    }

    @Test
    void issueLogFieldsPersistIndependently(@TempDir Path configDir) {
        ConfigData data = new ConfigData();
        data.debugModelDeformationLog = true;
        data.debugSidePhysicsLog = false;
        data.debugCollisionModeLog = true;
        data.debugOutlinePaperDollLog = false;
        data.save(configDir);

        ConfigData loaded = ConfigData.load(configDir);

        assertEquals(IssueDiagnosticOptions.MODEL_DEFORMATION | IssueDiagnosticOptions.COLLISION_MODE,
                new TestConfig(loaded).getIssueDiagnosticFlags());
        loaded.debugModelDeformationLog = false;
        loaded.debugSidePhysicsLog = true;
        loaded.debugCollisionModeLog = false;
        loaded.debugOutlinePaperDollLog = true;
        loaded.save(configDir);
        ConfigData secondRoundTrip = ConfigData.load(configDir);
        assertEquals(IssueDiagnosticOptions.SIDE_PHYSICS | IssueDiagnosticOptions.OUTLINE_PAPER_DOLL,
                new TestConfig(secondRoundTrip).getIssueDiagnosticFlags());
    }

    private static final class TestConfig extends AbstractMmdSkinConfig {
        private TestConfig(ConfigData data) { super(data); }

        @Override public String getMobModelReplacement(String entityTypeId) { return ""; }
    }
}
