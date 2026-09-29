//! 对本机真实凭据跑一遍配额快照,打印每张卡的结果与缺失原因。
//! 用于真机验收各配额卡(含代理、登录态、CPA 链路)。

use tui_token_monitor::model::{Card, Snapshot};
use tui_token_monitor::{backend, model};

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
                for limit in &report.limits {
                    let reset = match limit.resets_in_ms {
                        Some(ms) => format!(" · {}s 后重置", ms / 1000),
                        None => String::new(),
                    };
                    println!("    {:<22} {:>5.1}%{}", limit.label, limit.used_pct, reset);
                }
            }
            Card::Missing { provider } => {
                println!("○ {} 缺卡:{}", provider.title(), provider.missing_hint());
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
