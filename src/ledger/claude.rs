//! Claude Code 会话采集:`~/.claude/projects/**/*.jsonl`(CLAUDE_CONFIG_DIR 可重定向).
//!
//! 口径抄 tokscale sessions/claudecode.rs:
//! - 只认 `type=="assistant"` 且带 `message.usage` 的行;
//! - 同一轮的流式重写共享 `message.id`,按 `(message.id, requestId)` 去重、逐字段取 max;
//! - Anthropic 语义:input_tokens 不含 cache,cache_read/cache_creation 单列 → 原样入桶;
//! - 时间取首次出现的 timestamp(轮次开始),缺省回退文件 mtime.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::DateTime;

use super::types::{TokenBreakdown, UsageRecord};

pub const CLIENT: &str = "claude";

pub fn claude_config_dir(home: &Path) -> PathBuf {
    std::env::var("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".claude"))
}

pub fn scan_paths(home: &Path) -> Vec<PathBuf> {
    let root = claude_config_dir(home).join("projects");
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    for e in walkdir::WalkDir::new(root) {
        let Ok(e) = e else { continue };
        if e.file_type().is_file() && e.file_name().to_string_lossy().ends_with(".jsonl") {
            out.push(e.into_path());
        }
    }
    out
}

#[derive(Default, Clone)]
struct Merged {
    input: i64,
    output: i64,
    cache_read: i64,
    cache_write: i64,
    ts_ms: i64,
    model: String,
    session: Option<String>,
}

pub fn parse_file(path: &Path) -> std::io::Result<Vec<UsageRecord>> {
    let mut f = std::fs::File::open(path)?;
    let mut text = String::new();
    f.read_to_string(&mut text)?;
    let mtime_ms = mtime_ms(path);
    // 项目 = projects/ 下的目录名(Claude Code 以编码后的 cwd 作为目录名)
    let project = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned());
    Ok(parse_str(&text, mtime_ms, project))
}

fn mtime_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn parse_str(text: &str, fallback_ts_ms: i64, project: Option<String>) -> Vec<UsageRecord> {
    let mut merged: HashMap<(String, String), Merged> = HashMap::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let Some(usage) = v.pointer("/message/usage") else {
            continue;
        };
        let get = |k: &str| usage.get(k).and_then(|x| x.as_i64()).unwrap_or(0).max(0);
        let input = get("input_tokens");
        let output = get("output_tokens");
        let cache_read = get("cache_read_input_tokens");
        let cache_write = get("cache_creation_input_tokens");
        if input + output + cache_read + cache_write == 0 {
            continue;
        }
        let session_id = v
            .get("sessionId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let key = (
            v.pointer("/message/id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            v.get("requestId")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        );
        let ts = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.timestamp_millis())
            .unwrap_or(fallback_ts_ms);
        let model = v
            .pointer("/message/model")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string();
        let e = merged.entry(key).or_insert(Merged {
            ts_ms: ts,
            model,
            session: if session_id.is_empty() {
                None
            } else {
                Some(session_id.clone())
            },
            ..Merged::default()
        });
        // 流式重写逐字段取 max:同一 message.id 的后续快照只会更完整.
        e.input = e.input.max(input);
        e.output = e.output.max(output);
        e.cache_read = e.cache_read.max(cache_read);
        e.cache_write = e.cache_write.max(cache_write);
        e.ts_ms = e.ts_ms.min(ts);
    }
    let mut recs: Vec<UsageRecord> = merged
        .into_values()
        .map(|m| UsageRecord {
            client: CLIENT,
            model: m.model,
            ts_ms: m.ts_ms,
            tokens: TokenBreakdown {
                input: m.input,
                output: m.output,
                cache_read: m.cache_read,
                cache_write: m.cache_write,
                reasoning: 0,
            },
            cost: None,
            cost_is_estimate: false,
            session: m.session,
            project: project.clone(),
        })
        .collect();
    recs.sort_by_key(|r| r.ts_ms);
    recs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(
        ts: &str,
        msg_id: &str,
        req: &str,
        input: i64,
        output: i64,
        cache_read: i64,
        cache_write: i64,
    ) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","requestId":"{req}","isSidechain":false,"message":{{"id":"{msg_id}","model":"claude-opus-4","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_read_input_tokens":{cache_read},"cache_creation_input_tokens":{cache_write}}}}}}}"#
        )
    }

    #[test]
    fn streaming_rewrites_merge_by_max() {
        let text = format!(
            "{}\n{}\n{}\n",
            line("2026-09-20T10:00:00Z", "msg_1", "req_1", 100, 10, 0, 0),
            // 同一 message 的中间快照(字段更小)不得叠加
            line("2026-09-20T10:00:02Z", "msg_1", "req_1", 150, 5, 0, 0),
            // 最终快照
            line("2026-09-20T10:00:04Z", "msg_1", "req_1", 200, 20, 500, 100),
        );
        let recs = parse_str(&text, 0, None);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].tokens.input, 200);
        assert_eq!(recs[0].tokens.output, 20);
        assert_eq!(recs[0].tokens.cache_read, 500);
        assert_eq!(recs[0].tokens.cache_write, 100);
        // 时间取首次出现
        assert_eq!(recs[0].ts_ms, 1_789_898_400_000);
        assert_eq!(recs[0].model, "claude-opus-4");
    }

    #[test]
    fn distinct_requests_are_separate_records() {
        let text = format!(
            "{}\n{}\n",
            line("2026-09-20T10:00:00Z", "msg_1", "req_1", 100, 10, 0, 0),
            line("2026-09-20T10:01:00Z", "msg_2", "req_2", 100, 10, 0, 0),
        );
        let recs = parse_str(&text, 0, None);
        assert_eq!(recs.len(), 2);
    }

    #[test]
    fn zero_and_non_assistant_lines_are_skipped() {
        let text = format!(
            "{}\n{}\n{}\n",
            r#"{"type":"user","message":{"role":"user"}}"#,
            line("2026-09-20T10:00:00Z", "msg_1", "req_1", 0, 0, 0, 0),
            "garbage",
        );
        assert!(parse_str(&text, 0, None).is_empty());
    }

    #[test]
    fn records_sorted_by_ts() {
        let text = format!(
            "{}\n{}\n",
            line("2026-09-20T10:05:00Z", "msg_2", "req_2", 1, 1, 0, 0),
            line("2026-09-20T10:00:00Z", "msg_1", "req_1", 1, 1, 0, 0),
        );
        let recs = parse_str(&text, 0, None);
        assert!(recs[0].ts_ms <= recs[1].ts_ms);
    }
}
