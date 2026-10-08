//! 问题诊断快照 JNI。每次只读，不触发模拟或重建。

use jni::objects::JObject;
use jni::sys::{jint, jlong, jstring};
use jni::JNIEnv;
use std::panic::{catch_unwind, AssertUnwindSafe};

use super::MODELS;

#[no_mangle]
pub extern "system" fn Java_com_shiroha_mmdskin_NativeFunc_GetIssueDebugDiagnostic(
    env: JNIEnv,
    _receiver: JObject,
    handle: jlong,
    flags: jint,
) -> jstring {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let diagnostic = if handle <= 0 {
            "[MMD诊断] invalid model handle".to_owned()
        } else {
            let model = match MODELS.read() {
                Ok(models) => models.get(&handle).cloned(),
                Err(_) => None,
            };
            match model {
                Some(model) => match model.lock() {
                    Ok(model) => model.issue_debug_diagnostic(flags.max(0) as u32),
                    Err(_) => "[MMD诊断] model lock unavailable".to_owned(),
                },
                None => "[MMD诊断] invalid handle or model registry unavailable".to_owned(),
            }
        };
        env.new_string(diagnostic).map(|value| value.into_raw())
    }));
    match result {
        Ok(Ok(value)) => value,
        _ => std::ptr::null_mut(),
    }
}
