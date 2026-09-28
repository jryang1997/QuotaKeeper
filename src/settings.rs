//! 本地偏好设置。只保存显示基准等非敏感选项，不写入订阅凭据。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_MONTHLY_PLAN_USD: f64 = 40.0;
const FILE_NAME: &str = "settings.json";
const MAX_MONTHLY_PLAN_USD: f64 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanBenchmark {
    pub monthly_usd: f64,
    pub is_custom: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct SettingsFile {
    monthly_plan_usd: Option<f64>,
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

pub fn settings_path(home: &Path) -> PathBuf {
    home.join(".tui-token-monitor").join(FILE_NAME)
}

pub fn monthly_plan_benchmark() -> PlanBenchmark {
    home_dir()
        .map(|home| load_monthly_plan_benchmark(&home))
        .unwrap_or(PlanBenchmark {
            monthly_usd: DEFAULT_MONTHLY_PLAN_USD,
            is_custom: false,
        })
}

pub fn load_monthly_plan_benchmark(home: &Path) -> PlanBenchmark {
    let custom = std::fs::read_to_string(settings_path(home))
        .ok()
        .and_then(|text| serde_json::from_str::<SettingsFile>(&text).ok())
        .and_then(|settings| settings.monthly_plan_usd)
        .filter(|value| valid_monthly_plan(*value));

    match custom {
        Some(monthly_usd) => PlanBenchmark {
            monthly_usd,
            is_custom: true,
        },
        None => PlanBenchmark {
            monthly_usd: DEFAULT_MONTHLY_PLAN_USD,
            is_custom: false,
        },
    }
}

pub fn save_monthly_plan_benchmark(monthly_usd: f64) -> std::io::Result<()> {
    let home = home_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "home directory not found")
    })?;
    save_monthly_plan_benchmark_at(&home, monthly_usd)
}

pub fn save_monthly_plan_benchmark_at(home: &Path, monthly_usd: f64) -> std::io::Result<()> {
    if !valid_monthly_plan(monthly_usd) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "monthly plan price must be between 0 and 1,000,000 USD",
        ));
    }

    let path = settings_path(home);
    let mut settings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<SettingsFile>(&text).ok())
        .unwrap_or_default();
    settings.monthly_plan_usd = Some(monthly_usd);
    write_settings(&path, &settings)
}

pub fn reset_monthly_plan_benchmark() -> std::io::Result<()> {
    let home = home_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "home directory not found")
    })?;
    reset_monthly_plan_benchmark_at(&home)
}

pub fn reset_monthly_plan_benchmark_at(home: &Path) -> std::io::Result<()> {
    let path = settings_path(home);
    let mut settings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<SettingsFile>(&text).ok())
        .unwrap_or_default();
    settings.monthly_plan_usd = None;
    write_settings(&path, &settings)
}

fn valid_monthly_plan(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value <= MAX_MONTHLY_PLAN_USD
}

fn write_settings(path: &Path, settings: &SettingsFile) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(settings)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home() -> PathBuf {
        std::env::temp_dir().join(format!(
            "tui-token-monitor-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn custom_benchmark_is_saved_loaded_and_reset() {
        let home = temp_home();
        assert_eq!(
            load_monthly_plan_benchmark(&home),
            PlanBenchmark {
                monthly_usd: DEFAULT_MONTHLY_PLAN_USD,
                is_custom: false,
            }
        );

        save_monthly_plan_benchmark_at(&home, 28.5).unwrap();
        assert_eq!(
            load_monthly_plan_benchmark(&home),
            PlanBenchmark {
                monthly_usd: 28.5,
                is_custom: true,
            }
        );

        reset_monthly_plan_benchmark_at(&home).unwrap();
        assert_eq!(
            load_monthly_plan_benchmark(&home),
            PlanBenchmark {
                monthly_usd: DEFAULT_MONTHLY_PLAN_USD,
                is_custom: false,
            }
        );
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn invalid_price_is_rejected_and_bad_saved_value_uses_default() {
        let home = temp_home();
        assert!(save_monthly_plan_benchmark_at(&home, f64::NAN).is_err());
        assert!(save_monthly_plan_benchmark_at(&home, 0.0).is_err());
        assert!(save_monthly_plan_benchmark_at(&home, MAX_MONTHLY_PLAN_USD + 1.0).is_err());

        let path = settings_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"monthly_plan_usd":-3}"#).unwrap();
        assert_eq!(
            load_monthly_plan_benchmark(&home),
            PlanBenchmark {
                monthly_usd: DEFAULT_MONTHLY_PLAN_USD,
                is_custom: false,
            }
        );
        let _ = std::fs::remove_dir_all(home);
    }
}
