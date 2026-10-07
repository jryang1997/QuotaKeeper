//! 冒烟:列出本机账本里无法估价的模型(按缺价 token 量降序).仅用于开发验证.
//!
//! 与 app 同路径:真实采集 → Pricing 缓存估价 → 仍 None 的按 canonical 模型汇总.

use std::collections::HashMap;

use chrono::Local;
use tui_token_monitor::ledger::{self, claude, codex, dsh, omp, opencode, pricing, zcode};
use tui_token_monitor::paths;

fn main() {
    let home = paths::home_dir();
    let mut records: Vec<ledger::UsageRecord> = Vec::new();

    for (scan, parse) in [
        (codex::scan_paths as fn(&std::path::Path) -> Vec<std::path::PathBuf>, codex::parse_file as fn(&std::path::Path) -> std::io::Result<Vec<ledger::UsageRecord>>),
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
        (opencode::db_path(&home), opencode::collect as fn(&std::path::Path) -> Result<Vec<ledger::UsageRecord>, String>),
        (zcode::db_path(&home), zcode::collect),
    ] {
        if let Ok(rs) = run(&path) {
            records.extend(rs);
        }
    }

    let pricing = pricing::Pricing::load(&home);
    for r in records.iter_mut() {
        if r.cost.is_none()
            && let Some(c) = pricing.estimate(&r.model, &r.tokens)
        {
            r.cost = Some(c);
            r.cost_is_estimate = true;
        }
    }

    // canonical 模型 → (缺价 messages, 缺价 tokens, 总 messages, 总 tokens, 客户端集合, 原始名样例, 缺价记录的桶拆分)
    type ModelStat = (
        u64,
        i64,
        u64,
        i64,
        std::collections::BTreeSet<&'static str>,
        Vec<String>,
        [i64; 4],
    );
    let mut models: HashMap<String, ModelStat> = HashMap::new();
    let now_ms = Local::now().timestamp_millis();
    for r in records.iter().filter(|r| r.ts_ms > 0 && r.ts_ms <= now_ms) {
        let key = ledger::model_alias::canonical(&r.model);
        let e = models.entry(key).or_default();
        e.3 += r.tokens.total();
        e.2 += 1;
        if r.cost.is_none() {
            e.0 += 1;
            e.1 += r.tokens.total();
            e.6 = [
                e.6[0] + r.tokens.input,
                e.6[1] + r.tokens.output,
                e.6[2] + r.tokens.cache_read,
                e.6[3] + r.tokens.cache_write,
            ];
        }
        e.4.insert(r.client);
        if e.5.len() < 3 && !e.5.iter().any(|n| n == &r.model) {
            e.5.push(r.model.clone());
        }
    }

    let mut rows: Vec<_> = models.into_iter().filter(|(_, e)| e.0 > 0).collect();
    rows.sort_by_key(|(_, e)| std::cmp::Reverse(e.1));
    let total_tok: i64 = rows.iter().map(|(_, e)| e.3).sum();
    let unpriced_tok: i64 = rows.iter().map(|(_, e)| e.1).sum();
    if rows.is_empty() {
        println!("全部模型均可估价 ✓");
        return;
    }
    println!(
        "无法估价的模型 {} 个 · 缺价 {} tok / 总 {} tok({:.1}%)",
        rows.len(),
        unpriced_tok,
        total_tok,
        100.0 * unpriced_tok as f64 / total_tok.max(1) as f64
    );
    for (model, (umsg, utok, msg, _tok, clients, raws, buckets)) in rows {
        println!(
            "  {model:<38} 缺价 {utok:>10} tok({umsg}/{msg} 条) · {clients:?} · 记录名 {raws:?}"
        );
        println!(
            "      缺价桶 in={} out={} cache_read={} cache_write={}",
            buckets[0], buckets[1], buckets[2], buckets[3]
        );
    }
}
