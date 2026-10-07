//! OMP 会话采集:`~/.omp/agent/sessions/<项目>/<ts>_<uuid>.jsonl`.
//!
//! 官方 stats 组件的 stats.db 自 2026-09-25 起不再写入(OMP 更新后断供),
//! 且末尾批次把 duration/ttft 落成了浮点,旧采集器按整数读取 → 一行坏行
//! 整源报错,OMP 在账本里整体消失。会话文件里的 assistant 消息自带完整
//! usage(四桶 token + OMP 自算成本),与 stats.db 同粒度,是权威源:
//! - 只扫项目目录下的主会话文件;会话子目录(`<ts>_<uuid>/`)里的工具
//!   日志与派生文件(CodeDir 等)不读,防双计数;
//! - 同一 responseId 的流式重写行取最后一行;
//! - 零 token 行(被取消的请求等)跳过,与 stats.db 口径一致。

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::types::{TokenBreakdown, UsageRecord};

pub const CLIENT: &str = "omp";

pub fn scan_paths(home: &Path) -> Vec<PathBuf> {
    scan_paths_in(&home.join(".omp").join("agent").join("sessions"))
}

fn scan_paths_in(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    for e in walkdir::WalkDir::new(root).max_depth(2) {
        let Ok(e) = e else { continue };
        // 主会话文件固定在项目目录下(深度 2);更深的是会话子目录里的派生文件
        if e.depth() != 2 || !e.file_type().is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy();
        let is_session = name.len() > 5
            && name.ends_with(".jsonl")
            && name.contains('_')
            && name.chars().next().is_some_and(|c| c.is_ascii_digit());
        if is_session {
            out.push(e.into_path());
        }
    }
    out
}

pub fn parse_file(path: &Path) -> io::Result<Vec<UsageRecord>> {
    let text = std::fs::read_to_string(path)?;
    // 文件名 <ts>_<uuid>.jsonl → 会话 id 取 uuid 段
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let session = stem.rsplit('_').next().map(String::from);
    Ok(parse_str(&text, session))
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    // session 行的 cwd 在行顶层(session/id/title/version 同级)
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    role: String,
    model: Option<String>,
    timestamp: Option<i64>,
    duration: Option<f64>,
    ttft: Option<f64>,
    #[serde(rename = "responseId")]
    response_id: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    input: i64,
    output: i64,
    #[serde(rename = "cacheRead")]
    cache_read: i64,
    #[serde(rename = "cacheWrite")]
    cache_write: i64,
    cost: Option<Cost>,
}

#[derive(Deserialize)]
struct Cost {
    total: Option<f64>,
}

fn parse_str(text: &str, session: Option<String>) -> Vec<UsageRecord> {
    let mut project: Option<String> = None;
    // 流式重写:同 responseId 的行按首见位置存、末见内容覆盖
    let mut order: Vec<String> = Vec::new();
    let mut seen: HashMap<String, Message> = HashMap::new();
    let mut anonymous: Vec<Message> = Vec::new();
    for line in text.lines() {
        let Ok(line) = serde_json::from_str::<Line>(line) else {
            continue;
        };
        if line.kind == "session" && project.is_none() && line.cwd.is_some() {
            project = line.cwd;
        }
        if line.kind != "message" {
            continue;
        }
        let Some(m) = line.message else { continue };
        if m.role != "assistant" {
            continue;
        }
        match m.response_id.clone() {
            Some(id) => {
                if !seen.contains_key(&id) {
                    order.push(id.clone());
                }
                seen.insert(id, m);
            }
            None => anonymous.push(m),
        }
    }
    let mut recs: Vec<UsageRecord> = order
        .into_iter()
        .filter_map(|id| seen.remove(&id))
        .chain(anonymous)
        .filter_map(|m| build_record(m, &session, &project))
        .collect();
    recs.sort_by_key(|r| r.ts_ms);
    recs
}

fn build_record(
    m: Message,
    session: &Option<String>,
    project: &Option<String>,
) -> Option<UsageRecord> {
    let u = m.usage?;
    let tokens = TokenBreakdown {
        input: u.input.max(0),
        output: u.output.max(0),
        cache_read: u.cache_read.max(0),
        cache_write: u.cache_write.max(0),
        reasoning: 0,
    };
    if tokens.is_zero() {
        return None;
    }
    Some(UsageRecord {
        client: CLIENT,
        model: m
            .model
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".into()),
        ts_ms: m.timestamp.unwrap_or(0),
        tokens,
        cost: u.cost.and_then(|c| c.total).filter(|c| c.is_finite()),
        cost_is_estimate: false,
        session: session.clone(),
        project: project.clone(),
        duration_ms: m.duration.map(|d| d.round().max(0.0) as i64).unwrap_or(0),
        ttft_ms: m.ttft.map(|d| d.round().max(0.0) as i64).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usage_cost_timing_and_merges_rewrites() {
        // 真实文件形态:session 行给 cwd;assistant 消息带 usage;同
        // responseId 的重写行取末行;user 行/零 token 行/坏行跳过
        let text = concat!(
            r#"{"type":"session","version":3,"id":"01a","cwd":"C:\\Users\\Jerry\\Desktop\\demo"}"#, "\n",
            r#"{"type":"message","id":"m1","timestamp":1,"message":{"role":"assistant","model":"deepseek-v4.1-flash","provider":"workbuddy-ai","timestamp":1791155575081,"duration":2988.7,"ttft":1406.9,"responseId":"resp_1","usage":{"input":15388,"output":154,"cacheRead":1200,"cacheWrite":0,"totalTokens":16742,"cost":{"input":0.03,"output":0.0015,"cacheRead":0,"cacheWrite":0,"total":0.0323},"premiumRequests":0}}}"#, "\n",
            r#"{"type":"message","id":"m2","message":{"role":"assistant","model":"glm-5.3","timestamp":1791155575900,"responseId":"resp_1","usage":{"input":10,"output":20,"cacheRead":0,"cacheWrite":0,"totalTokens":30,"cost":{"total":0.5}}}}"#, "\n",
            r#"{"type":"message","id":"m3","message":{"role":"user"}}"#, "\n",
            r#"{"type":"message","id":"m4","message":{"role":"assistant","model":"x","timestamp":1791155580000,"usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0}}}"#, "\n",
            "not-json-line", "\n",
            r#"{"type":"message","id":"m5","message":{"role":"assistant","model":"mimo-v2.6-flash","timestamp":1791155581000,"duration":2302.57,"ttft":1853.59,"responseId":"resp_2","usage":{"input":5,"output":6,"cacheRead":0,"cacheWrite":0}}}"#, "\n",
        );
        let recs = parse_str(text, Some("abc".into()));
        assert_eq!(recs.len(), 2);
        // resp_1 取重写末行(glm-5.3,10+20),deepseek 首行被覆盖
        assert_eq!(recs[0].model, "glm-5.3");
        assert_eq!(recs[0].tokens.input, 10);
        assert_eq!(recs[0].tokens.output, 20);
        assert_eq!(recs[0].cost, Some(0.5));
        assert!(!recs[0].cost_is_estimate);
        assert_eq!(recs[0].session.as_deref(), Some("abc"));
        assert_eq!(recs[0].project.as_deref(), Some("C:\\Users\\Jerry\\Desktop\\demo"));
        // 无 cost → None;浮点 duration/ttft 四舍五入成毫秒
        assert_eq!(recs[1].model, "mimo-v2.6-flash");
        assert_eq!(recs[1].cost, None);
        assert_eq!(recs[1].duration_ms, 2303);
        assert_eq!(recs[1].ttft_ms, 1854);
        assert_eq!(recs[1].tokens.input, 5);
    }

    #[test]
    fn anonymous_rows_and_missing_usage_are_kept_or_skipped() {
        // 无 responseId 的行不与任何 id 合并;usage 缺失/空消息跳过
        let text = concat!(
            r#"{"type":"message","id":"m1","message":{"role":"assistant","model":"a","timestamp":10,"usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0}}}"#, "\n",
            r#"{"type":"message","id":"m2","message":{"role":"assistant","model":"b","timestamp":20}}"#, "\n",
            r#"{"type":"message","id":"m3","message":{"role":"assistant","model":"c","timestamp":30,"usage":{"input":2,"output":2,"cacheRead":0,"cacheWrite":0}}}"#, "\n",
        );
        let recs = parse_str(text, None);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].model, "a");
        assert_eq!(recs[1].model, "c");
        assert_eq!(recs[0].session, None);
        assert_eq!(recs[0].project, None);
    }

    #[test]
    fn scan_takes_only_top_level_session_files() {
        let root = std::env::temp_dir().join(format!("tm-omp-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let proj = root.join("-Desktop");
        let inner = proj.join("2026-10-07T08-17-28-663Z_inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(proj.join("2026-10-07T08-17-28-663Z_abc.jsonl"), "").unwrap();
        std::fs::write(inner.join("CodeDir.jsonl"), "").unwrap();
        std::fs::write(inner.join("12.bash.log"), "").unwrap();
        std::fs::write(proj.join("notes.jsonl"), "").unwrap();
        let found = scan_paths_in(&root);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("2026-10-07T08-17-28-663Z_abc.jsonl"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn missing_sessions_dir_is_empty_not_error() {
        let found = scan_paths_in(Path::new("Z:/definitely/not/here"));
        assert!(found.is_empty());
    }
}
