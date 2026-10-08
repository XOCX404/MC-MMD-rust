package com.shiroha.mmdskin.render.outline;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class OutlineRenderDiagnosticsTest {
    @Test
    void disabledGateDoesNotConsumeSampleAndSceneHasIndependentInterval() {
        OutlineRenderDiagnostics.RateLimiter limiter = new OutlineRenderDiagnostics.RateLimiter(4);

        assertFalse(limiter.shouldSample(false, 12L, "GUI", 10L));
        assertTrue(limiter.shouldSample(true, 12L, "GUI", 10L));
        assertFalse(limiter.shouldSample(true, 12L, "GUI", 10L + 1_000_000_000L));
        assertTrue(limiter.shouldSample(true, 12L, "PAPERDOLL", 10L + 1_000_000_000L));
        assertTrue(limiter.shouldSample(true, 12L, "GUI", 10L + 2_000_000_000L));
    }

    @Test
    void rateLimiterKeepsOnlyItsConfiguredNumberOfKeys() {
        OutlineRenderDiagnostics.RateLimiter limiter = new OutlineRenderDiagnostics.RateLimiter(2);

        assertTrue(limiter.shouldSample(true, 1L, "WORLD", 1L));
        assertTrue(limiter.shouldSample(true, 2L, "WORLD", 1L));
        assertTrue(limiter.shouldSample(true, 3L, "WORLD", 1L));
        assertTrue(limiter.shouldSample(true, 1L, "WORLD", 2L));
    }
}
