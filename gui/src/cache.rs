//! 上次 push 的本地留存:启动先回放缓存(旧数据立即可见),后台刷新到位后覆盖.
//!
//! - 落盘:每次满足节流条件的 push 后原子写(同目录 tmp + rename),失败静默
//!   (缓存只是体验优化,绝不能影响主流程);
//! - 回放:webview 就绪后把缓存的 payload 打上 `stale:true` 推给前端;
//! - 位置:`%USERPROFILE%\.quotakeeper\gui-last-push.json`.

use std::io::Write;
use std::path::PathBuf;

use quotakeeper::paths;

pub fn cache_path(home: &std::path::Path) -> PathBuf {
    home.join(".quotakeeper").join("gui-last-push.json")
}

/// 读缓存 payload(原始 JSON);缺失或不像我们的 push 则 None.
pub fn load() -> Option<String> {
    let text = std::fs::read_to_string(cache_path(&paths::home_dir())).ok()?;
    let sane = text.starts_with('{')
        && text.contains("\"generated_at_ms\"")
        && text.contains("\"ledger\"");
    sane.then_some(text)
}

/// 原子落盘;任何失败静默忽略(缓存不允许拖垮主流程).
pub fn save(payload: &str) {
    let path = cache_path(&paths::home_dir());
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = dir.join("gui-last-push.json.tmp");
    if std::fs::File::create(&tmp)
        .and_then(|mut f| f.write_all(payload.as_bytes()).and_then(|_| f.flush()))
        .is_ok()
        && std::fs::rename(&tmp, &path).is_err()
    {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// 给缓存的 payload 打上 stale 标记;解析失败返回 None.
pub fn mark_stale(payload: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(payload).ok()?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert("stale".into(), serde_json::Value::Bool(true));
        obj.insert(
            "replayed_at_ms".into(),
            serde_json::Value::from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
            ),
        );
        serde_json::to_string(&value).ok()
    } else {
        None
    }
}
