//! Codex CLI 会话采集:`~/.codex/sessions/**/rollout-*.jsonl`(含 archived_sessions).
//!
//! 口径抄 tokscale sessions/codex.rs 的防重复计数规则:
//! - 增量取 `payload.info.last_token_usage`,不用 total(total 是可变快照,
//!   compaction/上下文截断会改写历史值);
//! - 首条只计 last(resume 会话的 total 里承接了历史 token,直接用会重复计);
//! - total 与上一条相等 → 重复快照,跳过;
//! - total 小幅回退(≤2%)→ 陈旧快照回写,跳过,不当 reset;
//! - total 大幅回退 → 新基线,接受本条 last;
//! - 归一化:Codex 的 input 含 cached,扣出 cache_read;reasoning ⊆ output 只作 info.

use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::DateTime;

use super::types::{TokenBreakdown, UsageRecord};

pub const CLIENT: &str = "codex";

pub fn scan_paths(home: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in ["sessions", "archived_sessions"] {
        let root = home.join(".codex").join(sub);
        if !root.is_dir() {
            continue;
        }
        for e in walkdir::WalkDir::new(root) {
            let Ok(e) = e else { continue };
            if e.file_type().is_file() && e.file_name().to_string_lossy().ends_with(".jsonl") {
                out.push(e.into_path());
            }
        }
    }
    out
}

pub fn parse_file(path: &Path) -> Vec<UsageRecord> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut text = String::new();
    if f.read_to_string(&mut text).is_err() {
        return Vec::new();
    }
    let mtime_ms = mtime_ms(path);
    parse_str(&text, mtime_ms)
}

fn mtime_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn parse_str(text: &str, fallback_ts_ms: i64) -> Vec<UsageRecord> {
    let mut model = String::from("unknown");
    let mut session: Option<String> = None;
    let mut project: Option<String> = None;
    let mut prev_total: Option<i64> = None;
    let mut recs = Vec::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        // 会话头:session_id 与 cwd(项目维度)
        if v.get("type").and_then(|t| t.as_str()) == Some("session_meta") {
            session = v
                .pointer("/payload/session_id")
                .and_then(|x| x.as_str())
                .map(String::from);
            project = v
                .pointer("/payload/cwd")
                .and_then(|x| x.as_str())
                .map(String::from);
        }
        // turn_context 等任意 payload 带 model 字段都更新"当前模型".
        if let Some(m) = v.pointer("/payload/model").and_then(|x| x.as_str())
            && !m.is_empty()
        {
            model = m.to_string();
        }
        if v.get("type").and_then(|t| t.as_str()) != Some("event_msg") {
            continue;
        }
        let Some(last) = parse_breakdown(v.pointer("/payload/info/last_token_usage")) else {
            continue;
        };
        let total = v
            .pointer("/payload/info/total_token_usage/total_tokens")
            .and_then(|x| x.as_i64());
        if let (Some(t), Some(prev)) = (total, prev_total) {
            if t == prev {
                continue; // 重复快照
            }
            if t < prev && (prev - t) * 50 <= prev.max(1) {
                continue; // ≤2% 回退 = 陈旧快照
            }
        }
        prev_total = total;
        let ts = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(parse_rfc3339_ms)
            .unwrap_or(fallback_ts_ms);
        recs.push(UsageRecord {
            client: CLIENT,
            model: model.clone(),
            ts_ms: ts,
            tokens: last,
            cost: None,
            cost_is_estimate: false,
            session: session.clone(),
            project: project.clone(),
        });
    }
    recs
}

fn parse_breakdown(v: Option<&serde_json::Value>) -> Option<TokenBreakdown> {
    let v = v?;
    let input = v.get("input_tokens")?.as_i64()?.max(0);
    let output = v
        .get("output_tokens")
        .and_then(|x| x.as_i64())
        .unwrap_or(0)
        .max(0);
    let cached = v
        .get("cached_input_tokens")
        .and_then(|x| x.as_i64())
        .unwrap_or(0)
        .clamp(0, input);
    let cache_write = v
        .get("cache_write_input_tokens")
        .and_then(|x| x.as_i64())
        .unwrap_or(0)
        .max(0);
    let reasoning = v
        .get("reasoning_output_tokens")
        .and_then(|x| x.as_i64())
        .unwrap_or(0)
        .clamp(0, output);
    Some(TokenBreakdown {
        input: input - cached,
        output,
        cache_read: cached,
        cache_write,
        reasoning,
    })
}

fn parse_rfc3339_ms(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(ts: &str, total: i64, last_in: i64, cached: i64, out: i64, reasoning: i64) -> String {
        serde_json::json!({
            "timestamp": ts,
            "ordinal": 1,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "total_token_usage": {
                        "input_tokens": total, "cached_input_tokens": cached,
                        "cache_write_input_tokens": 0, "output_tokens": 100,
                        "reasoning_output_tokens": reasoning, "total_tokens": total
                    },
                    "last_token_usage": {
                        "input_tokens": last_in, "cached_input_tokens": cached,
                        "cache_write_input_tokens": 0, "output_tokens": out,
                        "reasoning_output_tokens": reasoning, "total_tokens": last_in
                    }
                }
            }
        })
        .to_string()
    }

    #[test]
    fn first_event_counts_last_only_and_increment_after() {
        let text = format!(
            "{}\n{}\n",
            line("2026-09-23T07:30:20.932Z", 31_773, 31_773, 16_256, 275, 19),
            line("2026-09-23T07:31:00.000Z", 40_000, 8_227, 1_000, 300, 20),
        );
        let recs = parse_str(&text, 0);
        assert_eq!(recs.len(), 2);
        // 首条只计 last,不把 total 再叠一遍(resume 场景防重复).
        assert_eq!(recs[0].tokens.input, 31_773 - 16_256);
        assert_eq!(recs[0].tokens.cache_read, 16_256);
        assert_eq!(recs[0].tokens.output, 275);
        assert_eq!(recs[0].tokens.reasoning, 19);
        // 第二条按 last 增量计.
        assert_eq!(recs[1].tokens.input, 8_227 - 1_000);
        assert_eq!(recs[1].tokens.cache_read, 1_000);
        assert_eq!(recs[1].tokens.output, 300);
    }

    #[test]
    fn duplicate_and_stale_snapshots_are_skipped() {
        let text = format!(
            "{}\n{}\n{}\n{}\n",
            line("2026-09-23T07:30:20Z", 10_000, 10_000, 0, 5, 0),
            // total 相等 = 重复快照
            line("2026-09-23T07:30:25Z", 10_000, 0, 0, 0, 0),
            // 回退 1% = 陈旧快照
            line("2026-09-23T07:30:30Z", 9_900, 9_900, 0, 5, 0),
            // 正常增长
            line("2026-09-23T07:31:00Z", 12_000, 2_100, 0, 6, 0),
        );
        let recs = parse_str(&text, 0);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[1].tokens.input, 2_100);
    }

    #[test]
    fn big_regression_rebaselines() {
        let text = format!(
            "{}\n{}\n",
            line("2026-09-23T07:30:20Z", 50_000, 50_000, 0, 5, 0),
            // 回退远超 2% = compaction 重置,接受新基线
            line("2026-09-23T07:31:20Z", 1_000, 1_000, 0, 5, 0),
        );
        let recs = parse_str(&text, 0);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[1].tokens.input, 1_000);
    }

    #[test]
    fn model_tracks_last_seen_payload_model() {
        let text = format!(
            "{}\n{}\n",
            r#"{"timestamp":"2026-09-23T07:28:31Z","type":"turn_context","payload":{"turn_id":"t1","model":"gpt-6-sol"}}"#,
            line("2026-09-23T07:30:20Z", 100, 100, 0, 1, 0),
        );
        let recs = parse_str(&text, 0);
        assert_eq!(recs[0].model, "gpt-6-sol");
    }

    #[test]
    fn clamps_cached_and_reasoning() {
        let text = line("2026-09-23T07:30:20Z", 500, 500, 900, 30, 99);
        let recs = parse_str(&text, 0);
        assert_eq!(recs[0].tokens.cache_read, 500); // 钳到 input
        assert_eq!(recs[0].tokens.input, 0);
        assert_eq!(recs[0].tokens.reasoning, 30); // 钳到 output
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let recs = parse_str("not json\n{\"x\":1}\n", 0);
        assert!(recs.is_empty());
    }
}
