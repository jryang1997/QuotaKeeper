//! LiteLLM 开放价目数据集估价(与 tokscale 同源),带 7 天磁盘缓存.
//!
//! - 缓存:`%USERPROFILE%\.tui-token-monitor\litellm-pricing.json`(HOME 缺失则不落盘);
//! - 新鲜期内直接读缓存;过期/缺失才联网拉,失败回退旧缓存,全失败返回空表(显示 "—");
//! - 查找顺序:精确模型名 → 唯一的大小写不敏感全名 → 唯一的 provider 后缀。
//!   多个 provider 同后缀时不估价，避免套用不确定的费率。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::types::TokenBreakdown;

const PRICING_URLS: [&str; 2] = [
    // jsdelivr CDN 在不稳定链路下更可靠,放首位;raw.githubusercontent 兜底
    "https://cdn.jsdelivr.net/gh/BerriAI/litellm@main/model_prices_and_context_window.json",
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json",
];
const TTL_MS: i64 = 7 * 24 * 3_600 * 1_000;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, Default)]
pub struct ModelPrice {
    #[serde(default)]
    input: f64,
    #[serde(default)]
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

impl Pricing {
    /// 永不失败:新鲜缓存 → 联网刷新 → 旧缓存 → 空表.
    pub fn load(home: &Path, now_ms: i64) -> Pricing {
        let path = cache_path(home);
        let cached = read_cache(&path);
        if let Some((fetched_at, _)) = cached.as_ref()
            && now_ms - fetched_at < TTL_MS
        {
            return cached.unwrap().1;
        }
        match fetch() {
            Some(fresh) => {
                let _ = write_cache(&path, now_ms, &fresh);
                fresh
            }
            None => cached.map(|(_, p)| p).unwrap_or_default(),
        }
    }

    pub fn estimate(&self, model: &str, t: &TokenBreakdown) -> Option<f64> {
        let p = self.lookup(model)?;
        let mut cost = t.input as f64 * p.input + t.output as f64 * p.output;
        if let Some(r) = p.cache_read {
            cost += t.cache_read as f64 * r;
        }
        if let Some(w) = p.cache_write {
            cost += t.cache_write as f64 * w;
        }
        Some(cost)
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

        let suffix = lower.rsplit('/').next().unwrap_or(&lower);
        if self.ambiguous_suffixes.contains(suffix) {
            return None;
        }
        self.unique_suffixes.get(suffix)
    }
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
    let resp = ureq::get(url).timeout(TIMEOUT).call().ok()?;
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

/// Index exact and unique suffix names only. LiteLLM contains many providers for
/// the same model ID, often with different prices; choosing the first suffix
/// encountered in a HashMap would make the estimate nondeterministic.
fn build(entries: HashMap<String, ModelPrice>) -> Pricing {
    let mut case_insensitive = HashMap::new();
    let mut ambiguous_names = HashSet::new();
    let mut unique_suffixes = HashMap::new();
    let mut ambiguous_suffixes = HashSet::new();

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
        if !ambiguous_suffixes.contains(&suffix) {
            if unique_suffixes.remove(&suffix).is_some() {
                ambiguous_suffixes.insert(suffix);
            } else {
                unique_suffixes.insert(suffix, *price);
            }
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
}
