//! 对本机真实凭据跑一遍配额快照,打印每张卡的结果与缺失原因。
//! 用于真机验收各配额卡(含代理、登录态、CPA 链路)。

use quotakeeper::model::{Card, Snapshot};
use quotakeeper::{backend, model};

fn print_snapshot(snap: &Snapshot) {
    println!("生成于 {} ms\n", snap.generated_at_ms);
    for card in &snap.cards {
        match card {
            Card::Live(report) => {
                println!(
                    "● {} [{}]",
                    report.provider.title(),
                    report.source.label()
                );
                println!("    套餐:{}", report.plan_type);
                if let Some(ends) = report.subscription_ends_at_ms {
                    let left_days = (ends - chrono::Utc::now().timestamp_millis()).max(0)
                        / 86_400_000;
                    println!(
                        "    订阅期至:{}(剩 {} 天)",
                        chrono::DateTime::from_timestamp_millis(ends)
                            .map(|dt| dt.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "?".into()),
                        left_days
                    );
                }
                for limit in &report.limits {
                    let reset = match limit.resets_in_ms {
                        Some(ms) => format!(" · {}s 后重置", ms / 1000),
                        None => String::new(),
                    };
                    println!("    {:<22} {:>5.1}%{}", limit.label, limit.used_pct, reset);
                }
            }
            Card::Missing { provider } => {
                println!("○ {} 缺卡:{}", provider.title(), provider.missing_hints()[0]);
            }
        }
    }
}

fn main() {
    match backend::fetch_snapshot() {
        Ok(snap) => print_snapshot(&snap),
        Err(e) => {
            eprintln!("快照失败:{e:#}");
            std::process::exit(1);
        }
    }
}

// model 在此 re-export,便于手改本示例时少一行 use。
#[allow(unused_imports)]
use model as _model_reexport;
