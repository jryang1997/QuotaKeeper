//! 本地 token 账本:采集(6 个 collector)→ 估价 → 聚合(今日/本周/本月 × 按Agent × 每日).
//!
//! - JSONL/zstd 源(codex/claude/dsh)带进程级文件缓存:mtime+size 未变的文件直接复用
//!   上次解析结果,60s 轮询下不重读;
//! - SQLite 源(omp/opencode/zcode)每轮只读 SELECT,量小且始终新鲜;
//! - 单个源失败不影响其他源,错误进 `SourceStatus` 在 UI 展示;
//! - 口径归一化见 types.rs;周 = ISO 周一至今,月 = 自然月,日期按本地时区.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Timelike};

pub mod agentenv;
pub mod claude;
pub mod codex;
pub mod dsh;
pub mod model_alias;
pub mod omp;
pub mod opencode;
pub mod pricing;
pub mod types;
pub mod zcode;

pub use types::{Agg, SpeedStats, TokenBreakdown, UsageRecord, client_label};

use types::CLIENTS;

pub struct SourceStatus {
    pub client: &'static str,
    pub records: usize,
    pub error: Option<String>,
}

pub struct DayRow {
    pub date: String,
    pub agg: Agg,
    pub per_client: Vec<(&'static str, Agg)>,
}

pub struct AgentRow {
    pub client: &'static str,
    pub agg: Agg,
    /// 近 30 天该 agent 的按模型拆分,总量降序.
    pub models: Vec<(String, Agg)>,
}

/// 近 30 天按模型汇总,总量降序;同一模型的不同记录名(大小写/前缀/别名)
/// 经 model_alias::canonical 合并到一行,model 存 canonical 名.
pub struct ModelRow {
    pub model: String,
    /// 合并前的原始记录名(去重,按出现序)。
    pub aliases: Vec<String>,
    pub clients: Vec<&'static str>,
    pub agg: Agg,
    /// 有计时样本时的实测速度(TTFT / tok/s);无样本为 None.
    pub speed: Option<SpeedStats>,
}

/// 最近一小时按 Agent 汇总，仅代表本机记录到的活动。
pub struct RecentAgentRow {
    pub client: &'static str,
    pub agg: Agg,
    pub last_ts: i64,
}

/// 过去 24 小时连续的一小时桶，包含空桶。
pub struct TrendBucket {
    pub start_ms: i64,
    pub agg: Agg,
    pub per_client: Vec<(&'static str, Agg)>,
}

/// 热力图单日合计.
pub struct DailyTotal {
    pub date: String,
    pub tokens: i64,
    pub cost: f64,
    /// 当日请求数(GUI 月度 RUNS minibar 用;TUI 热力图不消费).
    pub messages: u64,
    /// 当日完整聚合(input/output/cache 五桶+成本),总览页分段条与周期聚合用.
    pub agg: Agg,
}

/// 可筛选的每日账本。保留来源、项目和原始模型维度，供任意日期分析及导出使用。
pub struct AnalysisRow {
    pub date: String,
    pub client: &'static str,
    pub project: Option<String>,
    pub model: String,
    pub raw_model: String,
    pub agg: Agg,
    pub unpriced_messages: u64,
}

/// 今天与昨天按本地整点拆分，供单日趋势图按两小时绘制。
pub struct IntradayRow {
    pub date: String,
    pub hour: u8,
    pub client: &'static str,
    pub model: String,
    pub raw_model: String,
    pub agg: Agg,
    pub unpriced_messages: u64,
}

/// 会话维度行(近 30 天,按最后活跃排序).
pub struct SessionRow {
    pub client: &'static str,
    pub session: String,
    pub project: Option<String>,
    pub agg: Agg,
    pub last_ts: i64,
}

/// 项目维度行(近 30 天,按总量排序).
pub struct ProjectRow {
    pub project: String,
    pub agg: Agg,
    pub sessions: usize,
    pub last_ts: i64,
}

pub const HEATMAP_WEEKS: usize = 26;

pub struct LedgerReport {
    pub sources: Vec<SourceStatus>,
    pub today: Agg,
    pub week: Agg,
    pub month: Agg,
    /// 当月按 client 聚合,token 降序,只含有数据的(GUI 套餐卡「MONTHLY」行).
    pub month_by_client: Vec<(&'static str, Agg)>,
    /// 近 30 天按 agent,总量降序,只含有数据的.
    pub by_agent: Vec<AgentRow>,
    /// 近 30 天按模型,总量降序,同名模型跨 Agent 合并.
    pub by_model: Vec<ModelRow>,
    /// 最近一小时按 token 降序。
    pub recent_by_agent: Vec<RecentAgentRow>,
    /// 最后一条有时间戳的本机记录，可能早于一小时。
    pub last_activity_ms: Option<i64>,
    /// 过去 24 小时的连续滚动小时桶，旧 → 新。
    pub trend_24h: Vec<TrendBucket>,
    /// 近 14 天,旧 → 新,空日也保留.
    pub by_day: Vec<DayRow>,
    /// 近 26 周每日合计,起点为周一(HEATMAP_WEEKS*7 天,旧 → 新,空日 tokens=0).
    pub daily_totals: Vec<DailyTotal>,
    /// 近 26 周内有数据日期的按 agent 拆分(热力图点击下钻用).
    pub daily_detail: HashMap<String, Vec<(&'static str, Agg)>>,
    pub analysis_rows: Vec<AnalysisRow>,
    pub intraday_rows: Vec<IntradayRow>,
    /// 近 30 天按本地小时(0-23)的 token 合计.
    pub hourly: [i64; 24],
    /// 近 30 天按星期(周一=0)的 token 合计.
    pub weekday: [i64; 7],
    /// 近 30 天「星期×小时」token 矩阵(周一=0 行,0-23 列;面板 punchcard 用).
    pub weekday_hour: [[i64; 24]; 7],
    /// 近 30 天会话维度,按最后活跃降序,最多 40 条.
    pub by_session: Vec<SessionRow>,
    /// 近 30 天项目维度,按总量降序,最多 20 条.
    pub by_project: Vec<ProjectRow>,
    /// 燃烧率:近 60 分钟 token/分钟;无近期活动为 None.
    pub burn_rate: Option<f64>,
    pub generated_at_ms: i64,
}

pub fn collect(now: DateTime<Local>) -> LedgerReport {
    let home = crate::paths::home_dir();
    let mut records: Vec<UsageRecord> = Vec::new();
    let mut sources = Vec::new();
    let mut active_files = HashSet::new();

    // JSONL/zstd 源:文件级缓存
    for (client, scan, parse) in [
        (
            codex::CLIENT,
            codex::scan_paths as fn(&Path) -> Vec<PathBuf>,
            codex::parse_file as fn(&Path) -> std::io::Result<Vec<UsageRecord>>,
        ),
        (claude::CLIENT, claude::scan_paths, claude::parse_file),
        (dsh::CLIENT, dsh::scan_paths, dsh::parse_file),
        (omp::CLIENT, omp::scan_paths, omp::parse_file),
    ] {
        let (recs, error) = load_client(&home, scan, parse, &mut active_files);
        sources.push(SourceStatus {
            client,
            records: recs.len(),
            error,
        });
        records.extend(recs);
    }
    file_cache()
        .lock()
        .unwrap()
        .retain(|path, _| active_files.contains(path));

    // SQLite 源:每轮直查
    for (client, path, run) in [
        (
            opencode::CLIENT,
            opencode::db_path(&home),
            opencode::collect as fn(&Path) -> Result<Vec<UsageRecord>, String>,
        ),
        (zcode::CLIENT, zcode::db_path(&home), zcode::collect),
    ] {
        match run(&path) {
            Ok(recs) => {
                sources.push(SourceStatus {
                    client,
                    records: recs.len(),
                    error: None,
                });
                records.extend(recs);
            }
            Err(e) => sources.push(SourceStatus {
                client,
                records: 0,
                error: Some(e),
            }),
        }
    }

    // 估价只读缓存;新价由后台线程刷新写入。
    pricing::spawn_refresher(&home);
    if records.iter().any(|r| r.cost.is_none()) {
        let pricing = pricing::Pricing::load(&home);
        for r in records.iter_mut() {
            if r.cost.is_none()
                && let Some(c) = pricing.estimate(&r.model, &r.tokens)
            {
                r.cost = Some(c);
                r.cost_is_estimate = true;
            }
        }
    }

    let report = aggregate(records, now);
    report.sources_filled(sources)
}

fn load_client(
    home: &Path,
    scan: fn(&Path) -> Vec<PathBuf>,
    parse: fn(&Path) -> std::io::Result<Vec<UsageRecord>>,
    active_files: &mut HashSet<PathBuf>,
) -> (Vec<UsageRecord>, Option<String>) {
    let paths = scan(home);
    active_files.extend(paths.iter().cloned());
    if paths.is_empty() {
        return (Vec::new(), Some("未找到会话文件".into()));
    }
    let mut out = Vec::new();
    let mut errors = Vec::new();
    let cache = file_cache();
    let mut cache = cache.lock().unwrap();
    for path in paths {
        let sig = file_sig(&path);
        match (sig, cache.get(&path)) {
            (Some(sig), Some((cached_sig, recs))) if cached_sig == &sig => {
                out.extend(recs.iter().cloned());
            }
            _ => {
                match parse(&path) {
                    Ok(recs) => {
                        if let Some(sig) = sig {
                            cache.insert(path.clone(), (sig, recs.clone()));
                        }
                        out.extend(recs);
                    }
                    Err(e) => {
                        cache.remove(&path);
                        errors.push(format!("{}: {e}", path.display()));
                    }
                }
            }
        }
    }
    let error = if errors.is_empty() {
        None
    } else {
        Some(format!("{} 个会话文件读取失败; {}", errors.len(), errors[0]))
    };
    (out, error)
}

type FileSig = (u128, u64);
type FileCache = HashMap<PathBuf, (FileSig, Vec<UsageRecord>)>;

fn file_cache() -> &'static Mutex<FileCache> {
    static CACHE: OnceLock<Mutex<FileCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn file_sig(path: &Path) -> Option<FileSig> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some((mtime, meta.len()))
}

pub fn aggregate(mut records: Vec<UsageRecord>, now: DateTime<Local>) -> LedgerReport {
    let now_ms = now.timestamp_millis();
    records.retain(|r| r.ts_ms > 0 && r.ts_ms <= now_ms);
    let today = now.date_naive();
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let month_prefix = today.format("%Y-%m").to_string();
    let today_str = today.format("%Y-%m-%d").to_string();
    let heat_start = monday - Duration::weeks((HEATMAP_WEEKS - 1) as i64);
    let mut heat_raw: HashMap<String, HashMap<&'static str, Agg>> = HashMap::new();
    let mut analysis_raw: HashMap<(String, &'static str, Option<String>, String), (Agg, u64)> =
        HashMap::new();
    let mut intraday_raw: HashMap<(String, u8, &'static str, String), (Agg, u64)> = HashMap::new();

    let mut today_agg = Agg::default();
    let mut week_agg = Agg::default();
    let mut month_agg = Agg::default();
    let mut month_by_client_raw: HashMap<&'static str, Agg> = HashMap::new();
    let mut by_agent_raw: HashMap<&'static str, (Agg, HashMap<String, Agg>)> = HashMap::new();
    // canonical 模型名 → 30 天内的原始记录名(去重)
    let mut model_aliases: HashMap<String, std::collections::BTreeSet<String>> = HashMap::new();
    let mut by_day_raw: HashMap<String, HashMap<&'static str, Agg>> = HashMap::new();
    // canonical 模型名 → 计时样本(仅近 30 天窗口内累计).
    let mut speed_raw: HashMap<String, SpeedStats> = HashMap::new();
    let mut hourly = [0i64; 24];
    let mut weekday = [0i64; 7];
    let mut weekday_hour = [[0i64; 24]; 7];
    let mut recent60 = 0i64; // 近 60 分钟 token 合计(燃烧率)
    let recent60_since = now_ms - 3_600_000;
    let trend_start = now_ms - 24 * 3_600_000;
    let mut recent_raw: HashMap<&'static str, (Agg, i64)> = HashMap::new();
    let mut last_activity_ms: Option<i64> = None;
    let mut trend_raw: Vec<HashMap<&'static str, Agg>> = vec![HashMap::new(); 24];
    let mut session_raw: HashMap<(&'static str, String), (Agg, i64, Option<String>)> =
        HashMap::new();
    type ProjectEntry = (
        Agg,
        i64,
        std::collections::HashSet<String>,
    );
    let mut project_raw: HashMap<String, ProjectEntry> = HashMap::new();

    for r in &records {
        let Some(ts) = DateTime::from_timestamp_millis(r.ts_ms) else {
            continue;
        };
        let date: NaiveDate = ts.with_timezone(&Local).date_naive();
        let date_str = date.format("%Y-%m-%d").to_string();
        if date <= today {
            let entry = analysis_raw
                .entry((date_str.clone(), r.client, r.project.clone(), r.model.clone()))
                .or_default();
            entry.0.add(&r.tokens, r.cost, r.cost_is_estimate);
            if r.cost.is_none() {
                entry.1 += 1;
            }
        }
        if date >= today - Duration::days(1) && date <= today {
            let entry = intraday_raw
                .entry((
                    date_str.clone(),
                    ts.with_timezone(&Local).hour() as u8,
                    r.client,
                    r.model.clone(),
                ))
                .or_default();
            entry.0.add(&r.tokens, r.cost, r.cost_is_estimate);
            if r.cost.is_none() {
                entry.1 += 1;
            }
        }
        if date_str == today_str {
            today_agg.add(&r.tokens, r.cost, r.cost_is_estimate);
        }
        last_activity_ms = Some(last_activity_ms.map_or(r.ts_ms, |last| last.max(r.ts_ms)));
        // 滚动小时桶与最近一小时只接纳已发生的记录。
        if r.ts_ms >= trend_start {
            let bucket = ((r.ts_ms - trend_start) / 3_600_000).min(23) as usize;
            trend_raw[bucket].entry(r.client).or_default().add(
                &r.tokens,
                r.cost,
                r.cost_is_estimate,
            );
        }
        if r.ts_ms >= recent60_since {
            recent60 += r.tokens.total();
            let entry = recent_raw.entry(r.client).or_default();
            entry.0.add(&r.tokens, r.cost, r.cost_is_estimate);
            entry.1 = entry.1.max(r.ts_ms);
        }
        if date >= monday && date <= today {
            week_agg.add(&r.tokens, r.cost, r.cost_is_estimate);
        }
        if date_str.starts_with(&month_prefix) {
            month_agg.add(&r.tokens, r.cost, r.cost_is_estimate);
            month_by_client_raw
                .entry(r.client)
                .or_default()
                .add(&r.tokens, r.cost, r.cost_is_estimate);
        }
        if date >= today - Duration::days(29) && date <= today {
            let entry = by_agent_raw.entry(r.client).or_default();
            entry.0.add(&r.tokens, r.cost, r.cost_is_estimate);
            let canonical_model = model_alias::canonical(&r.model);
            model_aliases
                .entry(canonical_model.clone())
                .or_default()
                .insert(r.model.clone());
            entry
                .1
                .entry(canonical_model.clone())
                .or_default()
                .add(&r.tokens, r.cost, r.cost_is_estimate);
            speed_raw
                .entry(canonical_model)
                .or_default()
                .sample(r);
            // 时段分布(本地小时 / 星期 / 星期×小时 punchcard)
            let local = ts.with_timezone(&Local);
            let hour_idx = local.hour() as usize;
            let weekday_idx = local.weekday().num_days_from_monday() as usize;
            let total = r.tokens.total();
            hourly[hour_idx] += total;
            weekday[weekday_idx] += total;
            weekday_hour[weekday_idx][hour_idx] += total;
            // 会话 / 项目维度
            if let Some(s) = &r.session {
                let e = session_raw.entry((r.client, s.clone())).or_default();
                e.0.add(&r.tokens, r.cost, r.cost_is_estimate);
                e.1 = e.1.max(r.ts_ms);
                if e.2.is_none() {
                    e.2 = r.project.clone();
                }
            }
            if let Some(p) = &r.project {
                let e = project_raw.entry(p.clone()).or_insert_with(|| {
                    (
                        Agg::default(),
                        0,
                        std::collections::HashSet::new(),
                    )
                });
                e.0.add(&r.tokens, r.cost, r.cost_is_estimate);
                e.1 = e.1.max(r.ts_ms);
                if let Some(s) = &r.session {
                    e.2.insert(s.clone());
                }
            }
        }
        if date > today - Duration::days(14) && date <= today {
            by_day_raw
                .entry(date_str.clone())
                .or_default()
                .entry(r.client)
                .or_default()
                .add(&r.tokens, r.cost, r.cost_is_estimate);
        }
        if date >= heat_start && date <= today {
            heat_raw
                .entry(date_str)
                .or_default()
                .entry(r.client)
                .or_default()
                .add(&r.tokens, r.cost, r.cost_is_estimate);
        }
    }

    let by_agent: Vec<AgentRow> = CLIENTS
        .iter()
        .filter_map(|(client, _)| {
            let (agg, models) = by_agent_raw.remove(client)?;
            if agg.total() == 0 && agg.messages == 0 {
                return None;
            }
            let mut models: Vec<(String, Agg)> = models.into_iter().collect();
            models.sort_by_key(|(_, a)| std::cmp::Reverse(a.total()));
            Some(AgentRow {
                client,
                agg,
                models,
            })
        })
        .collect();

    let mut by_model_raw: HashMap<String, (Agg, std::collections::HashSet<&'static str>)> =
        HashMap::new();
    for agent in &by_agent {
        for (model, agg) in &agent.models {
            let entry = by_model_raw.entry(model.clone()).or_default();
            entry.0.merge(agg);
            entry.1.insert(agent.client);
        }
    }
    let mut by_model: Vec<ModelRow> = by_model_raw
        .into_iter()
        .map(|(model, (agg, clients))| {
            let aliases = model_aliases
                .remove(&model)
                .map(|set| set.into_iter().collect())
                .unwrap_or_default();
            let speed = speed_raw.remove(&model).filter(|s| s.timed_calls > 0);
            ModelRow {
                model,
                aliases,
                clients: CLIENTS
                    .iter()
                    .filter_map(|(client, _)| clients.contains(client).then_some(*client))
                    .collect(),
                agg,
                speed,
            }
        })
        .collect();
    by_model.sort_by(|a, b| {
        b.agg
            .total()
            .cmp(&a.agg.total())
            .then_with(|| a.model.cmp(&b.model))
    });

    let mut recent_by_agent: Vec<RecentAgentRow> = recent_raw
        .into_iter()
        .map(|(client, (agg, last_ts))| RecentAgentRow {
            client,
            agg,
            last_ts,
        })
        .collect();
    recent_by_agent.sort_by(|a, b| {
        b.agg
            .total()
            .cmp(&a.agg.total())
            .then(b.last_ts.cmp(&a.last_ts))
    });

    let trend_24h: Vec<TrendBucket> = trend_raw
        .into_iter()
        .enumerate()
        .map(|(i, mut raw)| {
            let per_client: Vec<(&'static str, Agg)> = CLIENTS
                .iter()
                .filter_map(|(client, _)| raw.remove(client).map(|agg| (*client, agg)))
                .collect();
            let mut agg = Agg::default();
            for (_, value) in &per_client {
                agg.merge(value);
            }
            TrendBucket {
                start_ms: trend_start + i as i64 * 3_600_000,
                agg,
                per_client,
            }
        })
        .collect();

    let mut by_day: Vec<DayRow> = Vec::new();
    for i in (0..14).rev() {
        let date = today - Duration::days(i);
        let date_str = date.format("%Y-%m-%d").to_string();
        let per_client: Vec<(&'static str, Agg)> = CLIENTS
            .iter()
            .filter_map(|(client, _)| {
                by_day_raw
                    .get(&date_str)
                    .and_then(|m| m.get(client))
                    .map(|agg| (*client, agg.clone()))
            })
            .collect();
        let mut agg = Agg::default();
        for (_, a) in per_client.iter() {
            agg.merge(a);
        }
        by_day.push(DayRow {
            date: date_str,
            agg,
            per_client,
        });
    }

    // 热力图数据:26 周 × 7 天网格,起点为周一;空日 tokens=0;有数据日按 agent 拆分.
    let mut daily_totals = Vec::with_capacity(HEATMAP_WEEKS * 7);
    let mut daily_detail: HashMap<String, Vec<(&'static str, Agg)>> = HashMap::new();
    for i in 0..HEATMAP_WEEKS * 7 {
        let date = heat_start + Duration::days(i as i64);
        let date_str = date.format("%Y-%m-%d").to_string();
        let per = heat_raw.remove(&date_str).unwrap_or_default();
        let detail: Vec<(&'static str, Agg)> = CLIENTS
            .iter()
            .filter_map(|(client, _)| per.get(client).map(|a| (*client, a.clone())))
            .collect();
        let mut sum = Agg::default();
        for (_, a) in &detail {
            sum.merge(a);
        }
        daily_totals.push(DailyTotal {
            date: date_str.clone(),
            tokens: sum.total(),
            cost: sum.cost(),
            messages: sum.messages,
            agg: sum.clone(),
        });
        if sum.total() > 0 {
            daily_detail.insert(date_str, detail);
        }
    }

    // 会话维度:按最后活跃降序
    let mut by_session: Vec<SessionRow> = session_raw
        .into_iter()
        .map(|((client, session), (agg, last_ts, project))| SessionRow {
            client,
            session,
            project,
            agg,
            last_ts,
        })
        .collect();
    by_session.sort_by_key(|s| std::cmp::Reverse(s.last_ts));
    by_session.truncate(40);

    // 项目维度:按总量降序
    let mut by_project: Vec<ProjectRow> = project_raw
        .into_iter()
        .map(|(project, (agg, last_ts, sessions))| ProjectRow {
            project,
            agg,
            sessions: sessions.len(),
            last_ts,
        })
        .collect();
    by_project.sort_by(|a, b| {
        b.agg
            .total()
            .cmp(&a.agg.total())
            .then(b.last_ts.cmp(&a.last_ts))
    });
    by_project.truncate(20);

    let mut analysis_rows: Vec<AnalysisRow> = analysis_raw
        .into_iter()
        .map(
            |((date, client, project, raw_model), (agg, unpriced_messages))| AnalysisRow {
                date,
                client,
                project,
                model: model_alias::canonical(&raw_model),
                raw_model,
                agg,
                unpriced_messages,
            },
        )
        .collect();
    analysis_rows.sort_by(|a, b| {
        (&a.date, a.client, &a.project, &a.raw_model).cmp(&(
            &b.date,
            b.client,
            &b.project,
            &b.raw_model,
        ))
    });

    let mut intraday_rows: Vec<IntradayRow> = intraday_raw
        .into_iter()
        .map(
            |((date, hour, client, raw_model), (agg, unpriced_messages))| IntradayRow {
                date,
                hour,
                client,
                model: model_alias::canonical(&raw_model),
                raw_model,
                agg,
                unpriced_messages,
            },
        )
        .collect();
    intraday_rows.sort_by(|a, b| {
        (&a.date, a.hour, a.client, &a.raw_model).cmp(&(&b.date, b.hour, b.client, &b.raw_model))
    });

    let mut month_by_client: Vec<(&'static str, Agg)> = month_by_client_raw.into_iter().collect();
    month_by_client.sort_by(|a, b| b.1.total().cmp(&a.1.total()));

    LedgerReport {
        sources: Vec::new(),
        today: today_agg,
        week: week_agg,
        month: month_agg,
        month_by_client,
        by_agent,
        by_model,
        recent_by_agent,
        last_activity_ms,
        trend_24h,
        by_day,
        daily_totals,
        daily_detail,
        analysis_rows,
        intraday_rows,
        hourly,
        weekday,
        weekday_hour,
        by_session,
        by_project,
        burn_rate: if recent60 > 0 {
            Some(recent60 as f64 / 60.0)
        } else {
            None
        },
        generated_at_ms: now.timestamp_millis(),
    }
}

impl LedgerReport {
    fn sources_filled(mut self, sources: Vec<SourceStatus>) -> Self {
        self.sources = sources;
        self
    }
}

/// "$1.23" / "$0.4567" / "$123"
pub fn format_usd(v: f64) -> String {
    if v >= 100.0 {
        format!("${v:.0}")
    } else if v >= 1.0 {
        format!("${v:.2}")
    } else {
        format!("${v:.4}")
    }
}

/// "12.3k" / "1.2M" / "856"
pub fn format_tokens(v: i64) -> String {
    let v = v as f64;
    if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.1}k", v / 1_000.0)
    } else {
        format!("{v:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn rec(client: &'static str, model: &str, ts_ms: i64, tokens: TokenBreakdown) -> UsageRecord {
        UsageRecord {
            client,
            model: model.into(),
            ts_ms,
            tokens,
            cost: Some(0.5),
            cost_is_estimate: false,
            session: None,
            project: None,
            duration_ms: 0,
            ttft_ms: 0,
        }
    }

    #[test]
    fn model_speed_aggregates_timed_samples_only() {
        let n = now();
        let mut timed = rec(
            "claude",
            "claude-opus-4",
            n.timestamp_millis() - 60_000,
            TokenBreakdown {
                output: 900,
                ..Default::default()
            },
        );
        timed.duration_ms = 9_000;
        timed.ttft_ms = 3_000;
        let mut timed2 = timed.clone();
        timed2.duration_ms = 11_000;
        timed2.ttft_ms = 5_000;
        timed2.model = "Claude-Opus-4".into(); // 别名合并到同一 canonical 行
        // 有时长但首字被上限裁掉的样本:不得稀释 ttft 均值.
        let mut no_ttft = timed.clone();
        no_ttft.duration_ms = 10_000;
        no_ttft.ttft_ms = 0;
        let untimed = rec(
            "codex",
            "claude-opus-4",
            n.timestamp_millis() - 30_000,
            TokenBreakdown {
                output: 5,
                ..Default::default()
            },
        );
        let rep = aggregate(vec![timed, timed2, no_ttft, untimed], n);
        let row = rep.by_model.iter().find(|m| m.model == "claude-opus-4").unwrap();
        let speed = row.speed.clone().expect("有计时样本");
        assert_eq!(speed.timed_calls, 3);
        assert_eq!(speed.ttft_samples, 2);
        assert_eq!(speed.duration_ms_total, 30_000);
        assert_eq!(speed.ttft_ms_total, 8_000);
        assert_eq!(speed.output_total, 2_700);
        // 均值只对 ttft 样本取:8000/2 = 4000,不被第三次稀释
        assert!((speed.avg_ttft_ms().unwrap() - 4_000.0).abs() < 1e-9);
        assert!((speed.tok_per_sec().unwrap() - 90.0).abs() < 1e-9);
    }

    #[test]
    fn model_speed_is_none_without_timed_samples() {
        let n = now();
        let rep = aggregate(
            vec![rec(
                "codex",
                "m",
                n.timestamp_millis(),
                TokenBreakdown {
                    output: 5,
                    ..Default::default()
                },
            )],
            n,
        );
        assert!(rep.by_model[0].speed.is_none());
    }

    #[test]
    fn file_read_error_keeps_other_records_and_reports_the_failure() {
        fn scan(home: &Path) -> Vec<PathBuf> {
            vec![
                home.join("session-good.jsonl.zstd"),
                home.join("session-bad.jsonl.zstd"),
            ]
        }

        let home = std::env::temp_dir().join(format!(
            "token-monitor-ledger-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&home).unwrap();
        let good = br#"{"type":"assistant/message","time":1000,"data":{"usage":{"inputTokens":5}}}"#;
        std::fs::write(
            home.join("session-good.jsonl.zstd"),
            zstd::encode_all(good.as_slice(), 0).unwrap(),
        )
        .unwrap();
        std::fs::write(home.join("session-bad.jsonl.zstd"), b"bad zstd").unwrap();

        let (records, error) = load_client(&home, scan, dsh::parse_file, &mut HashSet::new());
        std::fs::remove_file(home.join("session-good.jsonl.zstd")).unwrap();
        std::fs::remove_file(home.join("session-bad.jsonl.zstd")).unwrap();
        std::fs::remove_dir(&home).unwrap();

        assert_eq!(records.len(), 1);
        let error = error.expect("failed file must be reported");
        assert!(error.contains("1 个会话文件读取失败"));
        assert!(error.contains("session-bad.jsonl.zstd"));
    }

    #[test]
    fn missing_session_files_are_parse_errors() {
        let missing = Path::new("definitely-missing-session-file.jsonl");
        assert!(codex::parse_file(missing).is_err());
        assert!(claude::parse_file(missing).is_err());
        assert!(dsh::parse_file(missing).is_err());
    }

    #[test]
    fn analysis_rows_preserve_project_and_missing_price() {
        let now = Local.with_ymd_and_hms(2026, 9, 27, 12, 0, 0).unwrap();
        let mut first = rec(
            "codex",
            "model",
            now.timestamp_millis(),
            TokenBreakdown {
                input: 10,
                ..Default::default()
            },
        );
        first.project = Some("alpha".into());
        first.cost = None;
        let mut second = first.clone();
        second.project = Some("beta".into());
        second.cost = Some(1.25);
        let report = aggregate(vec![first, second], now);
        assert_eq!(report.analysis_rows.len(), 2);
        let alpha = report
            .analysis_rows
            .iter()
            .find(|row| row.project.as_deref() == Some("alpha"))
            .unwrap();
        assert_eq!(alpha.agg.total(), 10);
        assert_eq!(alpha.unpriced_messages, 1);
        let beta = report
            .analysis_rows
            .iter()
            .find(|row| row.project.as_deref() == Some("beta"))
            .unwrap();
        assert_eq!(beta.agg.cost_reported, 1.25);
        assert_eq!(beta.unpriced_messages, 0);
    }

    #[test]
    fn model_rows_keep_two_models_for_one_client_and_day() {
        let n = now();
        let records = ["openai/gpt-5", "anthropic/claude-opus-4"]
            .into_iter()
            .map(|model| {
                let mut record = rec(
                    "codex",
                    model,
                    n.timestamp_millis(),
                    TokenBreakdown {
                        input: 10,
                        ..Default::default()
                    },
                );
                record.project = Some("alpha".into());
                record
            })
            .collect();
        let report = aggregate(records, n);

        assert_eq!(report.analysis_rows.len(), 2);
        assert_eq!(report.intraday_rows.len(), 2);
        assert_eq!(report.today.total(), 20);
        for row in &report.analysis_rows {
            assert_eq!(row.project.as_deref(), Some("alpha"));
            assert!(matches!(
                (row.model.as_str(), row.raw_model.as_str()),
                ("gpt-5", "openai/gpt-5") | ("claude-opus-4", "anthropic/claude-opus-4")
            ));
        }
        for row in &report.intraday_rows {
            assert!(matches!(
                (row.model.as_str(), row.raw_model.as_str()),
                ("gpt-5", "openai/gpt-5") | ("claude-opus-4", "anthropic/claude-opus-4")
            ));
        }
    }

    #[test]
    fn model_rows_preserve_raw_channels_clients_and_cost_parts() {
        let n = now();
        let mut reported = rec(
            "codex",
            "OpenAI/GPT-5",
            n.timestamp_millis(),
            TokenBreakdown {
                input: 10,
                output: 20,
                cache_read: 30,
                cache_write: 40,
                reasoning: 5,
            },
        );
        reported.project = Some("alpha".into());
        reported.cost = Some(1.25);
        let mut estimated = reported.clone();
        estimated.model = "openrouter/gpt-5".into();
        estimated.tokens = TokenBreakdown {
            input: 1,
            output: 2,
            cache_read: 3,
            cache_write: 4,
            reasoning: 1,
        };
        estimated.cost = Some(0.125);
        estimated.cost_is_estimate = true;
        let mut unpriced = reported.clone();
        unpriced.tokens = estimated.tokens;
        unpriced.cost = None;
        let mut other_client = reported.clone();
        other_client.client = "claude";
        other_client.tokens = TokenBreakdown {
            input: 100,
            output: 200,
            cache_read: 300,
            cache_write: 400,
            reasoning: 50,
        };
        other_client.cost = Some(2.5);
        let mut other_project = reported.clone();
        other_project.project = Some("beta".into());
        other_project.tokens = TokenBreakdown {
            input: 7,
            output: 8,
            cache_read: 9,
            cache_write: 10,
            reasoning: 2,
        };
        other_project.cost = Some(0.0);
        let report = aggregate(
            vec![reported, estimated, unpriced, other_client, other_project],
            n,
        );

        let expected = Agg {
            input: 119,
            output: 232,
            cache_read: 345,
            cache_write: 458,
            reasoning: 59,
            messages: 5,
            cost_reported: 3.75,
            cost_estimated: 0.125,
        };
        assert_eq!(report.today, expected);
        assert_eq!(report.today.total(), 1_154);
        assert_eq!(report.analysis_rows.len(), 4);
        assert_eq!(report.intraday_rows.len(), 3);
        let daily = report
            .analysis_rows
            .iter()
            .fold(Agg::default(), |mut sum, row| {
                assert_eq!(row.model, "gpt-5");
                sum.merge(&row.agg);
                sum
            });
        let hourly = report
            .intraday_rows
            .iter()
            .fold(Agg::default(), |mut sum, row| {
                assert_eq!(row.model, "gpt-5");
                sum.merge(&row.agg);
                sum
            });
        assert_eq!(daily, expected);
        assert_eq!(hourly, expected);
        assert_eq!(
            report
                .analysis_rows
                .iter()
                .map(|r| r.unpriced_messages)
                .sum::<u64>(),
            1
        );
        assert_eq!(
            report
                .intraday_rows
                .iter()
                .map(|r| r.unpriced_messages)
                .sum::<u64>(),
            1
        );

        let alpha = report
            .analysis_rows
            .iter()
            .find(|row| {
                row.client == "codex"
                    && row.project.as_deref() == Some("alpha")
                    && row.raw_model == "OpenAI/GPT-5"
            })
            .unwrap();
        assert_eq!(
            alpha.agg,
            Agg {
                input: 11,
                output: 22,
                cache_read: 33,
                cache_write: 44,
                reasoning: 6,
                messages: 2,
                cost_reported: 1.25,
                cost_estimated: 0.0,
            }
        );
        assert_eq!(alpha.unpriced_messages, 1);
        let channel = report
            .analysis_rows
            .iter()
            .find(|row| row.raw_model == "openrouter/gpt-5")
            .unwrap();
        assert_eq!(channel.agg.cost_reported, 0.0);
        assert_eq!(channel.agg.cost_estimated, 0.125);
        let beta = report
            .analysis_rows
            .iter()
            .find(|row| row.project.as_deref() == Some("beta"))
            .unwrap();
        assert_eq!(beta.agg.cost(), 0.0);
        assert_eq!(beta.unpriced_messages, 0);
        let claude = report
            .intraday_rows
            .iter()
            .find(|row| row.client == "claude")
            .unwrap();
        assert_eq!(claude.raw_model, "OpenAI/GPT-5");
        assert_eq!(claude.agg.cost_reported, 2.5);
    }

    #[test]
    fn model_rows_keep_unknown_names_and_all_retained_history() {
        let n = now();
        let mut empty = rec(
            "codex",
            "",
            n.timestamp_millis(),
            TokenBreakdown {
                input: 11,
                ..Default::default()
            },
        );
        empty.cost = None;
        let mut whitespace = rec(
            "codex",
            " \t ",
            n.timestamp_millis(),
            TokenBreakdown {
                output: 13,
                ..Default::default()
            },
        );
        whitespace.cost = Some(0.0);
        let old = rec(
            "codex",
            "OpenAI/GPT-5",
            ts(2025, 1, 1, 12),
            TokenBreakdown {
                input: 17,
                ..Default::default()
            },
        );
        let report = aggregate(vec![empty, whitespace, old], n);

        assert_eq!(report.analysis_rows.len(), 3);
        let old = &report.analysis_rows[0];
        assert_eq!(old.date, "2025-01-01");
        assert_eq!(old.model, "gpt-5");
        assert_eq!(old.raw_model, "OpenAI/GPT-5");
        assert_eq!(old.agg.total(), 17);
        assert_eq!(old.agg.cost_reported, 0.5);
        assert_eq!(report.today.total(), 24);
        assert_eq!(report.by_model.len(), 1);
        assert_eq!(report.by_model[0].model, "unknown");
        assert_eq!(report.by_model[0].agg.total(), 24);
        assert_eq!(report.intraday_rows.len(), 2);
        for raw in ["", " \t "] {
            let daily = report
                .analysis_rows
                .iter()
                .find(|row| row.raw_model == raw)
                .unwrap();
            let hourly = report
                .intraday_rows
                .iter()
                .find(|row| row.raw_model == raw)
                .unwrap();
            assert_eq!(daily.model, "unknown");
            assert_eq!(hourly.model, "unknown");
            assert_eq!(daily.agg, hourly.agg);
            assert_eq!(daily.unpriced_messages, u64::from(raw.is_empty()));
            assert_eq!(hourly.unpriced_messages, daily.unpriced_messages);
        }
        assert_eq!(
            report
                .analysis_rows
                .iter()
                .map(|r| r.agg.total())
                .sum::<i64>(),
            41
        );
    }

    #[test]
    fn model_rows_keep_local_midnight_windows_and_drop_invalid_timestamps() {
        let n = now();
        let tokens = TokenBreakdown {
            input: 1,
            output: 2,
            cache_read: 3,
            cache_write: 4,
            reasoning: 1,
        };
        let yesterday_at = Local.with_ymd_and_hms(2026, 9, 24, 23, 59, 59).unwrap();
        let mut yesterday = rec(
            "codex",
            "OpenAI/GPT-5",
            yesterday_at.timestamp_millis(),
            tokens,
        );
        yesterday.cost = Some(1.0);
        let mut midnight = rec("codex", "OpenAI/GPT-5", ts(2026, 9, 25, 0), tokens);
        midnight.cost = Some(0.25);
        midnight.cost_is_estimate = true;
        let mut today = rec("omp", "", n.timestamp_millis(), tokens);
        today.cost = None;
        let older = rec("codex", "OpenAI/GPT-5", ts(2026, 9, 23, 12), tokens);
        let invalid = [0, -1, n.timestamp_millis() + 1, ts(2026, 9, 26, 0)]
            .map(|at| rec("codex", "invalid-timestamp", at, tokens));
        let records = [yesterday, midnight, today, older]
            .into_iter()
            .chain(invalid)
            .collect();
        let report = aggregate(records, n);

        assert_eq!(report.analysis_rows.len(), 4);
        assert_eq!(report.intraday_rows.len(), 3);
        assert_eq!(
            report
                .intraday_rows
                .iter()
                .map(|r| (r.date.as_str(), r.hour))
                .collect::<Vec<_>>(),
            [("2026-09-24", 23), ("2026-09-25", 0), ("2026-09-25", 12),]
        );
        for (date, expected, unpriced) in [
            (
                "2026-09-24",
                Agg {
                    input: 1,
                    output: 2,
                    cache_read: 3,
                    cache_write: 4,
                    reasoning: 1,
                    messages: 1,
                    cost_reported: 1.0,
                    cost_estimated: 0.0,
                },
                0,
            ),
            (
                "2026-09-25",
                Agg {
                    input: 2,
                    output: 4,
                    cache_read: 6,
                    cache_write: 8,
                    reasoning: 2,
                    messages: 2,
                    cost_reported: 0.0,
                    cost_estimated: 0.25,
                },
                1,
            ),
        ] {
            let daily: Vec<_> = report
                .analysis_rows
                .iter()
                .filter(|r| r.date == date)
                .collect();
            let hourly: Vec<_> = report
                .intraday_rows
                .iter()
                .filter(|r| r.date == date)
                .collect();
            for aggs in [
                daily.iter().map(|r| &r.agg).collect::<Vec<_>>(),
                hourly.iter().map(|r| &r.agg).collect(),
            ] {
                let sum = aggs.into_iter().fold(Agg::default(), |mut sum, agg| {
                    sum.merge(agg);
                    sum
                });
                assert_eq!(sum, expected, "{date}");
            }
            assert_eq!(
                daily.iter().map(|r| r.unpriced_messages).sum::<u64>(),
                unpriced
            );
            assert_eq!(
                hourly.iter().map(|r| r.unpriced_messages).sum::<u64>(),
                unpriced
            );
        }
        assert_eq!(report.today.messages, 2);
        assert!(
            !report
                .analysis_rows
                .iter()
                .any(|r| r.raw_model == "invalid-timestamp")
        );
    }

    #[test]
    fn intraday_rows_keep_local_hour_and_agent_totals() {
        let now = Local.with_ymd_and_hms(2026, 9, 29, 16, 0, 0).unwrap();
        let at = Local.with_ymd_and_hms(2026, 9, 29, 13, 25, 0).unwrap();
        let record = rec("codex", "model", at.timestamp_millis(), TokenBreakdown {
            input: 12,
            ..Default::default()
        });
        let report = aggregate(vec![record], now);
        assert_eq!(report.intraday_rows.len(), 1);
        assert_eq!(report.intraday_rows[0].date, "2026-09-29");
        assert_eq!(report.intraday_rows[0].hour, 13);
        assert_eq!(report.intraday_rows[0].client, "codex");
        assert_eq!(report.intraday_rows[0].agg.total(), 12);
    }

    fn now() -> DateTime<Local> {
        // 固定为 2026-09-25(周五)12:00 本地时,使日期判定可复现.
        Local
            .with_ymd_and_hms(2026, 9, 25, 12, 0, 0)
            .single()
            .unwrap()
    }

    fn ts(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Local
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn windows_bucket_correctly() {
        let records = vec![
            rec(
                "codex",
                "gpt",
                ts(2026, 9, 25, 9),
                TokenBreakdown {
                    input: 1,
                    ..Default::default()
                },
            ),
            rec(
                "omp",
                "m",
                ts(2026, 9, 24, 9),
                TokenBreakdown {
                    input: 10,
                    ..Default::default()
                },
            ), // 昨天同周
            rec(
                "dsh",
                "m",
                ts(2026, 9, 1, 9),
                TokenBreakdown {
                    input: 100,
                    ..Default::default()
                },
            ), // 本月非本周
            rec(
                "zcode",
                "m",
                ts(2026, 8, 20, 9),
                TokenBreakdown {
                    input: 1000,
                    ..Default::default()
                },
            ), // 上月但在30天内
            rec(
                "claude",
                "m",
                ts(2026, 8, 1, 9),
                TokenBreakdown {
                    input: 10_000,
                    ..Default::default()
                },
            ), // 30天外
        ];
        let rep = aggregate(records, now());
        assert_eq!(rep.today.input, 1);
        assert_eq!(rep.week.input, 11);
        assert_eq!(rep.month.input, 111);
        // 当月按 client 拆分:codex 1、omp 10、dsh 100;上月记录(8/20、8/1)不入月账.
        assert_eq!(rep.month_by_client.len(), 3);
        let by = |client: &str| {
            rep.month_by_client
                .iter()
                .find(|(c, _)| *c == client)
                .map(|(_, agg)| agg.input)
                .unwrap()
        };
        assert_eq!(by("codex"), 1);
        assert_eq!(by("omp"), 10);
        assert_eq!(by("dsh"), 100);
        // punchcard 矩阵:9/25=周五(4) 9 点=1、9/24=周四(3)=10、9/1=周二(1)=100;30 天外的 8/20、8/1 不入格.
        assert_eq!(rep.weekday_hour[4][9], 1);
        assert_eq!(rep.weekday_hour[3][9], 10);
        assert_eq!(rep.weekday_hour[1][9], 100);
        assert_eq!(rep.weekday_hour[0][9], 0);
        assert_eq!(rep.weekday_hour[4].iter().sum::<i64>(), 1);
        // 30 天窗口含 8-20(36 天前?)— 8/20 距 9/25 是 36 天,应被排除
        assert_eq!(
            rep.by_agent.iter().map(|a| a.client).collect::<Vec<_>>(),
            vec!["codex", "omp", "dsh"]
        );
    }

    #[test]
    fn thirty_day_window_boundary() {
        let records = vec![
            rec(
                "codex",
                "gpt",
                ts(2026, 8, 27, 9),
                TokenBreakdown {
                    input: 5,
                    ..Default::default()
                },
            ), // 恰好29天前
            rec(
                "codex",
                "gpt",
                ts(2026, 8, 26, 9),
                TokenBreakdown {
                    input: 5,
                    ..Default::default()
                },
            ), // 30天前,排除
        ];
        let rep = aggregate(records, now());
        let codex = rep.by_agent.iter().find(|a| a.client == "codex").unwrap();
        assert_eq!(codex.agg.input, 5);
    }

    #[test]
    fn by_day_covers_14_days_with_empty_days() {
        let records = vec![rec(
            "omp",
            "m",
            ts(2026, 9, 25, 8),
            TokenBreakdown {
                input: 7,
                ..Default::default()
            },
        )];
        let rep = aggregate(records, now());
        assert_eq!(rep.by_day.len(), 14);
        assert_eq!(rep.by_day[13].date, "2026-09-25");
        assert_eq!(rep.by_day[13].agg.input, 7);
        assert_eq!(rep.by_day[0].date, "2026-09-12");
        assert_eq!(rep.by_day[0].agg.total(), 0);
        // per_client 只含有数据的
        assert_eq!(rep.by_day[13].per_client.len(), 1);
        assert_eq!(rep.by_day[12].per_client.len(), 0);
    }

    #[test]
    fn week_starts_on_monday() {
        // 9/25 是周五;周一 = 9/21.上周日 9/20 不算本周.
        let records = vec![
            rec(
                "codex",
                "g",
                ts(2026, 9, 21, 9),
                TokenBreakdown {
                    input: 3,
                    ..Default::default()
                },
            ),
            rec(
                "codex",
                "g",
                ts(2026, 9, 20, 23),
                TokenBreakdown {
                    input: 4,
                    ..Default::default()
                },
            ),
        ];
        let rep = aggregate(records, now());
        assert_eq!(rep.week.input, 3);
        assert_eq!(rep.month.input, 7);
    }

    #[test]
    fn agent_models_sorted_desc() {
        let records = vec![
            rec(
                "omp",
                "small",
                ts(2026, 9, 25, 9),
                TokenBreakdown {
                    input: 10,
                    ..Default::default()
                },
            ),
            rec(
                "omp",
                "big",
                ts(2026, 9, 25, 9),
                TokenBreakdown {
                    input: 999,
                    ..Default::default()
                },
            ),
        ];
        let rep = aggregate(records, now());
        let omp = rep.by_agent.iter().find(|a| a.client == "omp").unwrap();
        assert_eq!(omp.models[0].0, "big");
    }

    #[test]
    fn every_view_excludes_future_records() {
        let n = now();
        let at = n.timestamp_millis();
        let records = [at, at + 1, at + 86_400_000].map(|ts_ms| {
            let mut r = rec("codex", "m", ts_ms, TokenBreakdown { input: 10, ..Default::default() });
            r.session = Some(format!("session-{ts_ms}"));
            r.project = Some("project".into());
            r
        });
        let rep = aggregate(records.into(), n);
        for agg in [&rep.today, &rep.week, &rep.month, &rep.by_agent[0].agg, &rep.by_model[0].agg] {
            assert_eq!(agg.total(), 10);
            assert_eq!(agg.messages, 1);
        }
        assert_eq!(rep.analysis_rows.iter().map(|r| r.agg.total()).sum::<i64>(), 10);
        assert_eq!(rep.intraday_rows.iter().map(|r| r.agg.total()).sum::<i64>(), 10);
        assert_eq!(rep.daily_totals.iter().map(|r| r.tokens).sum::<i64>(), 10);
        assert_eq!(rep.by_day.iter().map(|r| r.agg.total()).sum::<i64>(), 10);
        assert_eq!(rep.by_session.len(), 1);
        assert_eq!(rep.by_project[0].sessions, 1);
        assert_eq!(rep.hourly.iter().sum::<i64>(), 10);
        assert_eq!(rep.weekday.iter().sum::<i64>(), 10);
    }

    #[test]
    fn unknown_ts_records_are_dropped() {
        let records = vec![rec(
            "codex",
            "g",
            0,
            TokenBreakdown {
                input: 5,
                ..Default::default()
            },
        )];
        let rep = aggregate(records, now());
        assert_eq!(rep.today.total(), 0);
        assert!(rep.by_agent.is_empty());
    }

    #[test]
    fn burn_rate_counts_only_last_60_min() {
        let n = now();
        let recent = rec(
            "omp",
            "m",
            n.timestamp_millis() - 10 * 60_000, // 10 分钟前
            TokenBreakdown {
                input: 600,
                ..Default::default()
            },
        );
        let old = rec(
            "omp",
            "m",
            n.timestamp_millis() - 3 * 3_600_000, // 3 小时前
            TokenBreakdown {
                input: 999_999,
                ..Default::default()
            },
        );
        let rep = aggregate(vec![recent, old], n);
        // 600 tok / 60 min = 10 tok/min(不含 3 小时前的量)
        let br = rep.burn_rate.expect("应有燃烧率");
        assert!((br - 10.0).abs() < 1e-9, "burn_rate = {br}");
    }

    #[test]
    fn burn_rate_is_none_when_idle() {
        let n = now();
        let old = rec(
            "omp",
            "m",
            n.timestamp_millis() - 5 * 3_600_000,
            TokenBreakdown {
                input: 1000,
                ..Default::default()
            },
        );
        assert!(aggregate(vec![old], n).burn_rate.is_none());
    }

    #[test]
    fn rolling_trend_and_recent_agents_use_real_time_boundaries() {
        let n = now();
        let at = n.timestamp_millis();
        let mk = |client, ts_ms, input| {
            rec(
                client,
                "m",
                ts_ms,
                TokenBreakdown {
                    input,
                    ..Default::default()
                },
            )
        };
        let rep = aggregate(
            vec![
                mk("omp", at - 24 * 3_600_000, 10),
                mk("codex", at - 3_600_000, 60),
                mk("omp", at - 5 * 60_000, 120),
                mk("codex", at, 30),
                mk("omp", at + 1, 999),
            ],
            n,
        );
        assert_eq!(rep.trend_24h.len(), 24);
        assert_eq!(rep.trend_24h[0].agg.total(), 10);
        assert!(rep.trend_24h[1..23].iter().all(|b| b.agg.total() == 0));
        assert_eq!(rep.trend_24h[23].agg.total(), 210);
        for bucket in &rep.trend_24h {
            assert_eq!(
                bucket
                    .per_client
                    .iter()
                    .map(|(_, agg)| agg.total())
                    .sum::<i64>(),
                bucket.agg.total()
            );
        }
        assert_eq!(
            rep.recent_by_agent
                .iter()
                .map(|a| a.client)
                .collect::<Vec<_>>(),
            vec!["omp", "codex"]
        );
        assert_eq!(rep.recent_by_agent[0].agg.total(), 120);
        assert_eq!(rep.recent_by_agent[1].agg.total(), 90);
        assert_eq!(rep.last_activity_ms, Some(at));
        assert_eq!(rep.burn_rate, Some(210.0 / 60.0));
    }

    #[test]
    fn daily_totals_cover_26_weeks_monday_aligned() {
        let records = vec![rec(
            "omp",
            "m",
            ts(2026, 9, 25, 9),
            TokenBreakdown {
                input: 7,
                ..Default::default()
            },
        )];
        let rep = aggregate(records, now());
        assert_eq!(rep.daily_totals.len(), 26 * 7);
        // 2026-09-25 是周五 → 本周一 09-21,起点 = 25 周前的周一 2026-03-30
        assert_eq!(rep.daily_totals[0].date, "2026-03-30");
        // 网格按整周对齐:最后一天是本周周日(未来日,tokens=0),今天在倒数第三格
        assert_eq!(rep.daily_totals[26 * 7 - 1].date, "2026-09-27");
        assert_eq!(rep.daily_totals[26 * 7 - 1].tokens, 0);
        assert_eq!(rep.daily_totals[26 * 7 - 3].date, "2026-09-25");
        assert_eq!(rep.daily_totals[26 * 7 - 3].tokens, 7);
        assert_eq!(rep.daily_totals[26 * 7 - 3].cost, 0.5);
        // 中段空日补零
        assert_eq!(rep.daily_totals[90].tokens, 0);
    }

    #[test]
    fn daily_detail_groups_by_client_in_clients_order() {
        let records = vec![
            rec(
                "zcode",
                "m",
                ts(2026, 9, 25, 10),
                TokenBreakdown {
                    input: 4,
                    ..Default::default()
                },
            ),
            rec(
                "omp",
                "m",
                ts(2026, 9, 25, 9),
                TokenBreakdown {
                    input: 3,
                    ..Default::default()
                },
            ),
        ];
        let rep = aggregate(records, now());
        let d = rep.daily_detail.get("2026-09-25").unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].0, "omp");
        assert_eq!(d[1].0, "zcode");
        // 无数据日期不进 detail
        assert!(!rep.daily_detail.contains_key("2026-06-01"));
    }

    #[test]
    fn formatters() {
        assert_eq!(format_tokens(856), "856");
        assert_eq!(format_tokens(12_340), "12.3k");
        assert_eq!(format_tokens(4_500_000), "4.5M");
        assert_eq!(format_usd(0.0007), "$0.0007");
        assert_eq!(format_usd(1.5), "$1.50");
        assert_eq!(format_usd(250.0), "$250");
    }
}
