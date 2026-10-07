//! 冒烟:按客户端汇总本机账本(总记录/token、近 7 天记录/token).
//! 与 app 同路径:真实采集 → 估价 → 分组汇总。用于核对"某客户端没统计"类问题.

use std::collections::HashMap;

use chrono::{Local, TimeZone};
use quotakeeper::ledger::{self, claude, codex, dsh, omp, opencode, zcode};

fn main() {
    let home = paths_home();
    let mut records: Vec<ledger::UsageRecord> = Vec::new();

    for (scan, parse) in [
        (
            codex::scan_paths as fn(&std::path::Path) -> Vec<std::path::PathBuf>,
            codex::parse_file as fn(&std::path::Path) -> std::io::Result<Vec<ledger::UsageRecord>>,
        ),
        (claude::scan_paths, claude::parse_file),
        (dsh::scan_paths, dsh::parse_file),
        (omp::scan_paths, omp::parse_file),
    ] {
        for p in scan(&home) {
            if let Ok(rs) = parse(&p) {
                records.extend(rs);
            }
        }
    }
    for (path, run) in [
        (
            opencode::db_path(&home),
            opencode::collect as fn(&std::path::Path) -> Result<Vec<ledger::UsageRecord>, String>,
        ),
        (zcode::db_path(&home), zcode::collect),
    ] {
        match run(&path) {
            Ok(rs) => records.extend(rs),
            Err(e) => println!("[error] {} : {e}", path.display()),
        }
    }

    let now_ms = Local::now().timestamp_millis();
    let week_ms = 7 * 24 * 3_600 * 1_000;

    // client → (records, tokens, 7d records, 7d tokens, earliest ts, latest ts)
    type Stat = (u64, i64, u64, i64, i64, i64);
    let mut by_client: HashMap<&'static str, Stat> = HashMap::new();
    for r in records.iter().filter(|r| r.ts_ms > 0 && r.ts_ms <= now_ms) {
        let e = by_client.entry(r.client).or_default();
        e.0 += 1;
        e.1 += r.tokens.total();
        if r.ts_ms > now_ms - week_ms {
            e.2 += 1;
            e.3 += r.tokens.total();
        }
        e.4 = if e.4 == 0 { r.ts_ms } else { e.4.min(r.ts_ms) };
        e.5 = e.5.max(r.ts_ms);
    }

    let fmt_ts = |ms: i64| {
        Local
            .timestamp_millis_opt(ms)
            .single()
            .map(|d| d.format("%m-%d %H:%M").to_string())
            .unwrap_or_default()
    };
    let mut rows: Vec<_> = by_client.into_iter().collect();
    rows.sort_by_key(|(_, e)| std::cmp::Reverse(e.1));
    println!(
        "{:<8} {:>9} {:>14} {:>9} {:>14}  {:<11} {:<11}",
        "client", "records", "tokens", "7d recs", "7d tokens", "earliest", "latest"
    );
    for (client, (n, tok, n7, tok7, lo, hi)) in rows {
        println!(
            "{:<8} {:>9} {:>14} {:>9} {:>14}  {:<11} {:<11}",
            client,
            n,
            tok,
            n7,
            tok7,
            fmt_ts(lo),
            fmt_ts(hi)
        );
    }
}

fn paths_home() -> std::path::PathBuf {
    quotakeeper::paths::home_dir()
}
