//! OpenCode 会话采集:`~/.local/share/opencode/opencode.db`(SQLite 只读).
//!
//! 两代 schema 都探(实测本机只有 v2 的 session_message,无 legacy message 表):
//! - v2:`session_message(type='assistant')`,model 在 `$.model.{id,providerID}`;
//! - v1:`message`,role 在 `$.role`,model 在顶层 `$.modelID`;
//! - token 一律在 `$.tokens.{input,output,reasoning,cache.read,cache.write}`,
//!   time.created 为 epoch 毫秒(可能是浮点);
//! - OpenCode 的 input 不含 cache → 原样入桶.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use super::types::{TokenBreakdown, UsageRecord};

pub const CLIENT: &str = "opencode";

pub fn db_path(home: &Path) -> PathBuf {
    // XDG_DATA_HOME 优先(与 OpenCode 本身一致),Windows 下通常走 ~/.local/share.
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
        && !xdg.is_empty()
    {
        let p = PathBuf::from(xdg).join("opencode").join("opencode.db");
        if p.is_file() {
            return p;
        }
    }
    home.join(".local/share/opencode/opencode.db")
}

pub fn collect(path: &Path) -> Result<Vec<UsageRecord>, String> {
    let con = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("opencode.db 打不开:{e}"))?;
    collect_conn(&con)
}

pub fn collect_conn(con: &Connection) -> Result<Vec<UsageRecord>, String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut recs = Vec::new();
    // v2:session_message(JOIN session_v2 拿 directory 作项目)
    if let Ok(mut stmt) = con.prepare(
        "SELECT sm.id, sm.session_id, sm.data, s.directory FROM session_message sm \
         LEFT JOIN session_v2 s ON s.id = sm.session_id \
         WHERE sm.type = 'assistant' \
         AND json_extract(sm.data, '$.tokens') IS NOT NULL ORDER BY sm.id",
    ) {
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (id, session, data, directory) = r.map_err(|e| e.to_string())?;
            if seen.insert(id)
                && let Some(mut rec) = parse_data(&data)
            {
                rec.session = session;
                rec.project = directory.filter(|d| !d.is_empty());
                recs.push(rec);
            }
        }
    }
    // v1:legacy message 表(存在才贡献)
    if let Ok(mut stmt) = con.prepare(
        "SELECT m.id, m.session_id, m.data, s.directory FROM message m \
         LEFT JOIN session s ON s.id = m.session_id \
         WHERE json_extract(m.data, '$.role') = 'assistant' \
         AND json_extract(m.data, '$.tokens') IS NOT NULL ORDER BY m.id",
    ) {
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (id, session, data, directory) = r.map_err(|e| e.to_string())?;
            if seen.insert(id)
                && let Some(mut rec) = parse_data(&data)
            {
                rec.session = session;
                rec.project = directory.filter(|d| !d.is_empty());
                recs.push(rec);
            }
        }
    }
    Ok(recs)
}

fn parse_data(data: &str) -> Option<UsageRecord> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    let t = v.get("tokens")?;
    let get = |pointer: &str| {
        t.pointer(pointer)
            .and_then(|x| x.as_i64())
            .unwrap_or(0)
            .max(0)
    };
    let tokens = TokenBreakdown {
        input: get("/input"),
        output: get("/output"),
        cache_read: get("/cache/read"),
        cache_write: get("/cache/write"),
        reasoning: get("/reasoning"),
    };
    if tokens.is_zero() {
        return None;
    }
    // v1 顶层 modelID 优先,v2 在 $.model.id.
    let model = v
        .get("modelID")
        .and_then(|x| x.as_str())
        .or_else(|| v.pointer("/model/id").and_then(|x| x.as_str()))
        .filter(|m| !m.is_empty())
        .unwrap_or("unknown")
        .to_string();
    // time.created 正常是毫秒浮点;防一下秒级时间戳.
    let ts = v
        .pointer("/time/created")
        .and_then(|x| x.as_f64())
        .map(|f| {
            let ms = f as i64;
            if ms > 0 && ms < 100_000_000_000 {
                ms * 1000
            } else {
                ms
            }
        })
        .unwrap_or(0);
    Some(UsageRecord {
        client: CLIENT,
        model,
        ts_ms: ts,
        tokens,
        cost: None,
        cost_is_estimate: false,
        session: None,
        project: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_v2() -> Connection {
        let con = Connection::open_in_memory().unwrap();
        con.execute_batch(
            "CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT, type TEXT,
             seq INTEGER, time_created INTEGER, time_updated INTEGER, data TEXT);
             CREATE TABLE session_v2 (id TEXT PRIMARY KEY, directory TEXT, title TEXT);
             CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT, title TEXT);",
        )
        .unwrap();
        con
    }

    #[test]
    fn v2_rows_parse_nested_model() {
        let con = setup_v2();
        con.execute(
            "INSERT INTO session_message (id, session_id, type, data) VALUES
             ('m1', 's1', 'assistant', '{\"tokens\":{\"input\":100,\"output\":50,\"reasoning\":10,\"cache\":{\"read\":200,\"write\":5}},\"model\":{\"id\":\"mimo-v2.6-flash-free\",\"providerID\":\"opencode\"},\"time\":{\"created\":1790145652652.0}}'),
             ('m2', 's1', 'user', '{\"tokens\":{\"input\":9}}'),
             ('m3', 's1', 'assistant', '{\"other\":1}')",
            [],
        )
        .unwrap();
        let recs = collect_conn(&con).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].model, "mimo-v2.6-flash-free");
        assert_eq!(recs[0].tokens.cache_read, 200);
        assert_eq!(recs[0].tokens.input, 100);
        assert_eq!(recs[0].ts_ms, 1_790_145_652_652);
    }

    #[test]
    fn v1_rows_parse_top_level_model() {
        let con = setup_v2();
        con.execute_batch(
            "CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, data TEXT);",
        )
        .unwrap();
        con.execute(
            "INSERT INTO message (id, session_id, data) VALUES
             ('m9', 's1', '{\"role\":\"assistant\",\"tokens\":{\"input\":7,\"output\":3},\"modelID\":\"qwen3-max\",\"time\":{\"created\":1700000000.0}}')",
            [],
        )
        .unwrap();
        let recs = collect_conn(&con).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].model, "qwen3-max");
        // 秒级时间戳被放大到毫秒
        assert_eq!(recs[0].ts_ms, 1_700_000_000_000);
    }

    #[test]
    fn missing_tables_yield_empty_not_error() {
        let con = Connection::open_in_memory().unwrap();
        assert!(collect_conn(&con).unwrap().is_empty());
    }
}
