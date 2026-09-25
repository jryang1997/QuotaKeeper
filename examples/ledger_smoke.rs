//! 冒烟:对本机真实数据源跑一遍 ledger::collect,打印结果.仅用于开发验证.

use chrono::Local;

fn main() {
    let rep = tui_token_monitor::ledger::collect(Local::now());
    println!("== sources ==");
    for s in &rep.sources {
        match &s.error {
            Some(e) => println!("  {:<10} ERR {e}", s.client),
            None => println!("  {:<10} {} 条", s.client, s.records),
        }
    }
    println!(
        "今日 {} tok ${:.4} · 本周 {} tok ${:.4} · 本月 {} tok ${:.4}",
        rep.today.total(),
        rep.today.cost(),
        rep.week.total(),
        rep.week.cost(),
        rep.month.total(),
        rep.month.cost()
    );
    println!("== 近30天按agent ==");
    for a in &rep.by_agent {
        println!(
            "  {:<10} 总 {} tok, {} 条, ${:.4}, 模型数 {}",
            a.client,
            a.agg.total(),
            a.agg.messages,
            a.agg.cost(),
            a.models.len()
        );
        for (m, agg) in a.models.iter().take(3) {
            println!("      {m}  {} tok  ${:.4}", agg.total(), agg.cost());
        }
    }
    println!("== 近5天 ==");
    for d in rep.by_day.iter().rev().take(5) {
        println!(
            "  {}  {} tok  ${:.4}  来源数 {}",
            d.date,
            d.agg.total(),
            d.agg.cost(),
            d.per_client.len()
        );
    }
}
