//! LiteLLM 开放价目数据集估价(与 tokscale 同源),磁盘缓存 + 后台自动刷新.
//!
//! - 缓存:`%USERPROFILE%\.quotakeeper\litellm-pricing.json`(HOME 缺失则不落盘),读取不联网;
//! - 查找:精确名 → 大小写全名 → canonical 别名 → 厂商官方条目(家族前缀,裸名) → 反向别名 → provider 后缀众数 → 家族兜底;
//! - 官方兜底:裸名回落厂商官方条目;主条目缺消耗桶(cache_read/cache_write 无价)
//!   时用官方条目补缺桶,渠道价完整的桶一律不动;
//!   多个 provider 同后缀且无众数时不估价。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use super::types::TokenBreakdown;

const PRICING_URLS: [&str; 2] = [
    // jsdelivr CDN 在不稳定链路下更可靠,放首位;raw.githubusercontent 兜底
    "https://cdn.jsdelivr.net/gh/BerriAI/litellm@main/model_prices_and_context_window.json",
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json",
];
/// 后台刷新周期:应用长时间开着时每 24h 自动续拉一次.
const REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(24 * 3_600);
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, Default, PartialEq)]
pub struct ModelPrice {
    input: f64,
    output: f64,
    #[serde(default)]
    cache_read: Option<f64>,
    #[serde(default)]
    cache_write: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceOrigin {
    Upstream,
    Supplement,
    LegacyUnknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    Exact,
    CaseInsensitive,
    ExplicitAlias,
    Canonical,
    OfficialReference,
    ReverseAlias,
    UniqueSuffix,
    MajorityReference,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AdoptedRate {
    pub usd_per_token: f64,
    pub entry: String,
    pub origin: PriceOrigin,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EstimateBasis {
    pub matched_entry: String,
    pub match_method: MatchMethod,
    pub family_fallback: bool,
    pub cache_fill: bool,
    pub input: AdoptedRate,
    pub output: AdoptedRate,
    pub cache_read: Option<AdoptedRate>,
    pub cache_write: Option<AdoptedRate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnpricedReason {
    UnknownModel,
    AmbiguousName,
    AmbiguousSuffix,
    MissingCacheRead,
    MissingCacheWrite,
    MissingCacheReadAndWrite,
    NonFiniteAmount,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PriceBasis {
    Reported,
    Estimated(EstimateBasis),
    Unpriced {
        reason: UnpricedReason,
        attempted: Option<EstimateBasis>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PriceQuote {
    pub amount: Option<f64>,
    pub basis: PriceBasis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSource {
    CacheV2,
    LegacyCache,
    SupplementsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheState {
    Fresh,
    Stale,
    UnknownAge,
    Missing,
    Corrupt,
    NotLoaded,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PriceSnapshot {
    pub version: String,
    pub loaded_at_ms: i64,
    pub fetched_at_ms: Option<i64>,
    pub dataset_url: Option<String>,
    pub source: SnapshotSource,
    pub cache_state: CacheState,
}

impl PriceSnapshot {
    fn supplements(loaded_at_ms: i64, cache_state: CacheState) -> Self {
        Self {
            version: String::new(),
            loaded_at_ms,
            fetched_at_ms: None,
            dataset_url: None,
            source: SnapshotSource::SupplementsOnly,
            cache_state,
        }
    }
}

#[derive(Debug)]
struct Entry {
    price: ModelPrice,
    origin: PriceOrigin,
}

struct MatchedEntry<'a> {
    name: &'a str,
    entry: &'a Entry,
    method: MatchMethod,
    family_fallback: bool,
}

impl MatchedEntry<'_> {
    fn rate(&self, usd_per_token: f64) -> AdoptedRate {
        AdoptedRate {
            usd_per_token,
            entry: self.name.into(),
            origin: self.entry.origin,
        }
    }

    fn basis(&self) -> EstimateBasis {
        let p = self.entry.price;
        EstimateBasis {
            matched_entry: self.name.into(),
            match_method: self.method,
            family_fallback: self.family_fallback,
            cache_fill: false,
            input: self.rate(p.input),
            output: self.rate(p.output),
            cache_read: p.cache_read.map(|rate| self.rate(rate)),
            cache_write: p.cache_write.map(|rate| self.rate(rate)),
        }
    }
}

#[derive(Debug)]
pub struct Pricing {
    snapshot: PriceSnapshot,
    entries: HashMap<String, Entry>,
    case_insensitive: HashMap<String, String>,
    ambiguous_names: HashSet<String>,
    unique_suffixes: HashMap<String, (String, MatchMethod)>,
    ambiguous_suffixes: HashSet<String>,
}

impl Default for Pricing {
    fn default() -> Self {
        build(
            HashMap::new(),
            PriceOrigin::LegacyUnknown,
            PriceSnapshot::supplements(now_ms(), CacheState::NotLoaded),
        )
    }
}

/// canonical 家族 → LiteLLM 目录里厂商官方条目的 provider 前缀。
/// 只收录实测在目录中有官方条目的家族(zai/glm-4.5v、deepseek/deepseek-v4-flash、
/// gemini/gemini-3.8-flash、moonshot/kimi-k2.6、minimax/MiniMax-M2.5-lightning);
/// 没有官方条目的家族(qwen/stepfun 等)不进表,行为与从前一致。
const OFFICIAL_PREFIXES: [(&str, &str); 5] = [
    ("glm-", "zai"),
    ("deepseek-", "deepseek"),
    ("gemini-", "gemini"),
    ("kimi-", "moonshot"),
    ("minimax-", "minimax"),
];

#[derive(Debug, Deserialize, serde::Serialize)]
struct CacheDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    schema_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dataset_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fetched_at_ms: Option<i64>,
    models: HashMap<String, ModelPrice>,
}

impl CacheDoc {
    fn valid_v2(&self) -> bool {
        self.schema_version == Some(2)
            && self
                .dataset_url
                .as_deref()
                .is_some_and(|url| url.starts_with("https://"))
            && self.fetched_at_ms.is_some_and(|time| time > 0)
            && !self.models.is_empty()
            && self.models.values().all(valid_price)
    }
}

pub fn cache_path(home: &Path) -> PathBuf {
    home.join(".quotakeeper").join("litellm-pricing.json")
}

/// 打开应用即后台拉最新价,每 24h 续拉;只写缓存,不碰内存表.
pub fn spawn_refresher(home: &Path) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let home = home.to_path_buf();
    let _ = std::thread::Builder::new()
        .name("pricing-refresh".into())
        .spawn(move || {
            loop {
                if let Some(fresh) = fetch() {
                    let _ = write_cache(&cache_path(&home), &fresh);
                }
                std::thread::sleep(REFRESH_INTERVAL);
            }
        });
}

impl Pricing {
    /// 只读本地缓存;冷启动/离线使用补充价,网络刷新由 spawn_refresher 负责。
    pub fn load(home: &Path) -> Pricing {
        let loaded_at_ms = now_ms();
        match read_cache(&cache_path(home), loaded_at_ms) {
            Ok((entries, origin, snapshot)) => build(entries, origin, snapshot),
            Err(state) => build(
                HashMap::new(),
                PriceOrigin::LegacyUnknown,
                PriceSnapshot::supplements(loaded_at_ms, state),
            ),
        }
    }

    pub fn snapshot(&self) -> &PriceSnapshot {
        &self.snapshot
    }

    pub fn quote(&self, model: &str, t: &TokenBreakdown) -> PriceQuote {
        let matched = match self.lookup(model) {
            Ok(matched) => matched,
            Err(reason) => match self.official_entry(model) {
                Some(matched) => matched,
                None => {
                    return PriceQuote {
                        amount: None,
                        basis: PriceBasis::Unpriced {
                            reason,
                            attempted: None,
                        },
                    };
                }
            },
        };
        let mut basis = matched.basis();
        let missing_read = t.cache_read > 0 && basis.cache_read.is_none();
        let missing_write = t.cache_write > 0 && basis.cache_write.is_none();
        if (missing_read || missing_write)
            && let Some(reference) = self.official_entry(model)
        {
            if missing_read && let Some(rate) = reference.entry.price.cache_read {
                basis.cache_read = Some(reference.rate(rate));
                basis.cache_fill = true;
            }
            if missing_write && let Some(rate) = reference.entry.price.cache_write {
                basis.cache_write = Some(reference.rate(rate));
                basis.cache_fill = true;
            }
        }
        let missing = match (
            t.cache_read > 0 && basis.cache_read.is_none(),
            t.cache_write > 0 && basis.cache_write.is_none(),
        ) {
            (true, true) => Some(UnpricedReason::MissingCacheReadAndWrite),
            (true, false) => Some(UnpricedReason::MissingCacheRead),
            (false, true) => Some(UnpricedReason::MissingCacheWrite),
            (false, false) => None,
        };
        if let Some(reason) = missing {
            return PriceQuote {
                amount: None,
                basis: PriceBasis::Unpriced {
                    reason,
                    attempted: Some(basis),
                },
            };
        }
        let mut amount = t.input as f64 * basis.input.usd_per_token
            + t.output as f64 * basis.output.usd_per_token;
        if t.cache_read > 0 {
            amount += t.cache_read as f64 * basis.cache_read.as_ref().unwrap().usd_per_token;
        }
        if t.cache_write > 0 {
            amount += t.cache_write as f64 * basis.cache_write.as_ref().unwrap().usd_per_token;
        }
        if amount.is_finite() {
            PriceQuote {
                amount: Some(amount),
                basis: PriceBasis::Estimated(basis),
            }
        } else {
            PriceQuote {
                amount: None,
                basis: PriceBasis::Unpriced {
                    reason: UnpricedReason::NonFiniteAmount,
                    attempted: Some(basis),
                },
            }
        }
    }

    fn matched(&self, name: &str, method: MatchMethod) -> Option<MatchedEntry<'_>> {
        self.entries
            .get_key_value(name)
            .map(|(name, entry)| MatchedEntry {
                name,
                entry,
                method,
                family_fallback: false,
            })
    }

    fn official_entry(&self, model: &str) -> Option<MatchedEntry<'_>> {
        let canonical = crate::ledger::model_alias::canonical(model);
        if canonical == "unknown" {
            return None;
        }
        let prefix = OFFICIAL_PREFIXES
            .iter()
            .find(|(family, _)| canonical.starts_with(family))
            .map(|(_, prefix)| *prefix);
        let name = prefix
            .and_then(|p| self.case_insensitive.get(&format!("{p}/{canonical}")))
            .or_else(|| self.case_insensitive.get(&canonical))?;
        self.matched(name, MatchMethod::OfficialReference)
    }

    fn lookup(&self, model: &str) -> Result<MatchedEntry<'_>, UnpricedReason> {
        if model.is_empty() {
            return Err(UnpricedReason::UnknownModel);
        }
        if let Some(matched) = self.matched(model, MatchMethod::Exact) {
            return Ok(matched);
        }
        let lower = model.to_lowercase();
        if self.ambiguous_names.contains(&lower) {
            return Err(UnpricedReason::AmbiguousName);
        }
        if let Some(name) = self.case_insensitive.get(&lower) {
            return Ok(self.matched(name, MatchMethod::CaseInsensitive).unwrap());
        }

        let (canonical, explicit_alias) = crate::ledger::model_alias::resolve(model);
        let method = if explicit_alias {
            MatchMethod::ExplicitAlias
        } else {
            MatchMethod::Canonical
        };
        if canonical != model
            && canonical != "unknown"
            && let Some(matched) = self.matched(&canonical, method)
        {
            return Ok(matched);
        }

        if !model.contains('/') && canonical != "unknown" {
            let prefix = OFFICIAL_PREFIXES
                .iter()
                .find(|(family, _)| canonical.starts_with(family))
                .map(|(_, prefix)| *prefix);
            let candidates = prefix
                .map(|p| format!("{p}/{canonical}"))
                .into_iter()
                .chain(std::iter::once(canonical.clone()));
            for candidate in candidates {
                if candidate != lower
                    && let Some(name) = self.case_insensitive.get(&candidate)
                {
                    return Ok(self.matched(name, MatchMethod::OfficialReference).unwrap());
                }
            }
        }

        if canonical != "unknown" {
            for source in crate::ledger::model_alias::source_names(&canonical) {
                if source != lower
                    && let Some(name) = self.case_insensitive.get(source)
                {
                    return Ok(self.matched(name, MatchMethod::ReverseAlias).unwrap());
                }
            }
        }

        let suffix = lower.rsplit('/').next().unwrap_or(&lower);
        if self.ambiguous_suffixes.contains(suffix) {
            return Err(UnpricedReason::AmbiguousSuffix);
        }
        if let Some((name, method)) = self.unique_suffixes.get(suffix) {
            return Ok(self.matched(name, *method).unwrap());
        }
        let family = family_name(model).ok_or(UnpricedReason::UnknownModel)?;
        self.lookup(&family).map(|mut matched| {
            matched.family_fallback = true;
            matched
        })
    }
}

/// 剥掉版本段里的 minor 位:段内恰有一个点、两侧全为数字才认定是 major.minor(只动第一处).
fn family_name(model: &str) -> Option<String> {
    let mut out = String::with_capacity(model.len());
    let mut stripped = false;
    for seg in model.split('-') {
        if !stripped
            && seg.matches('.').count() == 1
            && let Some((major, minor)) = seg.split_once('.')
            && !major.is_empty()
            && !minor.is_empty()
            && major.chars().all(|c| c.is_ascii_digit())
            && minor.chars().all(|c| c.is_ascii_digit())
        {
            out.push_str(major);
            stripped = true;
        } else {
            out.push_str(seg);
        }
        out.push('-');
    }
    if !stripped {
        return None;
    }
    out.pop();
    Some(out)
}

fn read_cache(
    path: &Path,
    loaded_at_ms: i64,
) -> Result<(HashMap<String, ModelPrice>, PriceOrigin, PriceSnapshot), CacheState> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CacheState::Missing
        } else {
            CacheState::Corrupt
        }
    })?;
    let mut doc: CacheDoc = serde_json::from_str(&text).map_err(|_| CacheState::Corrupt)?;
    doc.models.retain(|_, price| valid_price(price));
    if doc.models.is_empty() {
        return Err(CacheState::Corrupt);
    }
    let (source, origin, dataset_url) = match doc.schema_version {
        Some(2) if doc.valid_v2() => (
            SnapshotSource::CacheV2,
            PriceOrigin::Upstream,
            doc.dataset_url,
        ),
        None => (
            SnapshotSource::LegacyCache,
            PriceOrigin::LegacyUnknown,
            None,
        ),
        _ => return Err(CacheState::Corrupt),
    };
    let fetched_at_ms = doc.fetched_at_ms.filter(|time| *time > 0);
    let cache_state = match fetched_at_ms {
        Some(time) if time > loaded_at_ms => CacheState::UnknownAge,
        Some(time) if loaded_at_ms.saturating_sub(time) >= REFRESH_INTERVAL.as_millis() as i64 => {
            CacheState::Stale
        }
        Some(_) => CacheState::Fresh,
        None => CacheState::UnknownAge,
    };
    Ok((
        doc.models,
        origin,
        PriceSnapshot {
            version: String::new(),
            loaded_at_ms,
            fetched_at_ms,
            dataset_url,
            source,
            cache_state,
        },
    ))
}

fn write_cache(path: &Path, doc: &CacheDoc) -> std::io::Result<()> {
    publish_cache(path, doc, |temporary, target| {
        std::fs::rename(temporary, target)
    })
}

fn publish_cache(
    path: &Path,
    doc: &CacheDoc,
    replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    if !doc.valid_v2() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid upstream pricing document",
        ));
    }
    let bytes = serde_json::to_vec(doc).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let filename = path.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing cache filename")
    })?;
    let temporary = path.with_file_name(format!(
        ".{}.{}-{}-{}.tmp",
        filename.to_string_lossy(),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        replace(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn fetch() -> Option<CacheDoc> {
    PRICING_URLS.iter().find_map(|url| fetch_one(url))
}

fn fetch_one(url: &str) -> Option<CacheDoc> {
    let resp = crate::http::get(url).timeout(TIMEOUT).call().ok()?;
    let doc: HashMap<String, serde_json::Value> = resp.into_json().ok()?;
    let fetched_at_ms = now_ms();
    let mut entries = HashMap::new();
    for (name, value) in doc {
        let Some(input) = value
            .get("input_cost_per_token")
            .and_then(|value| value.as_f64())
        else {
            continue;
        };
        let Some(output) = value
            .get("output_cost_per_token")
            .and_then(|value| value.as_f64())
        else {
            continue;
        };
        let optional = |key: &str| {
            value
                .get(key)
                .and_then(|value| value.as_f64())
                .filter(|rate| rate.is_finite())
        };
        let price = ModelPrice {
            input,
            output,
            cache_read: optional("cache_read_input_token_cost"),
            cache_write: optional("cache_creation_input_token_cost"),
        };
        if valid_price(&price) {
            entries.insert(name, price);
        }
    }
    if entries.is_empty() {
        return None;
    }
    Some(CacheDoc {
        schema_version: Some(2),
        dataset_url: Some(url.into()),
        fetched_at_ms: Some(fetched_at_ms),
        models: entries,
    })
}

/// 补充 LiteLLM 缺失的官方或主流模型定价 (来源: OpenRouter / DeepInfra 官方公开定价 / 限免模型)
/// 注意: 只作为 fallback 补齐; LiteLLM 原有条目具有优先权，不被覆盖。
const SUPPLEMENT: &[(&str, ModelPrice)] = &[
    // OpenAI GPT-6.1 系列(LiteLLM 尚未收录;来源: OpenRouter 官方挂牌 2026-09-30,
    // 与 gpt-6-sol 同价: 输入 $2/M, 输出 $10/M, 缓存读 $0.1/M)
    (
        "gpt-6.1-sol",
        ModelPrice {
            input: 2e-6,
            output: 1e-5,
            cache_read: Some(1e-7),
            cache_write: None,
        },
    ),
    (
        "openai/gpt-6.1-sol",
        ModelPrice {
            input: 2e-6,
            output: 1e-5,
            cache_read: Some(1e-7),
            cache_write: None,
        },
    ),
    // Xiaomi MiMo (来源: OpenRouter / DeepInfra 官方定盘价: 输入 $0.14/M, 输出 $0.28/M, 缓存读 $0.0028/M)
    (
        "mimo-v2.6-flash",
        ModelPrice {
            input: 1.4e-7,
            output: 2.8e-7,
            cache_read: Some(2.8e-9),
            cache_write: None,
        },
    ),
    (
        "xiaomi/mimo-v2.6-flash",
        ModelPrice {
            input: 1.4e-7,
            output: 2.8e-7,
            cache_read: Some(2.8e-9),
            cache_write: None,
        },
    ),
    (
        "mimo-v2.5",
        ModelPrice {
            input: 1.4e-7,
            output: 2.8e-7,
            cache_read: Some(2.8e-9),
            cache_write: None,
        },
    ),
    // 腾讯混元 3 (来源: DeepInfra tencent/Hy3: 输入 $0.13/M, 输出 $0.53/M, 缓存读 $0.033/M)
    (
        "hy3",
        ModelPrice {
            input: 1.3e-7,
            output: 5.3e-7,
            cache_read: Some(3.3e-8),
            cache_write: None,
        },
    ),
    (
        "tencent/hy3",
        ModelPrice {
            input: 1.3e-7,
            output: 5.3e-7,
            cache_read: Some(3.3e-8),
            cache_write: None,
        },
    ),
    // OpenCode Zen / OpenRouter 限免与隐身模型 ($0.0 官方限免)
    (
        "big-pickle",
        ModelPrice {
            input: 0.0,
            output: 0.0,
            cache_read: Some(0.0),
            cache_write: None,
        },
    ),
    (
        "space-bunny-free",
        ModelPrice {
            input: 0.0,
            output: 0.0,
            cache_read: Some(0.0),
            cache_write: None,
        },
    ),
    (
        "space-bunny-alpha",
        ModelPrice {
            input: 0.0,
            output: 0.0,
            cache_read: Some(0.0),
            cache_write: None,
        },
    ),
    (
        "mimo-v2.6-flash-free",
        ModelPrice {
            input: 0.0,
            output: 0.0,
            cache_read: Some(0.0),
            cache_write: None,
        },
    ),
];

fn build(
    entries: HashMap<String, ModelPrice>,
    origin: PriceOrigin,
    mut snapshot: PriceSnapshot,
) -> Pricing {
    let mut entries: HashMap<String, Entry> = entries
        .into_iter()
        .filter(|(_, price)| valid_price(price))
        .map(|(name, price)| (name, Entry { price, origin }))
        .collect();
    for (name, price) in SUPPLEMENT {
        entries.entry((*name).into()).or_insert(Entry {
            price: *price,
            origin: PriceOrigin::Supplement,
        });
    }

    let mut case_insensitive = HashMap::new();
    let mut ambiguous_names = HashSet::new();
    let mut unique_suffixes = HashMap::new();
    let mut ambiguous_suffixes = HashSet::new();
    let mut suffix_groups: HashMap<String, Vec<&String>> = HashMap::new();
    for name in entries.keys() {
        let lower = name.to_lowercase();
        if !ambiguous_names.contains(&lower) {
            if case_insensitive.remove(&lower).is_some() {
                ambiguous_names.insert(lower);
            } else {
                case_insensitive.insert(lower, name.clone());
            }
        }
        let suffix = name.rsplit('/').next().unwrap_or(name).to_lowercase();
        suffix_groups.entry(suffix).or_default().push(name);
    }

    for (suffix, mut group) in suffix_groups {
        group.sort();
        if group.len() == 1 {
            unique_suffixes.insert(suffix, (group[0].clone(), MatchMethod::UniqueSuffix));
            continue;
        }
        let key = |name: &str| {
            let p = entries[name].price;
            (
                p.input.to_bits(),
                p.output.to_bits(),
                p.cache_read.map(f64::to_bits),
                p.cache_write.map(f64::to_bits),
            )
        };
        let mut counts = HashMap::new();
        for name in &group {
            let entry = counts.entry(key(name)).or_insert((0, *name));
            entry.0 += 1;
        }
        if let Some((number, name)) = counts.values().max_by_key(|(number, _)| *number)
            && number * 2 > group.len()
        {
            unique_suffixes.insert(suffix, ((*name).clone(), MatchMethod::MajorityReference));
        } else {
            ambiguous_suffixes.insert(suffix);
        }
    }

    snapshot.version = content_version(&entries, snapshot.dataset_url.as_deref());
    Pricing {
        snapshot,
        entries,
        case_insensitive,
        ambiguous_names,
        unique_suffixes,
        ambiguous_suffixes,
    }
}

fn valid_price(price: &ModelPrice) -> bool {
    [
        Some(price.input),
        Some(price.output),
        price.cache_read,
        price.cache_write,
    ]
    .into_iter()
    .flatten()
    .all(|value| value.is_finite() && value >= 0.0)
}

fn content_version(entries: &HashMap<String, Entry>, dataset_url: Option<&str>) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    let mut update = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    let dataset = dataset_url.unwrap_or("");
    update(&(dataset.len() as u64).to_le_bytes());
    update(dataset.as_bytes());
    let mut named: Vec<_> = entries.iter().collect();
    named.sort_by_key(|(name, _)| *name);
    for (name, entry) in named {
        update(&(name.len() as u64).to_le_bytes());
        update(name.as_bytes());
        update(&[match entry.origin {
            PriceOrigin::Upstream => 0,
            PriceOrigin::Supplement => 1,
            PriceOrigin::LegacyUnknown => 2,
        }]);
        let p = entry.price;
        for rate in [Some(p.input), Some(p.output), p.cache_read, p.cache_write] {
            match rate {
                Some(rate) => {
                    update(&[1]);
                    update(&rate.to_bits().to_le_bytes());
                }
                None => update(&[0]),
            }
        }
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(entries: HashMap<String, ModelPrice>) -> Pricing {
        super::build(
            entries,
            PriceOrigin::LegacyUnknown,
            PriceSnapshot::supplements(now_ms(), CacheState::NotLoaded),
        )
    }

    struct TestHome(std::path::PathBuf);

    impl TestHome {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "quotakeeper-pricing-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
            ));
            std::fs::create_dir_all(path.join(".quotakeeper")).unwrap();
            Self(path)
        }
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn upstream_doc(models: HashMap<String, ModelPrice>) -> CacheDoc {
        CacheDoc {
            schema_version: Some(2),
            dataset_url: Some(PRICING_URLS[1].into()),
            fetched_at_ms: Some(now_ms() - 1_000),
            models,
        }
    }

    #[test]
    fn future_fetch_time_keeps_prices_without_claiming_freshness() {
        let home = TestHome::new();
        let path = cache_path(&home.0);
        let mut doc = upstream_doc(HashMap::from([(
            "test".into(),
            ModelPrice {
                input: 3.0,
                ..Default::default()
            },
        )]));
        doc.fetched_at_ms = Some(2_000);
        write_cache(&path, &doc).unwrap();
        let (entries, origin, snapshot) = read_cache(&path, 1_000).unwrap();
        assert_eq!(snapshot.fetched_at_ms, Some(2_000));
        assert_eq!(snapshot.loaded_at_ms, 1_000);
        assert_eq!(snapshot.cache_state, CacheState::UnknownAge);
        let pricing = super::build(entries, origin, snapshot);
        assert_eq!(
            pricing
                .quote(
                    "test",
                    &TokenBreakdown {
                        input: 1,
                        ..Default::default()
                    }
                )
                .amount,
            Some(3.0)
        );
    }

    #[test]
    fn failed_cache_replacement_preserves_previous_document() {
        let home = TestHome::new();
        let path = cache_path(&home.0);
        let mut doc = upstream_doc(HashMap::from([(
            "test".into(),
            ModelPrice {
                input: 1.0,
                ..Default::default()
            },
        )]));
        write_cache(&path, &doc).unwrap();
        let previous = std::fs::read(&path).unwrap();
        doc.models.get_mut("test").unwrap().input = 2.0;
        let error = publish_cache(&path, &doc, |temporary, target| {
            assert_eq!(temporary.parent(), target.parent());
            assert_ne!(temporary, target);
            assert_eq!(std::fs::read(target).unwrap(), previous);
            let ready: CacheDoc =
                serde_json::from_slice(&std::fs::read(temporary).unwrap()).unwrap();
            assert_eq!(ready.models["test"].input, 2.0);
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "replacement denied",
            ))
        })
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(std::fs::read(&path).unwrap(), previous);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn cache_v2_preserves_real_metadata_and_independent_origins() {
        let home = TestHome::new();
        let path = cache_path(&home.0);
        let doc = upstream_doc(HashMap::from([(
            "mimo-v2.6-flash".into(),
            ModelPrice {
                input: 9.0,
                output: 9.0,
                ..Default::default()
            },
        )]));
        write_cache(&path, &doc).unwrap();
        let p = Pricing::load(&home.0);
        let snapshot = p.snapshot();
        assert_eq!(snapshot.source, SnapshotSource::CacheV2);
        assert_eq!(snapshot.cache_state, CacheState::Fresh);
        assert_eq!(snapshot.fetched_at_ms, doc.fetched_at_ms);
        assert_eq!(snapshot.dataset_url, doc.dataset_url);
        assert!(snapshot.loaded_at_ms >= doc.fetched_at_ms.unwrap());
        assert_eq!(Pricing::load(&home.0).snapshot().version, snapshot.version);
        let tokens = TokenBreakdown {
            input: 1,
            ..Default::default()
        };
        for (name, origin) in [
            ("mimo-v2.6-flash", PriceOrigin::Upstream),
            ("hy3", PriceOrigin::Supplement),
        ] {
            let PriceBasis::Estimated(basis) = p.quote(name, &tokens).basis else {
                panic!("known entry must be quoted");
            };
            for rate in [&basis.input, &basis.output]
                .into_iter()
                .chain(basis.cache_read.as_ref())
            {
                assert_eq!(rate.origin, origin);
                assert_eq!(rate.entry, name);
            }
        }
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(saved["schema_version"], 2);
        assert_eq!(saved["models"].as_object().unwrap().len(), 1);
        assert!(saved["models"].get("hy3").is_none());
    }

    fn pricing_with(entries: &[(&str, f64, f64)]) -> Pricing {
        build(
            entries
                .iter()
                .map(|(n, i, o)| {
                    (
                        n.to_string(),
                        ModelPrice {
                            input: *i,
                            output: *o,
                            cache_read: Some(0.1),
                            cache_write: None,
                        },
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn quote_keeps_named_entries_for_exact_and_case_matches() {
        let p = pricing_with(&[("provider/test-model", 3.0, 4.0)]);
        let tokens = TokenBreakdown {
            input: 10,
            ..Default::default()
        };
        for (name, method) in [
            ("provider/test-model", MatchMethod::Exact),
            ("Provider/Test-Model", MatchMethod::CaseInsensitive),
        ] {
            let quote = p.quote(name, &tokens);
            assert_eq!(quote.amount, Some(30.0));
            let PriceBasis::Estimated(basis) = quote.basis else {
                panic!("known entry must have an estimated basis");
            };
            assert_eq!(basis.matched_entry, "provider/test-model");
            assert_eq!(basis.match_method, method);
            assert_eq!(basis.input.entry, "provider/test-model");
            assert_eq!(basis.input.usd_per_token, 3.0);
            for rate in [
                &basis.input,
                &basis.output,
                basis.cache_read.as_ref().unwrap(),
            ] {
                assert_eq!(rate.entry, "provider/test-model");
                assert_eq!(rate.origin, PriceOrigin::LegacyUnknown);
            }
            assert!(!basis.family_fallback);
            assert!(!basis.cache_fill);
        }
    }

    #[test]
    fn bare_glm_uses_official_price_when_resellers_disagree() {
        let official = ModelPrice { input: 1.5e-7, output: 5e-7, cache_read: Some(3e-8), cache_write: Some(0.0) };
        let reseller = ModelPrice { cache_write: None, ..official };
        let p = build(HashMap::from([
            ("zai/glm-5.3-flash".into(), official),
            ("openrouter/z-ai/glm-5.3-flash".into(), reseller),
            ("nebius/zai-org/GLM-5.3-Flash".into(), ModelPrice { cache_read: None, ..reseller }),
            ("aihubmix/glm-5.3-flash".into(), ModelPrice { input: 1.1268e-7, ..reseller }),
        ]));
        let t = TokenBreakdown { input: 1_000_000, output: 1_000_000, cache_read: 1_000_000, cache_write: 1_000_000, ..Default::default() };
        // 裸名直接落官方条目
        for name in ["glm-5.3-flash", "GLM-5.3-Flash"] {
            assert!((p.quote(name, &t).amount.expect("official GLM price") - 0.68).abs() < 1e-9);
        }
        // 后缀歧义的渠道名:官方条目兜底(渠道互相打架时不放弃估价)
        assert!(
            (p.quote("openrouter/z-ai/glm-5.3-flash", &t)
                .amount
                .expect("official rescue")
                - 0.68)
                .abs()
                < 1e-9
        );
        assert!(
            (p.quote("nebius/zai-org/GLM-5.3-Flash", &t)
                .amount
                .expect("official rescue")
                - 0.68)
                .abs()
                < 1e-9
        );
        // 裸名已有条目但缺缓存价:主价保留,官方只补缺桶 → 仍 0.68
        let p = build(HashMap::from([("glm-5.3-flash".into(), reseller), ("zai/glm-5.3-flash".into(), official)]));
        assert!((p.quote("glm-5.3-flash", &t).amount.expect("official fill") - 0.68).abs() < 1e-9);
    }

    #[test]
    fn official_fill_only_touches_missing_buckets() {
        // 多数渠道价完整时取众数价,官方不覆盖任何桶
        let complete = ModelPrice { input: 2e-7, output: 6e-7, cache_read: Some(1e-8), cache_write: Some(2e-8) };
        let official = ModelPrice { input: 1.5e-7, output: 5e-7, cache_read: Some(3e-8), cache_write: Some(0.0) };
        let p = build(HashMap::from([
            ("v1/glm-5.3-flash".into(), complete),
            ("v2/glm-5.3-flash".into(), complete),
            ("v3/glm-5.3-flash".into(), complete),
            ("zai/glm-5.3-flash".into(), official),
        ]));
        let t = TokenBreakdown {
            input: 1_000_000,
            output: 1_000_000,
            cache_read: 1_000_000,
            cache_write: 1_000_000,
            ..Default::default()
        };
        assert!(
            (p.quote("v1/glm-5.3-flash", &t)
                .amount
                .expect("majority kept")
                - 0.83)
                .abs()
                < 1e-9
        );
        // 众数渠道缺缓存价:基础桶用渠道价,缺的桶由官方补
        let no_cache = ModelPrice { cache_read: None, cache_write: None, ..complete };
        let p = build(HashMap::from([
            ("v1/glm-5.3-flash".into(), no_cache),
            ("v2/glm-5.3-flash".into(), no_cache),
            ("v3/glm-5.3-flash".into(), no_cache),
            ("zai/glm-5.3-flash".into(), official),
        ]));
        // 0.2(in) + 0.6(out) + 0.03(官方 cr);cw 未消耗不参与
        let t2 = TokenBreakdown {
            input: 1_000_000,
            output: 1_000_000,
            cache_read: 1_000_000,
            ..Default::default()
        };
        assert!(
            (p.quote("v1/glm-5.3-flash", &t2)
                .amount
                .expect("official fills cache_read")
                - 0.83)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn deepseek_full_name_prices_via_reverse_alias() {
        // 目录只有推广短名 deepseek-flash(官方价、缓存价完整),全名经反向别名取价
        let p = build(HashMap::from([(
            "deepseek-flash".into(),
            ModelPrice { input: 3e-7, output: 1.2e-6, cache_read: Some(6e-9), cache_write: Some(0.0) },
        )]));
        let t = TokenBreakdown { input: 1_000_000, output: 1_000_000, cache_read: 1_000_000, ..Default::default() };
        let expected = 0.3 + 1.2 + 0.006;
        for name in ["deepseek-v4.1-flash", "deepseek/deepseek-v4.1-flash"] {
            assert!((p.quote(name, &t).amount.expect("reverse alias") - expected).abs() < 1e-9);
        }
    }

    #[test]
    fn effort_variant_prices_as_base_model() {
        // -high 是推理力度尾缀:canonical 剥掉后按基名取价
        let p = build(HashMap::from([(
            "gemini-3.8-flash".into(),
            ModelPrice { input: 7.5e-7, output: 3.75e-6, cache_read: Some(7.5e-8), cache_write: None },
        )]));
        let t = TokenBreakdown { input: 1_000_000, output: 1_000_000, cache_read: 1_000_000, ..Default::default() };
        let expected = 0.75 + 3.75 + 0.075;
        assert!(
            (p.quote("gemini-3.8-flash-high", &t)
                .amount
                .expect("effort variant")
                - expected)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn ambiguous_suffix_without_official_entry_stays_none() {
        // 无官方条目的歧义后缀维持拒绝,官方兜底不凭空造价
        let p = build(HashMap::from([
            ("vendor-a/shared-model".into(), ModelPrice { input: 1.0, output: 2.0, ..Default::default() }),
            ("vendor-b/shared-model".into(), ModelPrice { input: 3.0, output: 4.0, ..Default::default() }),
        ]));
        assert_eq!(
            p.quote(
                "shared-model",
                &TokenBreakdown {
                    input: 10,
                    ..Default::default()
                }
            )
            .amount,
            None
        );
    }

    #[test]
    fn consumed_cache_buckets_need_prices_and_zero_is_explicit() {
        for json in ["{}", r#"{"input":0}"#, r#"{"output":0}"#] {
            assert!(serde_json::from_str::<ModelPrice>(json).is_err());
        }
        assert!(serde_json::from_str::<ModelPrice>(r#"{"input":0,"output":0}"#).is_ok());
        let price = ModelPrice { input: 1.0, output: 2.0, ..Default::default() };
        let p = build(HashMap::from([("test".into(), price)]));
        let mut t = TokenBreakdown { input: 10, ..Default::default() };
        assert_eq!(p.quote("test", &t).amount, Some(10.0));
        t.cache_read = 5;
        assert_eq!(p.quote("test", &t).amount, None);
        t.cache_read = 0;
        t.cache_write = 5;
        assert_eq!(p.quote("test", &t).amount, None);
        let p = build(HashMap::from([("test".into(), ModelPrice { cache_read: Some(0.0), cache_write: Some(0.0), ..price })]));
        assert_eq!(p.quote("test", &t).amount, Some(10.0));
        for input in [-1.0, f64::NAN, f64::INFINITY] {
            let p = build(HashMap::from([("invalid".into(), ModelPrice { input, ..price })]));
            assert_eq!(p.quote("invalid", &t).amount, None);
        }
    }

    #[test]
    fn qualified_case_insensitive_name_precedes_bare_alias() {
        let p = pricing_with(&[("test-model", 1.0, 2.0), ("provider/test-model", 3.0, 4.0)]);
        let t = TokenBreakdown { input: 10, ..Default::default() };
        assert_eq!(p.quote("Provider/Test-Model", &t).amount, Some(30.0));
    }

    #[test]
    fn estimate_multiplies_buckets() {
        let p = pricing_with(&[("gpt-6-sol", 2.0, 8.0)]);
        let t = TokenBreakdown {
            input: 1_000_000,
            output: 100_000,
            cache_read: 500_000,
            cache_write: 0,
            reasoning: 0,
        };
        // 每-token 单价 × token 数:1e6×2 + 1e5×8 + 5e5×0.1
        let cost = p.quote("gpt-6-sol", &t).amount.unwrap();
        assert!((cost - 2_850_000.0).abs() < 1e-6);
    }

    #[test]
    fn provider_prefix_falls_back_to_suffix() {
        let p = pricing_with(&[("deepseek-v4.1-flash", 1.0, 2.0)]);
        assert!(
            p.quote("deepseek/deepseek-v4.1-flash", &TokenBreakdown::default())
                .amount
                .is_some()
        );
        assert!(
            p.quote("DeepSeek/deepseek-v4.1-flash", &TokenBreakdown::default())
                .amount
                .is_some()
        );
        assert!(
            p.quote("totally-unknown-model", &TokenBreakdown::default())
                .amount
                .is_none()
        );
    }

    /// 目录只有上一代时,剥 minor 版本走家族兜底估价(新模型收录空窗期)。
    /// 注意不能选 gpt-6.1-sol:它在 SUPPLEMENT 里,build() 时就会精确命中。
    #[test]
    fn family_fallback_estimates_from_previous_minor() {
        let p = pricing_with(&[("glm-5-flash", 2.0, 10.0)]);
        let t = TokenBreakdown {
            input: 1_000_000,
            ..Default::default()
        };
        assert_eq!(p.quote("glm-5.3-flash", &t).amount, Some(2_000_000.0));
        assert_eq!(p.quote("zai/glm-5.3-flash", &t).amount, Some(2_000_000.0));
        // 精确名命中时优先于家族兜底
        assert_eq!(p.quote("glm-5-flash", &t).amount, Some(2_000_000.0));
    }

    #[test]
    fn family_name_strips_only_version_minor() {
        assert_eq!(family_name("gpt-6.1-sol").as_deref(), Some("gpt-6-sol"));
        assert_eq!(family_name("glm-5.3-flash").as_deref(), Some("glm-5-flash"));
        assert_eq!(family_name("openai/gpt-5.1-codex-max").as_deref(), Some("openai/gpt-5-codex-max"));
        // 无版本点 / 非数字点段不动
        assert_eq!(family_name("gpt-6-sol"), None);
        assert_eq!(family_name("gpt-4o"), None);
        assert_eq!(family_name("qwen3-235b-a22b"), None);
        assert_eq!(family_name("claude-sonnet-4.5").as_deref(), Some("claude-sonnet-4"));
        // 只剥第一处
        assert_eq!(family_name("x-1.2-y-3.4").as_deref(), Some("x-1-y-3.4"));
    }

    /// 目录缺名时经 canonical 别名表命中:deepseek-flash → deepseek-v4.1-flash。
    #[test]
    fn canonical_alias_recovers_missing_catalog_names() {
        let p = pricing_with(&[("deepseek-v4.1-flash", 1.0, 2.0)]);
        let cost = p
            .quote(
                "deepseek-flash",
                &TokenBreakdown {
                    input: 1_000_000,
                    ..Default::default()
                },
            )
            .amount
            .unwrap();
        assert!((cost - 1_000_000.0).abs() < 1e-9);
    }

    /// 多家同后缀且多数同价 → 取众数价;少数派不拦。
    #[test]
    fn suffix_mode_price_wins_over_minority() {
        let mut entries = HashMap::new();
        for name in ["zai/glm-5.3-flash", "nebius/zai-org/GLM-5.3-Flash", "openrouter/z-ai/glm-5.3-flash"] {
            entries.insert(
                name.to_string(),
                ModelPrice { input: 1.5e-7, output: 5e-7, ..Default::default() },
            );
        }
        entries.insert(
            "aihubmix/glm-5.3-flash".to_string(),
            ModelPrice { input: 1.1268e-7, output: 3.9438e-7, ..Default::default() },
        );
        let p = build(entries);
        let cost = p
            .quote(
                "glm-5.3-flash",
                &TokenBreakdown {
                    input: 1_000_000,
                    output: 1_000_000,
                    ..Default::default()
                },
            )
            .amount
            .unwrap();
        assert!((cost - (0.15 + 0.5)).abs() < 1e-9);
    }

    /// 后缀多家价格彼此不同(无众数) → 仍判歧义拒绝。
    #[test]
    fn truly_ambiguous_suffix_still_rejected() {
        let mut entries = HashMap::new();
        for (i, name) in ["a/shared-model", "b/shared-model", "c/shared-model"].iter().enumerate() {
            entries.insert(
                name.to_string(),
                ModelPrice { input: 1.0 + i as f64, output: 2.0, ..Default::default() },
            );
        }
        let p = build(entries);
        assert!(
            p.quote("shared-model", &TokenBreakdown::default())
                .amount
                .is_none()
        );
    }

    #[test]
    fn ambiguous_suffix_does_not_pick_a_provider_price() {
        let p = build(
            [
                (
                    "provider-a/shared-model".to_string(),
                    ModelPrice {
                        input: 1.0,
                        output: 2.0,
                        ..Default::default()
                    },
                ),
                (
                    "provider-b/shared-model".to_string(),
                    ModelPrice {
                        input: 3.0,
                        output: 4.0,
                        ..Default::default()
                    },
                ),
            ]
            .into_iter()
            .collect(),
        );
        let tokens = TokenBreakdown {
            input: 1_000_000,
            ..Default::default()
        };

        assert_eq!(
            p.quote("provider-a/shared-model", &tokens).amount,
            Some(1_000_000.0)
        );
        assert_eq!(
            p.quote("provider-b/shared-model", &tokens).amount,
            Some(3_000_000.0)
        );
        assert_eq!(p.quote("shared-model", &tokens).amount, None);
    }

    #[test]
    fn exact_unqualified_catalog_entry_wins_over_suffix_collision() {
        let p = build(
            [
                (
                    "shared-model".to_string(),
                    ModelPrice {
                        input: 5.0,
                        ..Default::default()
                    },
                ),
                (
                    "provider-a/shared-model".to_string(),
                    ModelPrice {
                        input: 1.0,
                        ..Default::default()
                    },
                ),
                (
                    "provider-b/shared-model".to_string(),
                    ModelPrice {
                        input: 3.0,
                        ..Default::default()
                    },
                ),
            ]
            .into_iter()
            .collect(),
        );
        let tokens = TokenBreakdown {
            input: 1_000_000,
            ..Default::default()
        };

        assert_eq!(p.quote("shared-model", &tokens).amount, Some(5_000_000.0));
    }

    #[test]
    fn case_insensitive_exact_match_keeps_qualified_provider_price() {
        let p = build(
            [(
                "openai/gpt-6-sol".to_string(),
                ModelPrice {
                    input: 0.000002,
                    output: 0.00001,
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
        );
        let tokens = TokenBreakdown {
            input: 1_000_000,
            ..Default::default()
        };

        assert_eq!(p.quote("OpenAI/GPT-6-SOL", &tokens).amount, Some(2.0));
    }

    /// 路由层命名(group/auto-*):点分版本被消毒成连字符再套 auto- 路由前缀。
    /// canonical 还原后应走既有价路:反向别名(deepseek-flash)、官方条目
    /// (zai/glm-5.3-flash)、补充价(mimo-v2.6-flash)。
    #[test]
    fn group_auto_router_names_price_via_canonical() {
        // 真实目录形态:裸 deepseek-v4.1-flash 缺,只有推广短名;glm 仅有官方条目
        let official_ds = ModelPrice {
            input: 3e-7,
            output: 1.2e-6,
            cache_read: Some(6e-9),
            cache_write: Some(0.0),
        };
        let p = build(HashMap::from([
            ("deepseek-flash".to_string(), official_ds),
            ("deepseek/deepseek-flash".to_string(), official_ds),
            (
                "zai/glm-5.3-flash".to_string(),
                ModelPrice {
                    input: 1.5e-7,
                    output: 5e-7,
                    cache_read: Some(3e-8),
                    cache_write: Some(0.0),
                },
            ),
        ]));
        let t = TokenBreakdown {
            input: 1_000_000,
            output: 1_000_000,
            cache_read: 1_000_000,
            ..Default::default()
        };
        // deepseek:反向别名 deepseek-flash,0.3 + 1.2 + 0.006
        let ds = p
            .quote("group/auto-deepseek-v4-1-flash", &t)
            .amount
            .expect("router deepseek");
        assert!((ds - (0.3 + 1.2 + 0.006)).abs() < 1e-9);
        // glm:官方条目兜底,0.15 + 0.5 + 0.03
        let glm = p
            .quote("group/auto-glm-5-3-flash", &t)
            .amount
            .expect("router glm");
        assert!((glm - (0.15 + 0.5 + 0.03)).abs() < 1e-9);
        // mimo:补充价 0.14 + 0.28 + 0.0028
        let mimo = p
            .quote("group/auto-mimo-v2-6-flash", &t)
            .amount
            .expect("router mimo");
        assert!((mimo - 0.4228).abs() < 1e-6);
    }

    #[test]
    fn supplement_prices_fill_missing_models() {
        let p = build(HashMap::new());
        let t = TokenBreakdown {
            input: 1_000_000,
            output: 1_000_000,
            cache_read: 1_000_000,
            ..Default::default()
        };
        // mimo-v2.6-flash: 1e6 * 1.4e-7 + 1e6 * 2.8e-7 + 1e6 * 2.8e-9 = 0.14 + 0.28 + 0.0028 = 0.4228
        let cost_mimo = p.quote("mimo-v2.6-flash", &t).amount.unwrap();
        assert!((cost_mimo - 0.4228).abs() < 1e-6);

        // xiaomi/mimo-v2.6-flash 匹配
        let cost_xiaomi = p.quote("xiaomi/mimo-v2.6-flash", &t).amount.unwrap();
        assert!((cost_xiaomi - 0.4228).abs() < 1e-6);

        // hy3: 1e6 * 1.3e-7 + 1e6 * 5.3e-7 + 1e6 * 3.3e-8 = 0.13 + 0.53 + 0.033 = 0.693
        let cost_hy3 = p.quote("hy3", &t).amount.unwrap();
        assert!((cost_hy3 - 0.693).abs() < 1e-6);

        // gpt-6.1-sol(LiteLLM 收录空窗期补的 OpenRouter 官价):
        // 1e6 * 2e-6 + 1e6 * 1e-5 + 1e6 * 1e-7 = 2 + 10 + 0.1 = 12.1
        let cost_61 = p.quote("gpt-6.1-sol", &t).amount.unwrap();
        assert!((cost_61 - 12.1).abs() < 1e-9);
        let cost_61_prefixed = p.quote("openai/gpt-6.1-sol", &t).amount.unwrap();
        assert!((cost_61_prefixed - 12.1).abs() < 1e-9);

        // 官方限免/隐身模型为 0 元
        let cost_free = p.quote("big-pickle", &t).amount.unwrap();
        assert_eq!(cost_free, 0.0);
        let cost_bunny = p.quote("space-bunny-free", &t).amount.unwrap();
        assert_eq!(cost_bunny, 0.0);

        // 内部审查模型不入表，保持缺价
        assert!(p.quote("codex-auto-review", &t).amount.is_none());
    }

    #[test]
    fn supplement_does_not_override_existing_litellm_entry() {
        let mut entries = HashMap::new();
        entries.insert(
            "mimo-v2.6-flash".to_string(),
            ModelPrice {
                input: 9.0,
                output: 9.0,
                ..Default::default()
            },
        );
        let p = build(entries);
        let t = TokenBreakdown {
            input: 1_000_000,
            ..Default::default()
        };
        let cost = p.quote("mimo-v2.6-flash", &t).amount.unwrap();
        assert!((cost - 9_000_000.0).abs() < 1e-6);
    }
    fn estimated_basis(quote: PriceQuote) -> EstimateBasis {
        match quote.basis {
            PriceBasis::Estimated(basis) => basis,
            other => panic!("expected estimated basis, got {other:?}"),
        }
    }

    #[test]
    fn quote_explains_alias_reference_suffix_and_family_matches() {
        let cases: &[(&[(&str, f64, f64)], &str, &str, MatchMethod, bool)] = &[
            (
                &[("deepseek-v4.1-flash", 1.0, 2.0)],
                "deepseek-flash",
                "deepseek-v4.1-flash",
                MatchMethod::ExplicitAlias,
                false,
            ),
            (
                &[("glm-5.3-flash", 1.0, 2.0)],
                "openrouter/z-ai/GLM-Flash-Latest",
                "glm-5.3-flash",
                MatchMethod::ExplicitAlias,
                false,
            ),
            (
                &[("deepseek-flash", 1.0, 2.0)],
                "deepseek-v4.1-flash",
                "deepseek-flash",
                MatchMethod::ReverseAlias,
                false,
            ),
            (
                &[("plain-model", 1.0, 2.0)],
                "provider/plain-model",
                "plain-model",
                MatchMethod::Canonical,
                false,
            ),
            (
                &[("gemini-3.8-flash", 1.0, 2.0)],
                "gemini-3.8-flash-high",
                "gemini-3.8-flash",
                MatchMethod::Canonical,
                false,
            ),
            (
                &[("gemini/gemini-3.8-flash", 1.0, 2.0)],
                "gemini-3.8-flash",
                "gemini/gemini-3.8-flash",
                MatchMethod::OfficialReference,
                false,
            ),
            (
                &[("vendor/unique-model", 1.0, 2.0)],
                "unique-model",
                "vendor/unique-model",
                MatchMethod::UniqueSuffix,
                false,
            ),
            (
                &[("glm-5-flash", 1.0, 2.0)],
                "glm-5.3-flash",
                "glm-5-flash",
                MatchMethod::Exact,
                true,
            ),
        ];
        let tokens = TokenBreakdown {
            input: 1,
            ..Default::default()
        };
        for (entries, model, entry, method, family) in cases {
            let quote = pricing_with(entries).quote(model, &tokens);
            assert_eq!(quote.amount, Some(1.0), "{model}");
            let basis = estimated_basis(quote);
            assert_eq!(basis.matched_entry, *entry, "{model}");
            for rate in [
                &basis.input,
                &basis.output,
                basis.cache_read.as_ref().unwrap(),
            ] {
                assert_eq!(rate.entry, *entry, "{model}");
                assert_eq!(rate.origin, PriceOrigin::LegacyUnknown, "{model}");
            }
            assert_eq!(basis.match_method, *method, "{model}");
            assert_eq!(basis.family_fallback, *family, "{model}");
        }
    }

    #[test]
    fn majority_reference_selects_a_stable_named_entry_without_overriding_channels() {
        let entries = [
            ("z/shared-model", 3.0, 2.0),
            ("b/shared-model", 1.0, 2.0),
            ("a/shared-model", 1.0, 2.0),
        ];
        let tokens = TokenBreakdown {
            input: 1,
            ..Default::default()
        };
        let first = pricing_with(&entries).quote("shared-model", &tokens);
        let second = pricing_with(&entries.into_iter().rev().collect::<Vec<_>>())
            .quote("shared-model", &tokens);
        assert_eq!(first, second);
        assert_eq!(first.amount, Some(1.0));
        let basis = estimated_basis(first);
        assert_eq!(basis.match_method, MatchMethod::MajorityReference);
        assert_eq!(basis.matched_entry, "a/shared-model");
        for rate in [
            &basis.input,
            &basis.output,
            basis.cache_read.as_ref().unwrap(),
        ] {
            assert_eq!(rate.entry, "a/shared-model");
            assert_eq!(rate.origin, PriceOrigin::LegacyUnknown);
        }
        let direct = pricing_with(&entries).quote("z/shared-model", &tokens);
        assert_eq!(direct.amount, Some(3.0));
        assert_eq!(estimated_basis(direct).match_method, MatchMethod::Exact);
    }

    #[test]
    fn quote_unpriced_reasons_preserve_attempted_rates_and_explicit_zero() {
        let price = ModelPrice {
            input: 1.0,
            output: 2.0,
            ..Default::default()
        };
        let p = build(HashMap::from([("test".into(), price)]));
        for (read, write, reason) in [
            (5, 0, UnpricedReason::MissingCacheRead),
            (0, 5, UnpricedReason::MissingCacheWrite),
            (5, 5, UnpricedReason::MissingCacheReadAndWrite),
        ] {
            let quote = p.quote(
                "test",
                &TokenBreakdown {
                    input: 10,
                    cache_read: read,
                    cache_write: write,
                    ..Default::default()
                },
            );
            assert_eq!(quote.amount, None);
            let PriceBasis::Unpriced {
                reason: actual,
                attempted: Some(basis),
            } = quote.basis
            else {
                panic!("missing consumed bucket must refuse the whole estimate");
            };
            assert_eq!(actual, reason);
            assert_eq!(basis.input.entry, "test");
            assert_eq!(basis.input.usd_per_token, 1.0);
        }
        let zero = build(HashMap::from([(
            "test".into(),
            ModelPrice {
                cache_read: Some(0.0),
                cache_write: Some(0.0),
                ..price
            },
        )]));
        let quote = zero.quote(
            "test",
            &TokenBreakdown {
                input: 10,
                cache_read: 5,
                cache_write: 5,
                ..Default::default()
            },
        );
        assert_eq!(quote.amount, Some(10.0));
        assert_eq!(
            estimated_basis(quote).cache_write.unwrap().usd_per_token,
            0.0
        );
        assert_eq!(
            p.quote("test", &TokenBreakdown::default()).amount,
            Some(0.0)
        );
        assert_eq!(
            p.quote("missing", &TokenBreakdown::default()).basis,
            PriceBasis::Unpriced {
                reason: UnpricedReason::UnknownModel,
                attempted: None
            }
        );
        let ambiguous = pricing_with(&[("Vendor/Test", 1.0, 2.0), ("vendor/test", 1.0, 2.0)]);
        assert_eq!(
            ambiguous
                .quote("VENDOR/TEST", &TokenBreakdown::default())
                .basis,
            PriceBasis::Unpriced {
                reason: UnpricedReason::AmbiguousName,
                attempted: None
            }
        );
        let ambiguous = pricing_with(&[("a/shared", 1.0, 2.0), ("b/shared", 3.0, 2.0)]);
        assert_eq!(
            ambiguous.quote("shared", &TokenBreakdown::default()).basis,
            PriceBasis::Unpriced {
                reason: UnpricedReason::AmbiguousSuffix,
                attempted: None
            }
        );
        let huge = build(HashMap::from([(
            "huge".into(),
            ModelPrice {
                input: f64::MAX,
                ..Default::default()
            },
        )]));
        assert!(matches!(
            huge.quote(
                "huge",
                &TokenBreakdown {
                    input: 2,
                    ..Default::default()
                }
            )
            .basis,
            PriceBasis::Unpriced {
                reason: UnpricedReason::NonFiniteAmount,
                attempted: Some(_)
            }
        ));
    }

    #[test]
    fn quote_keeps_channel_buckets_and_fills_only_consumed_missing_cache_rates() {
        let channel = "openrouter/z-ai/glm-5.3-flash";
        let official = "zai/glm-5.3-flash";
        let main = ModelPrice {
            input: 2.0,
            output: 3.0,
            cache_read: None,
            cache_write: Some(0.0),
        };
        let reference = ModelPrice {
            input: 1.0,
            output: 1.0,
            cache_read: Some(0.1),
            cache_write: Some(5.0),
        };
        let p = build(HashMap::from([
            (channel.into(), main),
            (official.into(), reference),
        ]));
        let tokens = TokenBreakdown {
            input: 1,
            output: 1,
            cache_read: 1,
            cache_write: 1,
            ..Default::default()
        };
        let quote = p.quote(channel, &tokens);
        assert_eq!(quote.amount, Some(5.1));
        let basis = estimated_basis(quote);
        assert_eq!(basis.matched_entry, channel);
        assert_eq!(basis.match_method, MatchMethod::Exact);
        assert!(basis.cache_fill);
        for rate in [
            &basis.input,
            &basis.output,
            basis.cache_read.as_ref().unwrap(),
            basis.cache_write.as_ref().unwrap(),
        ] {
            assert_eq!(rate.origin, PriceOrigin::LegacyUnknown);
        }
        assert_eq!(
            (basis.input.entry.as_str(), basis.input.usd_per_token),
            (channel, 2.0)
        );
        assert_eq!(
            (basis.output.entry.as_str(), basis.output.usd_per_token),
            (channel, 3.0)
        );
        let read = basis.cache_read.unwrap();
        assert_eq!((read.entry.as_str(), read.usd_per_token), (official, 0.1));
        let write = basis.cache_write.unwrap();
        assert_eq!((write.entry.as_str(), write.usd_per_token), (channel, 0.0));
        let unused = estimated_basis(p.quote(
            channel,
            &TokenBreakdown {
                input: 1,
                ..Default::default()
            },
        ));
        assert!(!unused.cache_fill);
        assert!(unused.cache_read.is_none());
        let p = build(HashMap::from([
            (
                channel.into(),
                ModelPrice {
                    cache_write: None,
                    ..main
                },
            ),
            (official.into(), reference),
        ]));
        let consumed = estimated_basis(p.quote(
            channel,
            &TokenBreakdown {
                cache_read: 1,
                ..Default::default()
            },
        ));
        assert!(consumed.cache_fill);
        assert!(consumed.cache_write.is_none());
        let p = build(HashMap::from([
            (
                channel.into(),
                ModelPrice {
                    cache_read: Some(0.25),
                    ..main
                },
            ),
            (official.into(), reference),
        ]));
        let complete = p.quote(channel, &tokens);
        assert_eq!(complete.amount, Some(5.25));
        assert!(!estimated_basis(complete).cache_fill);
    }

    #[test]
    fn legacy_channel_and_supplement_cache_fill_keep_different_origins() {
        let home = TestHome::new();
        std::fs::write(
            cache_path(&home.0),
            br#"{"fetched_at_ms":1,"models":{"vendor/gpt-6.1-sol":{"input":2,"output":5}}}"#,
        )
        .unwrap();
        let p = Pricing::load(&home.0);
        let quote = p.quote(
            "vendor/gpt-6.1-sol",
            &TokenBreakdown {
                input: 1,
                cache_read: 1,
                ..Default::default()
            },
        );
        assert_eq!(quote.amount, Some(2.0000001));
        let basis = estimated_basis(quote);
        assert!(basis.cache_fill);
        assert_eq!(basis.input.origin, PriceOrigin::LegacyUnknown);
        assert_eq!(basis.input.entry, "vendor/gpt-6.1-sol");
        assert_eq!(basis.output.entry, "vendor/gpt-6.1-sol");
        assert_eq!(basis.output.origin, PriceOrigin::LegacyUnknown);
        assert_eq!(basis.output.usd_per_token, 5.0);
        let cache = basis.cache_read.unwrap();
        assert_eq!(cache.origin, PriceOrigin::Supplement);
        assert_eq!(cache.entry, "gpt-6.1-sol");
        assert_eq!(cache.usd_per_token, 1e-7);
    }

    #[test]
    fn cache_load_distinguishes_missing_corrupt_legacy_and_stale_documents() {
        let home = TestHome::new();
        let path = cache_path(&home.0);
        let missing = Pricing::load(&home.0);
        assert_eq!(missing.snapshot().source, SnapshotSource::SupplementsOnly);
        assert_eq!(missing.snapshot().cache_state, CacheState::Missing);
        assert_eq!(missing.snapshot().fetched_at_ms, None);
        assert_eq!(missing.snapshot().dataset_url, None);
        assert_eq!(
            missing.snapshot().version,
            Pricing::default().snapshot().version
        );
        assert_eq!(
            estimated_basis(missing.quote("hy3", &TokenBreakdown::default()))
                .input
                .origin,
            PriceOrigin::Supplement
        );
        for bad in [
            "{",
            "{}",
            r#"{"models":{}}"#,
            r#"{"schema_version":3,"models":{"test":{"input":1,"output":2}}}"#,
            r#"{"schema_version":2,"models":{"test":{"input":1,"output":2}}}"#,
            r#"{"models":{"test":{"input":-1,"output":2}}}"#,
        ] {
            std::fs::write(&path, bad).unwrap();
            let corrupt = Pricing::load(&home.0);
            assert_eq!(corrupt.snapshot().cache_state, CacheState::Corrupt, "{bad}");
            assert_eq!(corrupt.snapshot().source, SnapshotSource::SupplementsOnly);
            assert_eq!(
                estimated_basis(corrupt.quote("hy3", &TokenBreakdown::default()))
                    .input
                    .origin,
                PriceOrigin::Supplement
            );
        }
        std::fs::write(
            &path,
            br#"{"models":{"mimo-v2.6-flash":{"input":9,"output":9}}}"#,
        )
        .unwrap();
        let legacy = Pricing::load(&home.0);
        assert_eq!(legacy.snapshot().source, SnapshotSource::LegacyCache);
        assert_eq!(legacy.snapshot().cache_state, CacheState::UnknownAge);
        assert_eq!(legacy.snapshot().fetched_at_ms, None);
        assert_eq!(legacy.snapshot().dataset_url, None);
        assert_eq!(
            estimated_basis(legacy.quote("mimo-v2.6-flash", &TokenBreakdown::default()))
                .input
                .origin,
            PriceOrigin::LegacyUnknown
        );
        assert_eq!(
            estimated_basis(legacy.quote("hy3", &TokenBreakdown::default()))
                .input
                .origin,
            PriceOrigin::Supplement
        );
        let mut doc = upstream_doc(HashMap::from([(
            "test".into(),
            ModelPrice {
                input: 1.0,
                ..Default::default()
            },
        )]));
        doc.fetched_at_ms = Some(1_000);
        write_cache(&path, &doc).unwrap();
        let interval = REFRESH_INTERVAL.as_millis() as i64;
        assert_eq!(
            read_cache(&path, 1_000 + interval - 1)
                .unwrap()
                .2
                .cache_state,
            CacheState::Fresh
        );
        assert_eq!(
            read_cache(&path, 1_000 + interval).unwrap().2.cache_state,
            CacheState::Stale
        );
        let stale = Pricing::load(&home.0);
        assert_eq!(stale.snapshot().cache_state, CacheState::Stale);
        assert_eq!(
            stale
                .quote(
                    "test",
                    &TokenBreakdown {
                        input: 1,
                        ..Default::default()
                    }
                )
                .amount,
            Some(1.0)
        );
    }

    #[test]
    fn cache_version_tracks_content_and_origins_not_fetch_clocks_or_map_order() {
        let home = TestHome::new();
        let path = cache_path(&home.0);
        let models = [
            (
                "a".to_string(),
                ModelPrice {
                    input: 1.0,
                    ..Default::default()
                },
            ),
            (
                "b".to_string(),
                ModelPrice {
                    input: 2.0,
                    ..Default::default()
                },
            ),
        ];
        let mut doc = upstream_doc(models.clone().into());
        doc.fetched_at_ms = Some(1_000);
        write_cache(&path, &doc).unwrap();
        let first = Pricing::load(&home.0).snapshot().version.clone();
        doc.models = models.into_iter().rev().collect();
        doc.fetched_at_ms = Some(2_000);
        write_cache(&path, &doc).unwrap();
        assert_eq!(Pricing::load(&home.0).snapshot().version, first);
        doc.models.get_mut("a").unwrap().input = 3.0;
        write_cache(&path, &doc).unwrap();
        assert_ne!(Pricing::load(&home.0).snapshot().version, first);
        doc.models.get_mut("a").unwrap().input = 1.0;
        doc.dataset_url = Some(PRICING_URLS[0].into());
        write_cache(&path, &doc).unwrap();
        assert_ne!(Pricing::load(&home.0).snapshot().version, first);
        doc.schema_version = None;
        std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        let legacy = Pricing::load(&home.0);
        assert_eq!(legacy.snapshot().source, SnapshotSource::LegacyCache);
        assert_ne!(legacy.snapshot().version, first);
    }

    #[test]
    fn invalid_upstream_documents_cannot_replace_a_valid_cache() {
        let home = TestHome::new();
        let path = cache_path(&home.0);
        let mut doc = upstream_doc(HashMap::from([(
            "test".into(),
            ModelPrice {
                input: 1.0,
                ..Default::default()
            },
        )]));
        write_cache(&path, &doc).unwrap();
        let previous = std::fs::read(&path).unwrap();
        for invalid in [-1.0, f64::NAN, f64::INFINITY] {
            doc.models.get_mut("test").unwrap().input = invalid;
            assert!(write_cache(&path, &doc).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), previous);
        }
        doc.models.get_mut("test").unwrap().input = 1.0;
        doc.dataset_url = None;
        assert!(write_cache(&path, &doc).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), previous);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn concurrent_cache_replacements_publish_complete_unique_documents() {
        let home = TestHome::new();
        let path = cache_path(&home.0);
        let temporary_names = std::sync::Mutex::new(HashSet::new());
        std::thread::scope(|scope| {
            for rate in 1..=4 {
                let path = &path;
                let names = &temporary_names;
                scope.spawn(move || {
                    let doc = upstream_doc(HashMap::from([(
                        "test".into(),
                        ModelPrice {
                            input: rate as f64,
                            ..Default::default()
                        },
                    )]));
                    publish_cache(path, &doc, |temporary, target| {
                        let ready: CacheDoc =
                            serde_json::from_slice(&std::fs::read(temporary).unwrap()).unwrap();
                        assert_eq!(ready.models["test"].input, rate as f64);
                        assert!(names.lock().unwrap().insert(temporary.to_owned()));
                        std::fs::rename(temporary, target)
                    })
                    .unwrap();
                });
            }
        });
        assert_eq!(temporary_names.into_inner().unwrap().len(), 4);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        let quote = Pricing::load(&home.0).quote(
            "test",
            &TokenBreakdown {
                input: 1,
                ..Default::default()
            },
        );
        assert!([Some(1.0), Some(2.0), Some(3.0), Some(4.0)].contains(&quote.amount));
    }
}
