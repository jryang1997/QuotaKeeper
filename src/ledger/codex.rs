//! Codex CLI 会话采集:`~/.codex/sessions/**/rollout-*.jsonl`(含 archived_sessions).
//!
//! 口径抄 tokscale sessions/codex.rs 的防重复计数规则:
//! - 增量取 `payload.info.last_token_usage`,不用 total(total 是可变快照,
//!   compaction/上下文截断会改写历史值);
//! - 首条只计 last(resume 会话的 total 里承接了历史 token,直接用会重复计);
//! - total 与上一条相等 → 重复快照,跳过;
//! - total 小幅回退(≤2%)→ 陈旧快照回写,跳过,不当 reset;
//! - total 大幅回退 → 新基线,接受本条 last;
//! - 归一化:Codex 的 input 含 cached,扣出 cache_read;reasoning ⊆ output 只作 info;
//! - ttft/duration:user 行(response_item message role=user / event_msg user_message)为发起行,
//!   本轮首条 token_count − 发起行 = ttft,末条 − 发起行 = duration,记到首/末条上.

use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::DateTime;

use super::types::{timed_span, TokenBreakdown, UsageRecord};

pub const CLIENT: &str = "codex";

pub fn scan_paths(home: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // CODEX_HOME 重定向优先;默认 ~/.codex(与 Claude 的 CLAUDE_CONFIG_DIR 对齐)。
    let codex_dir = std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".codex"));
    for sub in ["sessions", "archived_sessions"] {
        let root = codex_dir.join(sub);
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

pub fn parse_file(path: &Path) -> std::io::Result<Vec<UsageRecord>> {
    let mut f = std::fs::File::open(path)?;
    let mut text = String::new();
    f.read_to_string(&mut text)?;
    let mtime_ms = mtime_ms(path);
    Ok(parse_str(&text, mtime_ms))
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
    let mut recs: Vec<UsageRecord> = Vec::new();
    // 本轮(自最近 user 行起)已接受记录的下标;新 user 行或文件结束时结算 ttft/duration.
    let mut run: Vec<usize> = Vec::new();
    let mut ask_ts = 0i64;
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(ts) = user_line_ts(&v) {
            finalize_run(&mut recs, &mut run, ask_ts);
            ask_ts = ts;
            continue;
        }
        // id 标识当前 thread;session_id 在新格式中可能是 fork 的 root。
        if v.get("type").and_then(|t| t.as_str()) == Some("session_meta") {
            session = v
                .pointer("/payload/id")
                .or_else(|| v.pointer("/payload/session_id"))
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
            duration_ms: 0,
            ttft_ms: 0,
        });
        run.push(recs.len() - 1);
    }
    finalize_run(&mut recs, &mut run, ask_ts);
    recs
}

/// user 行的发起时间;非 user 行为 None.兼容新旧 rollout 两种载体.
fn user_line_ts(v: &serde_json::Value) -> Option<i64> {
    let typ = v.get("type").and_then(|t| t.as_str())?;
    let payload_type = v.pointer("/payload/type").and_then(|x| x.as_str());
    let is_user = match (typ, payload_type) {
        ("response_item", Some("message")) => {
            v.pointer("/payload/role").and_then(|x| x.as_str()) == Some("user")
        }
        ("event_msg", Some("user_message")) => true,
        _ => false,
    };
    if !is_user {
        return None;
    }
    v.get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(parse_rfc3339_ms)
}

/// 把本轮首/末条记录的时间跨度写回;无发起行则全部保持 0.
fn finalize_run(recs: &mut [UsageRecord], run: &mut Vec<usize>, ask_ts: i64) {
    if let (Some(&first), Some(&last)) = (run.first(), run.last()) {
        let (ttft, duration) = timed_span(ask_ts, recs[first].ts_ms, recs[last].ts_ms);
        recs[first].ttft_ms = ttft;
        recs[last].duration_ms = duration;
    }
    run.clear();
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
    fn session_meta_keeps_threads_distinct_and_accepts_legacy_id() {
        for (header, expected) in [
            (
                r#"{"type":"session_meta","payload":{"id":"thread","cwd":"project"}}"#,
                "thread",
            ),
            (
                r#"{"type":"session_meta","payload":{"id":"fork","session_id":"root","cwd":"project"}}"#,
                "fork",
            ),
            (
                r#"{"type":"session_meta","payload":{"session_id":"legacy","cwd":"project"}}"#,
                "legacy",
            ),
        ] {
            let text = format!(
                "{header}\n{}",
                line("2026-09-23T07:30:20Z", 100, 100, 0, 1, 0)
            );
            let recs = parse_str(&text, 0);
            assert_eq!(recs[0].session.as_deref(), Some(expected));
            assert_eq!(recs[0].project.as_deref(), Some("project"));
        }
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let recs = parse_str("not json\n{\"x\":1}\n", 0);
        assert!(recs.is_empty());
    }

    fn user_message(ts: &str) -> String {
        serde_json::json!({
            "timestamp": ts,
            "type": "response_item",
            "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "hi"}]}
        })
        .to_string()
    }

    #[test]
    fn run_span_lands_on_first_and_last_record() {
        let text = format!(
            "{}\n{}\n{}\n",
            user_message("2026-09-23T07:30:00Z"),
            line("2026-09-23T07:30:04Z", 100, 100, 0, 1, 0),
            line("2026-09-23T07:30:10Z", 200, 100, 0, 1, 0),
        );
        let recs = parse_str(&text, 0);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].ttft_ms, 4_000);
        assert_eq!(recs[0].duration_ms, 0);
        assert_eq!(recs[1].ttft_ms, 0);
        assert_eq!(recs[1].duration_ms, 10_000);
    }

    #[test]
    fn single_record_run_carries_both_spans() {
        let text = format!(
            "{}\n{}\n",
            user_message("2026-09-23T07:30:00Z"),
            line("2026-09-23T07:30:05Z", 100, 100, 0, 1, 0),
        );
        let recs = parse_str(&text, 0);
        assert_eq!(recs[0].ttft_ms, 5_000);
        assert_eq!(recs[0].duration_ms, 5_000);
    }

    #[test]
    fn records_without_asking_user_line_have_no_span() {
        let recs = parse_str(&line("2026-09-23T07:30:20Z", 100, 100, 0, 1, 0), 0);
        assert_eq!(recs[0].ttft_ms, 0);
        assert_eq!(recs[0].duration_ms, 0);
    }

    #[test]
    fn new_user_line_starts_a_fresh_run() {
        let text = format!(
            "{}\n{}\n{}\n{}\n",
            user_message("2026-09-23T07:30:00Z"),
            line("2026-09-23T07:30:04Z", 100, 100, 0, 1, 0),
            user_message("2026-09-23T07:31:00Z"),
            line("2026-09-23T07:31:03Z", 200, 100, 0, 1, 0),
        );
        let recs = parse_str(&text, 0);
        assert_eq!(recs[0].ttft_ms, 4_000);
        assert_eq!(recs[0].duration_ms, 4_000);
        assert_eq!(recs[1].ttft_ms, 3_000);
        assert_eq!(recs[1].duration_ms, 3_000);
    }
}
