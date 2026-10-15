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
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let sane = value["cards"].is_array() && value.get("ledger").is_some();
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
    let compatible = crate::ipc::compatible_ledger(&value["ledger"]);
    if let Some(obj) = value.as_object_mut() {
        if !compatible {
            obj.insert("ledger".into(), serde_json::Value::Null);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use quotakeeper::ledger::{self, TokenBreakdown, UsageRecord};
    use serde_json::{Value, json};

    fn payload() -> Value {
        let now = chrono::Local::now();
        let report = ledger::aggregate(
            vec![UsageRecord {
                client: "codex",
                model: String::new(),
                ts_ms: now.timestamp_millis(),
                tokens: TokenBreakdown { input: 63, ..Default::default() },
                cost: None,
                cost_is_estimate: false,
                session: None,
                project: None,
                duration_ms: 0,
                ttft_ms: 0,
            }],
            now,
            &ledger::pricing::Pricing::default(),
        );
        json!({
            "cards": [{ "quota_marker": "preserved" }],
            "quota_fetched_at_ms": 123,
            "ledger": crate::ipc::ledger_dto(&report),
        })
    }

    #[test]
    fn replay_preserves_quota_but_withholds_incompatible_ledger_and_absent_model_rows() {
        let fresh = payload();
        assert_eq!(fresh["ledger"]["analysis_rows"][0]["agg"]["total"], 63);
        assert_eq!(fresh["ledger"]["analysis_rows"][0]["model"], "unknown");
        let replay: Value = serde_json::from_str(&mark_stale(&fresh.to_string()).unwrap()).unwrap();
        assert_eq!(replay["ledger"], fresh["ledger"]);
        assert_eq!(replay["stale"], true);
        assert!(replay["replayed_at_ms"].as_u64().is_some());
        for missing in [
            "/schema_version", "/report_id", "/timezone_offset", "/price_snapshot",
            "/price_snapshot/fetched_at_ms", "/price_bases", "/analysis_rows/0/model",
            "/analysis_rows/0/raw_model", "/analysis_rows/0/project", "/intraday_rows/0/project",
        ] {
            let mut legacy = fresh.clone();
            let (parent, key) = missing.rsplit_once('/').unwrap();
            legacy["ledger"]
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            let replay: Value =
                serde_json::from_str(&mark_stale(&legacy.to_string()).unwrap()).unwrap();
            assert!(replay["ledger"].is_null(), "{missing}");
            assert_eq!(replay["cards"], fresh["cards"]);
            assert_eq!(replay["quota_fetched_at_ms"], 123);
            assert_eq!(replay["stale"], true);
        }
        let mut dangling = fresh.clone();
        dangling["ledger"]["analysis_rows"][0]["price_basis_id"] = json!(999);
        let replay: Value =
            serde_json::from_str(&mark_stale(&dangling.to_string()).unwrap()).unwrap();
        assert!(replay["ledger"].is_null());
        assert_eq!(replay["cards"], fresh["cards"]);
    }
}
