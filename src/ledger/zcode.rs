//! ZCode 会话采集:`~/.zcode/cli/db/db.sqlite` 的 `model_usage` 表(SQLite 只读).
//!
//! 口径抄 tokscale sessions/zcode.rs:
//! - 现代 schema 用 `computed_total_tokens` 列存在性探测;legacy 无该列;
//! - exclusive 风格归一化:total == input+output 时,cache/reasoning 是 input/output
//!   的子集 → 仅从 input 扣 cache;reasoning 留在 output(本项目是四桶).
//!   2026-09-25 实测本机数据正是这种:
//!   input 22822 含 cacheRead 19264,total 22948 = 22822+126);
//! - legacy 无 total,无条件扣;
//! - 加法式 total 若单列 reasoning,并入 output;否则 output 已含 reasoning.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use super::types::{TokenBreakdown, UsageRecord};

pub const CLIENT: &str = "zcode";

pub fn db_path(home: &Path) -> PathBuf {
    home.join(".zcode/cli/db/db.sqlite")
}

pub fn collect(path: &Path) -> Result<Vec<UsageRecord>, String> {
    let con = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("db.sqlite 打不开:{e}"))?;
    collect_conn(&con)
}

pub fn collect_conn(con: &Connection) -> Result<Vec<UsageRecord>, String> {
    let has_table: bool = con
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='model_usage'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    if !has_table {
        return Err("model_usage 表不存在(ZCode 版本过旧?)".into());
    }
    // 列存在性 = schema 新旧探测
    let modern: bool = con
        .prepare("SELECT computed_total_tokens FROM model_usage LIMIT 1")
        .is_ok();

    let sql = if modern {
        "SELECT mu.id, mu.model_id, mu.started_at, mu.completed_at, mu.duration_ms, \
         mu.input_tokens, mu.output_tokens, mu.reasoning_tokens, \
         mu.cache_read_input_tokens, mu.cache_creation_input_tokens, mu.computed_total_tokens, \
         mu.session_id, s.directory \
         FROM model_usage mu LEFT JOIN session s ON s.id = mu.session_id \
         ORDER BY COALESCE(mu.completed_at, mu.started_at, 0), mu.id"
    } else {
        "SELECT mu.id, mu.model_id, mu.started_at, mu.completed_at, mu.duration_ms, \
         mu.input_tokens, mu.output_tokens, mu.reasoning_tokens, \
         mu.cache_read_input_tokens, mu.cache_creation_input_tokens, NULL, \
         mu.session_id, NULL \
         FROM model_usage mu ORDER BY COALESCE(mu.completed_at, mu.started_at, 0), mu.id"
    };
    let mut stmt = con.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, Option<i64>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut recs = Vec::new();
    for r in rows {
        let (
            _id,
            model_id,
            started,
            completed,
            duration,
            input,
            output,
            reasoning,
            cr,
            cw,
            total,
            session,
            directory,
        ) = r.map_err(|e| e.to_string())?;
        let (mut input, mut output) = (input.max(0), output.max(0));
        let (reasoning, cr, cw) = (reasoning.max(0), cr.max(0), cw.max(0));
        let exclusive = match total {
            Some(t) => t == input + output,
            None => true, // legacy:无条件按 exclusive 扣
        };
        if exclusive {
            input = (input - cr - cw).max(0);
        } else if total == Some(input + output + cr + cw + reasoning) {
            output += reasoning;
        }
        let tokens = TokenBreakdown {
            input,
            output,
            cache_read: cr,
            cache_write: cw,
            reasoning: reasoning.min(output),
        };
        if tokens.is_zero() {
            continue;
        }
        // 时间:started_at 优先,否则 completed_at - duration,否则 completed_at.
        let ts = match (started, completed, duration) {
            (Some(s), _, _) if s > 0 => s,
            (_, Some(c), Some(d)) if c > 0 => (c - d.max(0)).max(0),
            (_, Some(c), _) if c > 0 => c,
            _ => 0,
        };
        recs.push(UsageRecord {
            client: CLIENT,
            model: model_id
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| "unknown".into()),
            ts_ms: ts,
            tokens,
            cost: None,
            cost_is_estimate: false,
            session,
            project: directory.filter(|d| !d.is_empty()),
            duration_ms: duration.unwrap_or(0).max(0),
            ttft_ms: 0,
        });
    }
    Ok(recs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(modern: bool) -> Connection {
        let con = Connection::open_in_memory().unwrap();
        let total_col = if modern {
            ", computed_total_tokens"
        } else {
            ""
        };
        con.execute_batch(&format!(
            "CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);
             CREATE TABLE model_usage (id TEXT PRIMARY KEY, session_id TEXT, model_id TEXT,
             started_at INTEGER, completed_at INTEGER, duration_ms INTEGER,
             input_tokens INTEGER, output_tokens INTEGER, reasoning_tokens INTEGER,
             cache_read_input_tokens INTEGER, cache_creation_input_tokens INTEGER{total_col});"
        ))
        .unwrap();
        con
    }

    #[test]
    fn exclusive_style_subtracts_overlap() {
        let con = setup(true);
        // 本机实测形状:input 含 cacheRead,total = input + output
        con.execute(
            "INSERT INTO model_usage (id, model_id, started_at, completed_at, duration_ms,
             input_tokens, output_tokens, reasoning_tokens, cache_read_input_tokens,
             cache_creation_input_tokens, computed_total_tokens)
             VALUES ('u1','GLM-5.3-Flash',1790293392603,1790293400467,7864,
             22822,126,30,19264,0,22948)",
            [],
        )
        .unwrap();
        let recs = collect_conn(&con).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].tokens.input, 22822 - 19264);
        assert_eq!(recs[0].tokens.output, 126);
        assert_eq!(recs[0].tokens.reasoning, 30);
        assert_eq!(recs[0].tokens.cache_read, 19264);
        assert_eq!(recs[0].tokens.total(), 22_948);
        assert_eq!(recs[0].ts_ms, 1_790_293_392_603);
    }

    #[test]
    fn additive_style_passes_through() {
        let con = setup(true);
        // total = input+output+cache:加法式,不扣
        con.execute(
            "INSERT INTO model_usage (id, model_id, started_at, completed_at, duration_ms,
             input_tokens, output_tokens, reasoning_tokens, cache_read_input_tokens,
             cache_creation_input_tokens, computed_total_tokens)
             VALUES ('u1','m',1,3,2, 100, 50, 10, 200, 0, 350)",
            [],
        )
        .unwrap();
        let recs = collect_conn(&con).unwrap();
        assert_eq!(recs[0].tokens.input, 100);
        assert_eq!(recs[0].tokens.output, 50);
        assert_eq!(recs[0].tokens.reasoning, 10);
        assert_eq!(recs[0].tokens.total(), 350);
        con.execute("UPDATE model_usage SET computed_total_tokens = 360", [])
            .unwrap();
        let recs = collect_conn(&con).unwrap();
        assert_eq!(recs[0].tokens.output, 60);
        assert_eq!(recs[0].tokens.total(), 360);
    }

    #[test]
    fn legacy_subtracts_unconditionally() {
        let con = setup(false);
        con.execute(
            "INSERT INTO model_usage (id, model_id, started_at, completed_at, duration_ms,
             input_tokens, output_tokens, reasoning_tokens, cache_read_input_tokens,
             cache_creation_input_tokens)
             VALUES ('u1','m',NULL,1000,500, 300, 40, 30, 200, 0)",
            [],
        )
        .unwrap();
        let recs = collect_conn(&con).unwrap();
        assert_eq!(recs[0].tokens.input, 100);
        assert_eq!(recs[0].tokens.output, 40);
        assert_eq!(recs[0].tokens.reasoning, 30);
        assert_eq!(recs[0].tokens.total(), 340);
        // started 缺失 → completed - duration
        assert_eq!(recs[0].ts_ms, 500);
    }

    #[test]
    fn zero_rows_and_missing_table() {
        let con = setup(true);
        con.execute(
            "INSERT INTO model_usage (id, model_id, input_tokens, output_tokens, reasoning_tokens,
             cache_read_input_tokens, cache_creation_input_tokens) VALUES ('u0','m',0,0,0,0,0)",
            [],
        )
        .unwrap();
        assert!(collect_conn(&con).unwrap().is_empty());
        let empty = Connection::open_in_memory().unwrap();
        assert!(collect_conn(&empty).is_err());
    }
}
