//! OMP 会话采集:`~/.omp/stats.db` 的 `messages` 表(OMP 官方 stats 组件的预聚合账本).
//!
//! 该表每行 = 一次模型请求:五种 token 互不重叠(total = input+output+cache_read+cache_write,
//! 2026-09-25 实测),`cost_total` 是 OMP 自己算的美元成本 → 直接采用,不走 LiteLLM 估价.
//! timestamp 为整数 epoch 毫秒.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use super::types::{TokenBreakdown, UsageRecord};

pub const CLIENT: &str = "omp";

pub fn db_path(home: &Path) -> std::path::PathBuf {
    home.join(".omp").join("stats.db")
}

pub fn collect(path: &Path) -> Result<Vec<UsageRecord>, String> {
    let con = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("stats.db 打不开:{e}"))?;
    collect_conn(&con)
}

pub fn collect_conn(con: &Connection) -> Result<Vec<UsageRecord>, String> {
    // 旧版 stats.db 没有 duration/ttft 列:pragma 探测,缺列就查 NULL(不计时).
    let timing_cols: i64 = con
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name IN ('duration','ttft')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("stats.db 无 messages 表(omp stats 组件未启用?): {e}"))?;
    let timing_select = if timing_cols == 2 { ", duration, ttft" } else { ", NULL, NULL" };
    let sql = format!(
        "SELECT model, timestamp, input_tokens, output_tokens, \
         cache_read_tokens, cache_write_tokens, cost_total, session_file, folder{timing_select} \
         FROM messages"
    );
    let mut stmt = con
        .prepare(&sql)
        .map_err(|e| format!("stats.db 无 messages 表(omp stats 组件未启用?): {e}"))?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<f64>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<i64>>(9)?,
            row.get::<_, Option<i64>>(10)?,
        ))
    });
    finish_rows(rows.and_then(|mapped| mapped.collect::<Result<Vec<OmpRow>, _>>()))
}

type OmpRow = (
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<f64>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
);

fn finish_rows(
    rows: Result<Vec<OmpRow>, rusqlite::Error>,
) -> Result<Vec<UsageRecord>, String> {
    let rows = rows.map_err(|e| e.to_string())?;
    let mut recs = Vec::new();
    for r in rows {
        let (
            model,
            ts,
            input,
            output,
            cache_read,
            cache_write,
            cost,
            session_file,
            folder,
            duration,
            ttft,
        ) = r;
        let input = input.unwrap_or(0).max(0);
        let output = output.unwrap_or(0).max(0);
        let cache_read = cache_read.unwrap_or(0).max(0);
        let cache_write = cache_write.unwrap_or(0).max(0);
        let tokens = TokenBreakdown {
            input,
            output,
            cache_read,
            cache_write,
            reasoning: 0,
        };
        if tokens.is_zero() {
            continue;
        }
        // session_file 形如 <时间戳>_<uuid>.jsonl → 会话 id 取 uuid 段
        let session = session_file.as_deref().and_then(|f| {
            f.rsplit('_')
                .next()
                .map(|u| u.trim_end_matches(".jsonl").to_string())
        });
        recs.push(UsageRecord {
            client: CLIENT,
            model: model
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| "unknown".into()),
            ts_ms: ts.unwrap_or(0),
            tokens,
            cost,
            cost_is_estimate: false,
            session,
            project: folder,
            duration_ms: duration.unwrap_or(0).max(0),
            ttft_ms: ttft.unwrap_or(0).max(0),
        });
    }
    Ok(recs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let con = Connection::open_in_memory().unwrap();
        con.execute_batch(
            "CREATE TABLE messages (id TEXT, session_file TEXT, entry_id TEXT, folder TEXT,
             model TEXT, provider TEXT, api TEXT, timestamp INTEGER, duration INTEGER, ttft INTEGER,
             stop_reason TEXT, error_message TEXT, input_tokens INTEGER, output_tokens INTEGER,
             cache_read_tokens INTEGER, cache_write_tokens INTEGER, total_tokens INTEGER,
             premium_requests INTEGER, cost_input REAL, cost_output REAL, cost_cache_read REAL,
             cost_cache_write REAL, cost_total REAL, cost_no_cache_input REAL, cost_unpriced REAL,
             agent_type TEXT);",
        )
        .unwrap();
        con
    }

    #[test]
    fn parses_rows_and_reported_cost() {
        let con = setup();
        con.execute(
            "INSERT INTO messages (model, timestamp, input_tokens, output_tokens, cache_read_tokens,
             cache_write_tokens, total_tokens, cost_total, duration, ttft) VALUES
             ('deepseek/deepseek-v4.1-flash', 1790175742434, 204, 199, 51712, 0, 52115, 0.000305136, 12000, 800),
             ('muse-spark-1.3', 1790175736189, 487, 303, 51072, 0, 51862, NULL, NULL, NULL),
             ('', NULL, 0, 0, 0, 0, 0, 0.0, NULL, NULL)",
            [],
        )
        .unwrap();
        let recs = collect_conn(&con).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].model, "deepseek/deepseek-v4.1-flash");
        assert_eq!(recs[0].ts_ms, 1_790_175_742_434);
        assert_eq!(recs[0].tokens.cache_read, 51_712);
        assert_eq!(recs[0].cost, Some(0.000305136));
        assert!(!recs[0].cost_is_estimate);
        assert_eq!(recs[0].duration_ms, 12_000);
        assert_eq!(recs[0].ttft_ms, 800);
        assert_eq!(recs[1].cost, None);
        assert_eq!(recs[1].duration_ms, 0);
        // 全零行被跳过
    }

    #[test]
    fn legacy_schema_without_timing_columns_still_parses() {
        let con = Connection::open_in_memory().unwrap();
        con.execute_batch(
            "CREATE TABLE messages (id TEXT, session_file TEXT, entry_id TEXT, folder TEXT,
             model TEXT, provider TEXT, api TEXT, timestamp INTEGER,
             stop_reason TEXT, error_message TEXT, input_tokens INTEGER, output_tokens INTEGER,
             cache_read_tokens INTEGER, cache_write_tokens INTEGER, total_tokens INTEGER,
             premium_requests INTEGER, cost_input REAL, cost_output REAL, cost_cache_read REAL,
             cost_cache_write REAL, cost_total REAL, cost_no_cache_input REAL, cost_unpriced REAL,
             agent_type TEXT);",
        )
        .unwrap();
        con.execute(
            "INSERT INTO messages (model, timestamp, input_tokens, output_tokens,
             cache_read_tokens, cache_write_tokens, total_tokens, cost_total) VALUES
             ('deepseek/deepseek-v4.1-flash', 1790175742434, 204, 199, 51712, 0, 52115, NULL)",
            [],
        )
        .unwrap();
        let recs = collect_conn(&con).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].tokens.input, 204);
        assert_eq!(recs[0].duration_ms, 0);
        assert_eq!(recs[0].ttft_ms, 0);
    }

    #[test]
    fn missing_table_is_a_named_error() {
        let con = Connection::open_in_memory().unwrap();
        let err = collect_conn(&con).unwrap_err();
        assert!(err.contains("messages"), "{err}");
    }
}
