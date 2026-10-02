//! Claude Code 会话采集:`~/.claude/projects/**/*.jsonl`(CLAUDE_CONFIG_DIR 可重定向).
//!
//! 口径抄 tokscale sessions/claudecode.rs:
//! - 只认 `type=="assistant"` 且带 `message.usage` 的行;
//! - 同一轮的流式重写共享 `message.id`,按 `(message.id, requestId)` 去重、逐字段取 max;
//! - Anthropic 语义:input_tokens 不含 cache,cache_read/cache_creation 单列 → 原样入桶;
//! - 时间取首次出现的 timestamp(轮次开始),缺省回退文件 mtime;
//! - ttft/duration:沿用 magpie 的口径——配对同侧(main/sidechain)最近一条 user 行
//!   (提示或工具结果)为发起行,首快照 − 发起行 = ttft,末快照 − 发起行 = duration;
//!   异常跨度(负值/超上限)丢弃,不臆造.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::DateTime;

use super::types::{timed_span, TokenBreakdown, UsageRecord};

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
    max_ts: i64,
    ask_ts: i64,
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
    // 每条链(main / sidechain)最近一条 user 行的时间,作为下一次调用的发起行.
    let mut last_ask = [0i64; 2];
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let side = v
            .get("isSidechain")
            .and_then(|x| x.as_bool())
            .unwrap_or(false) as usize;
        if kind == "user" {
            if let Some(ts) = v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp_millis())
            {
                last_ask[side] = ts;
            }
            continue;
        }
        if kind != "assistant" {
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
            max_ts: ts,
            ask_ts: last_ask[side],
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
        e.max_ts = e.max_ts.max(ts);
    }
    let mut recs: Vec<UsageRecord> = merged
        .into_values()
        .map(|m| {
            let (ttft_ms, duration_ms) = timed_span(m.ask_ts, m.ts_ms, m.max_ts);
            UsageRecord {
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
                duration_ms,
                ttft_ms,
            }
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

    fn user_line(ts: &str, side: bool) -> String {
        format!(
            r#"{{"type":"user","timestamp":"{ts}","isSidechain":{side},"message":{{"role":"user","content":"hi"}}}}"#
        )
    }

    fn parse_millis(s: &str) -> i64 {
        DateTime::parse_from_rfc3339(s).unwrap().timestamp_millis()
    }

    #[test]
    fn ttft_and_duration_pair_with_preceding_user_line() {
        let text = format!(
            "{}\n{}\n{}\n",
            user_line("2026-09-20T10:00:00Z", false),
            line("2026-09-20T10:00:03Z", "msg_1", "req_1", 100, 5, 0, 0),
            line("2026-09-20T10:00:09Z", "msg_1", "req_1", 100, 20, 0, 0),
        );
        let recs = parse_str(&text, 0, None);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].ttft_ms, 3_000);
        assert_eq!(recs[0].duration_ms, 9_000);
    }

    #[test]
    fn calls_without_asking_line_have_no_span() {
        let recs = parse_str(&line("2026-09-20T10:00:00Z", "msg_1", "req_1", 1, 1, 0, 0), 0, None);
        assert_eq!(recs[0].ttft_ms, 0);
        assert_eq!(recs[0].duration_ms, 0);
    }

    fn side_line(ts: &str, msg_id: &str, req: &str, input: i64, output: i64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","isSidechain":true,"requestId":"{req}","message":{{"id":"{msg_id}","model":"claude-opus-4","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}}}}"#
        )
    }

    #[test]
    fn sidechain_pairs_with_its_own_asking_line() {
        // 主链 user 行在前,子代理的 user 行更晚:子代理的调用必须配子代理发起行.
        let text = format!(
            "{}\n{}\n{}\n{}\n",
            user_line("2026-09-20T10:00:00Z", false),
            user_line("2026-09-20T10:01:00Z", true),
            side_line("2026-09-20T10:01:02Z", "msg_s", "req_s", 1, 1),
            line("2026-09-20T10:02:00Z", "msg_m", "req_m", 1, 1, 0, 0),
        );
        let recs = parse_str(&text, 0, None);
        let side = recs.iter().find(|r| r.ts_ms == parse_millis("2026-09-20T10:01:02Z")).unwrap();
        assert_eq!(side.ttft_ms, 2_000);
        let main = recs.iter().find(|r| r.ts_ms == parse_millis("2026-09-20T10:02:00Z")).unwrap();
        assert_eq!(main.ttft_ms, 120_000);
    }

    #[test]
    fn absurd_span_is_dropped() {
        // 发起行在 1 小时前:超出上限,不臆造.
        let text = format!(
            "{}\n{}\n",
            user_line("2026-09-20T09:00:00Z", false),
            line("2026-09-20T10:00:00Z", "msg_1", "req_1", 1, 1, 0, 0),
        );
        let recs = parse_str(&text, 0, None);
        assert_eq!(recs[0].ttft_ms, 0);
        assert_eq!(recs[0].duration_ms, 0);
    }
}
