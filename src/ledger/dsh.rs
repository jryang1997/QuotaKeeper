//! DeepSeek Harness(DSH)会话采集:`$DSH_HOME`/`~/.dsh/sessions/**/session*.jsonl.zstd`.
//!
//! 文件是**拼接的多帧 zstd**(每批 append 一帧,zstd 流式 Decoder 默认跨帧连续解).
//! 行结构:每行 `{type, seq, time(epoch ms), data}`:
//! - `request/header` → `data.config.model`(provider 在 config.provider);
//! - `assistant/message` → `data.usage.{inputTokens,outputTokens,cacheReadTokens,
//!   cacheWriteTokens,totalTokens}`(实测 usage 挂在 data 下,不在 message 里;
//!   兼容 message.usage 旧位置);实测 total = input+output+cacheRead →
//!   input 不含 cache,原样入桶.

use std::io::Read;
use std::path::{Path, PathBuf};

use super::types::{TokenBreakdown, UsageRecord};

pub const CLIENT: &str = "dsh";

pub fn dsh_home(home: &Path) -> PathBuf {
    std::env::var("DSH_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".dsh"))
}

pub fn scan_paths(home: &Path) -> Vec<PathBuf> {
    let root = dsh_home(home).join("sessions");
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    for e in walkdir::WalkDir::new(root) {
        let Ok(e) = e else { continue };
        let name = e.file_name().to_string_lossy();
        if e.file_type().is_file() && name.starts_with("session") && name.ends_with(".jsonl.zstd") {
            out.push(e.into_path());
        }
    }
    out
}

pub fn parse_file(path: &Path) -> std::io::Result<Vec<UsageRecord>> {
    let bytes = std::fs::read(path)?;
    let mtime_ms = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    parse_bytes(&bytes, mtime_ms)
}

pub fn parse_bytes(bytes: &[u8], fallback_ts_ms: i64) -> std::io::Result<Vec<UsageRecord>> {
    let mut dec = zstd::stream::read::Decoder::new(std::io::Cursor::new(bytes))?;
    let mut raw = Vec::new();
    dec.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw);
    let mut model = String::from("unknown");
    let mut session: Option<String> = None;
    let mut project: Option<String> = None;
    let mut recs = Vec::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("session") => {
                // 会话头:顶层 id + cwd(实测都在顶层)
                session = v
                    .pointer("/id")
                    .or_else(|| v.pointer("/data/id"))
                    .and_then(|x| x.as_str())
                    .map(String::from);
                project = v
                    .pointer("/cwd")
                    .or_else(|| v.pointer("/data/cwd"))
                    .and_then(|x| x.as_str())
                    .map(String::from);
                continue;
            }
            Some("request/header") => {
                // 实测嵌套:data.header.config.model(config 与 adapterDefaults 同级)
                if let Some(m) = [
                    "/data/header/config/model",
                    "/data/config/model",
                    "/data/model",
                ]
                .iter()
                .find_map(|p| v.pointer(p))
                .and_then(|x| x.as_str())
                    && !m.is_empty()
                {
                    model = m.to_string();
                }
                continue;
            }
            Some("assistant/message") => {}
            _ => continue,
        }
        // usage 实测在 data.usage;兼容旧位置 data.message.usage
        let u = match v.pointer("/data/usage").filter(|x| x.is_object()) {
            Some(u) => u,
            None => match v.pointer("/data/message/usage") {
                Some(u) if u.is_object() => u,
                _ => continue,
            },
        };
        let get = |k: &str| u.get(k).and_then(|x| x.as_i64()).unwrap_or(0).max(0);
        let tokens = TokenBreakdown {
            input: get("inputTokens"),
            output: get("outputTokens"),
            cache_read: get("cacheReadTokens"),
            cache_write: get("cacheWriteTokens"),
            reasoning: 0,
        };
        if tokens.is_zero() {
            continue;
        }
        let ts = v
            .get("time")
            .and_then(|x| x.as_i64())
            .unwrap_or(fallback_ts_ms);
        recs.push(UsageRecord {
            client: CLIENT,
            model: model.clone(),
            ts_ms: ts,
            tokens,
            cost: None,
            cost_is_estimate: false,
            session: session.clone(),
            project: project.clone(),
        });
    }
    Ok(recs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concatenated_frames_fully_decode() {
        // DSH 是每批 append 一帧:两个独立压缩帧拼一个文件,必须两帧都解出.
        let frame1 = "{\"type\":\"session\",\"time\":1000,\"data\":{}}\n\
                      {\"type\":\"request/header\",\"time\":1001,\"data\":{\"header\":{\"config\":{\"provider\":\"deepseek-official\",\"model\":\"deepseek-flash\"}}}}\n\
                      {\"type\":\"assistant/message\",\"time\":1002,\"data\":{\"usage\":{\"inputTokens\":9913,\"outputTokens\":525,\"cacheReadTokens\":384,\"cacheWriteTokens\":0,\"totalTokens\":10822}}}\n";
        let frame2 = "{\"type\":\"assistant/message\",\"time\":2000,\"data\":{\"usage\":{\"inputTokens\":10,\"outputTokens\":5,\"cacheReadTokens\":0,\"cacheWriteTokens\":0,\"totalTokens\":15}}}\n";
        let mut bytes = zstd::encode_all(frame1.as_bytes(), 3).unwrap();
        bytes.extend(zstd::encode_all(frame2.as_bytes(), 3).unwrap());
        let recs = parse_bytes(&bytes, 0).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].model, "deepseek-flash");
        assert_eq!(recs[0].tokens.input, 9913);
        assert_eq!(recs[0].tokens.cache_read, 384);
        assert_eq!(recs[0].ts_ms, 1002);
        assert_eq!(recs[1].tokens.total(), 15);
    }

    #[test]
    fn zero_usage_is_skipped_and_bad_compression_is_an_error() {
        assert!(parse_bytes(b"not zstd at all", 0).is_err());
        let plain = b"{\"type\":\"assistant/message\",\"time\":1,\"data\":{\"message\":{\"usage\":{\"inputTokens\":0,\"outputTokens\":0,\"totalTokens\":0}}}}\n".as_slice();
        let bytes = zstd::encode_all(plain, 3).unwrap();
        assert!(parse_bytes(&bytes, 0).unwrap().is_empty());
    }
}
