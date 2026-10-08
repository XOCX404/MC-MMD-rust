package com.shiroha.mmdskin.debug.client;

import com.shiroha.mmdskin.bridge.runtime.NativeRenderBackendPort;
import com.shiroha.mmdskin.config.ConfigManager;
import com.shiroha.mmdskin.config.IssueDiagnosticOptions;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

import java.util.function.Consumer;
import java.util.function.IntSupplier;
import java.util.function.LongSupplier;

/** 按模型限频读取诊断，关闭时不调用 JNI，不依赖旧物理日志。 */
public final class ModelIssueDiagnostics {
    private static final Logger LOGGER = LogManager.getLogger();
    private static final int NATIVE_FLAGS = IssueDiagnosticOptions.MODEL_DEFORMATION
            | IssueDiagnosticOptions.SIDE_PHYSICS | IssueDiagnosticOptions.COLLISION_MODE;
    private static final int MANIFEST = 0x100;
    private static final long INTERVAL_NS = 2_000_000_000L;
    private final IntSupplier flagsSource;
    private final LongSupplier clock;
    private final Consumer<String> output;
    private final Consumer<String> warning;
    private int previousFlags;
    private long previousSample;
    private boolean sampled;
    private boolean unavailable;

    public ModelIssueDiagnostics() {
        this(ConfigManager::getIssueDiagnosticFlags, System::nanoTime,
                message -> LOGGER.info("{}", message), message -> LOGGER.warn("{}", message));
    }

    ModelIssueDiagnostics(IntSupplier flagsSource, LongSupplier clock,
                          Consumer<String> output, Consumer<String> warning) {
        this.flagsSource = flagsSource;
        this.clock = clock;
        this.output = output;
        this.warning = warning;
    }

    public void sample(NativeRenderBackendPort backend, long modelHandle,
                       String modelName, String backendName) {
        int flags = flagsSource.getAsInt() & NATIVE_FLAGS;
        boolean changed = flags != previousFlags;
        if (changed) {
            previousFlags = flags;
            sampled = false;
            unavailable = false;
        }
        if (flags == 0 || modelHandle == 0 || unavailable) return;
        long now = clock.getAsLong();
        if (sampled && now - previousSample < INTERVAL_NS) return;
        int request = flags | (sampled ? 0 : MANIFEST);
        previousSample = now;
        sampled = true;
        String context = "handle=" + modelHandle + " model=" + safe(modelName)
                + " backend=" + safe(backendName);
        try {
            String records = backend.getIssueDebugDiagnostic(modelHandle, request);
            output.accept("[MMD诊断][SAMPLE] " + context + " flags=" + flags
                    + " interval_seconds=2 snapshot=true");
            if (records == null || records.isBlank()) {
                output.accept("[MMD诊断][UNAVAILABLE] " + context + " native_snapshot=empty");
            } else {
                // 每行附加实例来源，避免多人/多模型日志相互混淆。
                records.lines().filter(line -> !line.isBlank())
                        .forEach(line -> output.accept(line + " " + context));
            }
        } catch (UnsatisfiedLinkError error) {
            unavailable = true;
            warning.accept("[MMD诊断][UNAVAILABLE] " + context
                    + " 原生诊断接口缺失，请安装同一构建中的 Jar 和原生库并重启游戏。");
        } catch (RuntimeException error) {
            // 取证失败不能中断游戏绘制；下一个采样窗口仍可恢复。
            warning.accept("[MMD诊断][UNAVAILABLE] " + context + " error="
                    + safe(error.getClass().getSimpleName()));
        }
    }

    private static String safe(String text) {
        return text == null ? "unknown" : text.replace("\r", "\\r")
                .replace("\n", "\\n").replace("\t", "\\t");
    }
}
