//! LiteLLM 开放价目数据集估价(与 tokscale 同源),带 7 天磁盘缓存.
//!
//! - 缓存:`%USERPROFILE%\.tui-token-monitor\litellm-pricing.json`(HOME 缺失则不落盘);
//! - 新鲜期内直接读缓存;过期/缺失才联网拉,失败回退旧缓存,全失败返回空表(显示 "—");
//! - 查找顺序:原名 → 小写 → 去掉 `provider/` 前缀 → 小写前缀(OMP 的模型如
//!   `deepseek/deepseek-v4.1-flash` 走后缀命中;OMP 有报告价,不走这里).

use std::collections::HashMap;
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
        let lower = model.to_lowercase();
        let suffix = lower.rsplit('/').next().unwrap_or(&lower);
        for key in [model, lower.as_str(), suffix] {
            if let Some(p) = self.entries.get(key) {
                return Some(p);
            }
        }
        None
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

/// 后缀索引:客户端上报的模型名常无 provider 前缀(LiteLLM 键是
/// "deepseek/deepseek-v4.1-flash" 这类),按 '/' 后的尾部建一份索引;
/// 后缀冲突时先到先得(全键查找优先,不受影响).
fn build(mut entries: HashMap<String, ModelPrice>) -> Pricing {
    let mut suffixes: HashMap<String, String> = HashMap::new();
    for name in entries.keys() {
        if let Some(suf) = name.rsplit('/').next()
            && suf != name.as_str()
            && !suffixes.contains_key(suf)
        {
            suffixes.insert(suf.to_string(), name.clone());
        }
    }
    for (suf, full) in suffixes {
        let price = entries[&full];
        entries.entry(suf).or_insert(price);
    }
    Pricing { entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pricing_with(entries: &[(&str, f64, f64)]) -> Pricing {
        Pricing {
            entries: entries
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
        }
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
}
