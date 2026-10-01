//! LiteLLM 开放价目数据集估价(与 tokscale 同源),磁盘缓存 + 后台自动刷新.
//!
//! - 缓存:`%USERPROFILE%\.tui-token-monitor\litellm-pricing.json`(HOME 缺失则不落盘),读取不联网;
//! - 查找:精确名 → 大小写全名 → canonical 别名 → provider 后缀众数 → 家族兜底;
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

#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, Default)]
pub struct ModelPrice {
    input: f64,
    output: f64,
    #[serde(default)]
    cache_read: Option<f64>,
    #[serde(default)]
    cache_write: Option<f64>,
}

#[derive(Debug, Default)]
pub struct Pricing {
    entries: HashMap<String, ModelPrice>,
    case_insensitive: HashMap<String, ModelPrice>,
    ambiguous_names: HashSet<String>,
    unique_suffixes: HashMap<String, ModelPrice>,
    ambiguous_suffixes: HashSet<String>,
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
struct CacheDoc {
    #[serde(default)]
    fetched_at_ms: i64,
    #[serde(default)]
    models: HashMap<String, ModelPrice>,
}

pub fn cache_path(home: &Path) -> PathBuf {
    home.join(".tui-token-monitor").join("litellm-pricing.json")
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
        .spawn(move || loop {
            if let Some(fresh) = fetch() {
                let _ = write_cache(&cache_path(&home), now_ms(), &fresh);
            }
            std::thread::sleep(REFRESH_INTERVAL);
        });
}

impl Pricing {
    /// 只读本地缓存;冷启动/离线使用补充价,网络刷新由 spawn_refresher 负责。
    pub fn load(home: &Path) -> Pricing {
        let path = cache_path(home);
        if let Some((_, pricing)) = read_cache(&path) {
            return pricing;
        }
        build(HashMap::new())
    }

    pub fn estimate(&self, model: &str, t: &TokenBreakdown) -> Option<f64> {
        let p = self.lookup(model)?;
        let mut cost = t.input as f64 * p.input + t.output as f64 * p.output;
        if t.cache_read > 0 {
            cost += t.cache_read as f64 * p.cache_read?;
        }
        if t.cache_write > 0 {
            cost += t.cache_write as f64 * p.cache_write?;
        }
        cost.is_finite().then_some(cost)
    }

    fn lookup(&self, model: &str) -> Option<&ModelPrice> {
        if model.is_empty() {
            return None;
        }
        if let Some(price) = self.entries.get(model) {
            return Some(price);
        }
        let lower = model.to_lowercase();
        if self.ambiguous_names.contains(&lower) {
            return None;
        }
        if let Some(price) = self.case_insensitive.get(&lower) {
            return Some(price);
        }

        // 保留已知 provider 的价格,再用聚合别名补目录缺名。
        let canonical = crate::ledger::model_alias::canonical(model);
        if canonical != model && canonical != "unknown"
            && let Some(price) = self.entries.get(&canonical)
        {
            return Some(price);
        }

        let suffix = lower.rsplit('/').next().unwrap_or(&lower);
        if self.ambiguous_suffixes.contains(suffix) {
            return None;
        }
        if let Some(price) = self.unique_suffixes.get(suffix) {
            return Some(price);
        }
        // ponytail: 家族价只近似相邻 minor;需审计估价时改为显式匹配来源。
        let family = family_name(model)?;
        self.lookup(&family)
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

fn read_cache(path: &Path) -> Option<(i64, Pricing)> {
    let text = std::fs::read_to_string(path).ok()?;
    let doc: CacheDoc = serde_json::from_str(&text).ok()?;
    Some((doc.fetched_at_ms, build(doc.models)))
}

fn write_cache(path: &Path, now_ms: i64, pricing: &Pricing) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let doc = CacheDoc {
        fetched_at_ms: now_ms,
        models: pricing.entries.clone(),
    };
    std::fs::write(path, serde_json::to_string(&doc).unwrap_or_default())
}

fn fetch() -> Option<Pricing> {
    PRICING_URLS.iter().find_map(|url| fetch_one(url))
}

fn fetch_one(url: &str) -> Option<Pricing> {
    let resp = crate::http::get(url).timeout(TIMEOUT).call().ok()?;
    let doc: HashMap<String, serde_json::Value> = resp.into_json().ok()?;
    let mut entries = HashMap::new();
    for (name, v) in doc {
        let Some(input) = v.get("input_cost_per_token").and_then(|x| x.as_f64()) else {
            continue;
        };
        let Some(output) = v.get("output_cost_per_token").and_then(|x| x.as_f64()) else {
            continue;
        };
        let opt = |k: &str| v.get(k).and_then(|x| x.as_f64()).filter(|f| f.is_finite());
        entries.insert(
            name,
            ModelPrice {
                input,
                output,
                cache_read: opt("cache_read_input_token_cost"),
                cache_write: opt("cache_creation_input_token_cost"),
            },
        );
    }
    if entries.is_empty() {
        return None;
    }
    Some(build(entries))
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

/// Index exact and unique suffix names only. LiteLLM contains many providers for
/// the same model ID, often with different prices; choosing the first suffix
/// encountered in a HashMap would make the estimate nondeterministic.
fn build(mut entries: HashMap<String, ModelPrice>) -> Pricing {
    entries.retain(|_, price| {
        [Some(price.input), Some(price.output), price.cache_read, price.cache_write]
            .into_iter().flatten().all(|value| value.is_finite() && value >= 0.0)
    });
    for (name, price) in SUPPLEMENT {
        entries.entry((*name).to_string()).or_insert(*price);
    }

    let mut case_insensitive = HashMap::new();
    let mut ambiguous_names = HashSet::new();
    let mut unique_suffixes: HashMap<String, ModelPrice> = HashMap::new();
    let mut ambiguous_suffixes = HashSet::new();

    // 后缀 → 同后缀全部条目;若多数价格一致(众数),取众数价而非判歧义。
    // LiteLLM 里同一模型常被 6-9 家供应商收录且价格相同,歧义拒绝会把这些
    // 模型整体挡在定价之外;众数在「少数渠道差异化定价」时仍取主流价。
    let mut suffix_groups: HashMap<String, Vec<&ModelPrice>> = HashMap::new();
    for (name, price) in &entries {
        let lower = name.to_lowercase();
        if !ambiguous_names.contains(&lower) {
            if case_insensitive.remove(&lower).is_some() {
                ambiguous_names.insert(lower);
            } else {
                case_insensitive.insert(lower, *price);
            }
        }
        let suffix = name.rsplit('/').next().unwrap_or(name).to_lowercase();
        suffix_groups.entry(suffix).or_default().push(price);
    }

    for (suffix, group) in suffix_groups {
        if group.len() == 1 {
            unique_suffixes.insert(suffix, *group[0]);
            continue;
        }
        // 以四桶价格全等做众数统计;缺价和明确零价是不同状态。
        let key = |p: &ModelPrice| (
            p.input.to_bits(), p.output.to_bits(),
            p.cache_read.map(f64::to_bits), p.cache_write.map(f64::to_bits),
        );
        let mut counts = HashMap::new();
        for price in &group {
            let e = counts.entry(key(price)).or_insert((0, **price));
            e.0 += 1;
        }
        if let Some((best_n, best_price)) = counts.values().max_by_key(|(n, _)| *n)
            && best_n * 2 > group.len()
        {
            unique_suffixes.insert(suffix, *best_price);
        } else {
            ambiguous_suffixes.insert(suffix);
        }
    }

    Pricing {
        entries,
        case_insensitive,
        ambiguous_names,
        unique_suffixes,
        ambiguous_suffixes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn consumed_cache_buckets_need_prices_and_zero_is_explicit() {
        for json in ["{}", r#"{"input":0}"#, r#"{"output":0}"#] {
            assert!(serde_json::from_str::<ModelPrice>(json).is_err());
        }
        assert!(serde_json::from_str::<ModelPrice>(r#"{"input":0,"output":0}"#).is_ok());
        let price = ModelPrice { input: 1.0, output: 2.0, ..Default::default() };
        let p = build(HashMap::from([("test".into(), price)]));
        let mut t = TokenBreakdown { input: 10, ..Default::default() };
        assert_eq!(p.estimate("test", &t), Some(10.0));
        t.cache_read = 5;
        assert_eq!(p.estimate("test", &t), None);
        t.cache_read = 0;
        t.cache_write = 5;
        assert_eq!(p.estimate("test", &t), None);
        let p = build(HashMap::from([("test".into(), ModelPrice { cache_read: Some(0.0), cache_write: Some(0.0), ..price })]));
        assert_eq!(p.estimate("test", &t), Some(10.0));
        for input in [-1.0, f64::NAN, f64::INFINITY] {
            let p = build(HashMap::from([("invalid".into(), ModelPrice { input, ..price })]));
            assert_eq!(p.estimate("invalid", &t), None);
        }
    }

    #[test]
    fn qualified_case_insensitive_name_precedes_bare_alias() {
        let p = pricing_with(&[("test-model", 1.0, 2.0), ("provider/test-model", 3.0, 4.0)]);
        let t = TokenBreakdown { input: 10, ..Default::default() };
        assert_eq!(p.estimate("Provider/Test-Model", &t), Some(30.0));
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
        let cost = p.estimate("gpt-6-sol", &t).unwrap();
        assert!((cost - 2_850_000.0).abs() < 1e-6);
    }

    #[test]
    fn provider_prefix_falls_back_to_suffix() {
        let p = pricing_with(&[("deepseek-v4.1-flash", 1.0, 2.0)]);
        assert!(
            p.estimate("deepseek/deepseek-v4.1-flash", &TokenBreakdown::default())
                .is_some()
        );
        assert!(
            p.estimate("DeepSeek/deepseek-v4.1-flash", &TokenBreakdown::default())
                .is_some()
        );
        assert!(
            p.estimate("totally-unknown-model", &TokenBreakdown::default())
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
        assert_eq!(p.estimate("glm-5.3-flash", &t), Some(2_000_000.0));
        assert_eq!(p.estimate("zai/glm-5.3-flash", &t), Some(2_000_000.0));
        // 精确名命中时优先于家族兜底
        assert_eq!(p.estimate("glm-5-flash", &t), Some(2_000_000.0));
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
            .estimate("deepseek-flash", &TokenBreakdown {
                input: 1_000_000,
                ..Default::default()
            })
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
            .estimate("glm-5.3-flash", &TokenBreakdown {
                input: 1_000_000,
                output: 1_000_000,
                ..Default::default()
            })
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
        assert!(p.estimate("shared-model", &TokenBreakdown::default()).is_none());
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
            p.estimate("provider-a/shared-model", &tokens),
            Some(1_000_000.0)
        );
        assert_eq!(
            p.estimate("provider-b/shared-model", &tokens),
            Some(3_000_000.0)
        );
        assert_eq!(p.estimate("shared-model", &tokens), None);
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

        assert_eq!(p.estimate("shared-model", &tokens), Some(5_000_000.0));
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

        assert_eq!(p.estimate("OpenAI/GPT-6-SOL", &tokens), Some(2.0));
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
        let cost_mimo = p.estimate("mimo-v2.6-flash", &t).unwrap();
        assert!((cost_mimo - 0.4228).abs() < 1e-6);

        // xiaomi/mimo-v2.6-flash 匹配
        let cost_xiaomi = p.estimate("xiaomi/mimo-v2.6-flash", &t).unwrap();
        assert!((cost_xiaomi - 0.4228).abs() < 1e-6);

        // hy3: 1e6 * 1.3e-7 + 1e6 * 5.3e-7 + 1e6 * 3.3e-8 = 0.13 + 0.53 + 0.033 = 0.693
        let cost_hy3 = p.estimate("hy3", &t).unwrap();
        assert!((cost_hy3 - 0.693).abs() < 1e-6);

        // gpt-6.1-sol(LiteLLM 收录空窗期补的 OpenRouter 官价):
        // 1e6 * 2e-6 + 1e6 * 1e-5 + 1e6 * 1e-7 = 2 + 10 + 0.1 = 12.1
        let cost_61 = p.estimate("gpt-6.1-sol", &t).unwrap();
        assert!((cost_61 - 12.1).abs() < 1e-9);
        let cost_61_prefixed = p.estimate("openai/gpt-6.1-sol", &t).unwrap();
        assert!((cost_61_prefixed - 12.1).abs() < 1e-9);

        // 官方限免/隐身模型为 0 元
        let cost_free = p.estimate("big-pickle", &t).unwrap();
        assert_eq!(cost_free, 0.0);
        let cost_bunny = p.estimate("space-bunny-free", &t).unwrap();
        assert_eq!(cost_bunny, 0.0);

        // 内部审查模型不入表，保持缺价
        assert!(p.estimate("codex-auto-review", &t).is_none());
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
        let cost = p.estimate("mimo-v2.6-flash", &t).unwrap();
        assert!((cost - 9_000_000.0).abs() < 1e-6);
    }
}
