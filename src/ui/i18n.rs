//! 双语(中/英)界面文案:`l` 键切换,选择持久化到 `~/.tui-token-monitor/lang`.
//!
//! 所有界面文字集中在这里;带参数的用小函数(`age`/`msgs`/`rel`…)。
//! 数据层错误串保持原样(诊断信息,不做翻译)。

use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

static LANG_IDX: AtomicUsize = AtomicUsize::new(0); // 0 = Zh

pub fn lang() -> Lang {
    if LANG_IDX.load(Ordering::Relaxed) == 0 {
        Lang::Zh
    } else {
        Lang::En
    }
}

pub fn cycle() {
    LANG_IDX.store(1 - LANG_IDX.load(Ordering::Relaxed), Ordering::Relaxed);
}

pub fn set(name: &str) -> bool {
    match name.to_ascii_lowercase().as_str() {
        "zh" | "中文" => {
            LANG_IDX.store(0, Ordering::Relaxed);
            true
        }
        "en" | "english" => {
            LANG_IDX.store(1, Ordering::Relaxed);
            true
        }
        _ => false,
    }
}

pub fn name() -> &'static str {
    match lang() {
        Lang::Zh => "中文",
        Lang::En => "EN",
    }
}

/// 静态文案表。
#[derive(Debug, Clone, Copy)]
pub enum K {
    Title,
    PageOverview,
    PageQuota,
    PageTokens,
    PageHeat,
    PToday,
    P7,
    P30,
    P90,
    PAll,
    TotalTokens,
    TotalRuns,
    CacheHit,
    SourceHealth,
    DailyActivity,
    LocalLedger,
    Summary,
    Syncing,
    SToday,
    SWeek,
    SMonth,
    Last60,
    QuotaShort,
    QuotaUsed,
    QuotaRemaining,
    Idle,
    LastRecord,
    NoCost,
    HAgent,
    HIn,
    HOut,
    HCache,
    HTotal,
    HCost,
    H14,
    HShare,
    HDate,
    HTop,
    HSession,
    HProject,
    HClient,
    HLast,
    DimAgent,
    DimSession,
    DimProject,
    DimModel,
    HeatTitle,
    WMon,
    WWed,
    WFri,
    Less,
    More,
    ProfileHours,
    ProfileWeekday,
    Morning,
    Daytime,
    Evening,
    Night,
    Peak,
    Detail,
    ByModel,
    NoData,
    Loading,
}

pub fn t(k: K) -> &'static str {
    let zh = lang() == Lang::Zh;
    match k {
        K::Title => {
            if zh {
                "用量监测"
            } else {
                "Monitor"
            }
        }
        K::PageOverview => {
            if zh {
                "总览"
            } else {
                "Overview"
            }
        }
        K::PageQuota => {
            if zh {
                "配额"
            } else {
                "Quota"
            }
        }
        K::PageTokens => {
            if zh {
                "Token 用量"
            } else {
                "Tokens"
            }
        }
        K::PageHeat => {
            if zh {
                "热力图"
            } else {
                "Heatmap"
            }
        }
        K::PToday => {
            if zh {
                "今日"
            } else {
                "Today"
            }
        }
        K::P7 => {
            if zh {
                "7天"
            } else {
                "7d"
            }
        }
        K::P30 => {
            if zh {
                "30天"
            } else {
                "30d"
            }
        }
        K::P90 => {
            if zh {
                "90天"
            } else {
                "90d"
            }
        }
        K::PAll => {
            if zh {
                "全部"
            } else {
                "All"
            }
        }
        K::TotalTokens => {
            if zh {
                "累计 Tokens"
            } else {
                "Total Tokens"
            }
        }
        K::TotalRuns => {
            if zh {
                "累计调用"
            } else {
                "Total Runs"
            }
        }
        K::CacheHit => {
            if zh {
                "缓存命中"
            } else {
                "cache hit"
            }
        }
        K::SourceHealth => {
            if zh {
                "来源健康"
            } else {
                "Source health"
            }
        }
        K::DailyActivity => {
            if zh {
                "每日活动"
            } else {
                "Daily activity"
            }
        }
        K::Summary => {
            if zh {
                "汇总"
            } else {
                "Summary"
            }
        }
        K::LocalLedger => {
            if zh {
                "本机账本"
            } else {
                "local ledger"
            }
        }
        K::Syncing => {
            if zh {
                "刷新中"
            } else {
                "syncing"
            }
        }
        K::Last60 => {
            if zh {
                "近60m"
            } else {
                "Last 60m"
            }
        }
        K::QuotaShort => {
            if zh {
                "额度"
            } else {
                "Quota"
            }
        }
        K::QuotaUsed => {
            if zh {
                "已用"
            } else {
                "Used"
            }
        }
        K::QuotaRemaining => {
            if zh {
                "剩余"
            } else {
                "Remaining"
            }
        }
        K::Idle => {
            if zh {
                "无用量"
            } else {
                "idle"
            }
        }
        K::LastRecord => {
            if zh {
                "最近"
            } else {
                "last"
            }
        }
        K::NoCost => {
            if zh {
                "无金额数据"
            } else {
                "no cost data"
            }
        }
        K::SToday => {
            if zh {
                "今日"
            } else {
                "Today"
            }
        }
        K::SWeek => {
            if zh {
                "本周"
            } else {
                "Week"
            }
        }
        K::SMonth => {
            if zh {
                "本月"
            } else {
                "Month"
            }
        }
        K::HAgent => "Agent",
        K::HIn => {
            if zh {
                "输入"
            } else {
                "In"
            }
        }
        K::HOut => {
            if zh {
                "输出"
            } else {
                "Out"
            }
        }
        K::HCache => {
            if zh {
                "缓存"
            } else {
                "Cache"
            }
        }
        K::HTotal => {
            if zh {
                "总计"
            } else {
                "Total"
            }
        }
        K::HCost => {
            if zh {
                "成本"
            } else {
                "Cost"
            }
        }
        K::H14 => {
            if zh {
                "近14天"
            } else {
                "14d"
            }
        }
        K::HShare => {
            if zh {
                "占比"
            } else {
                "Share"
            }
        }
        K::HDate => {
            if zh {
                "日期"
            } else {
                "Date"
            }
        }
        K::HTop => {
            if zh {
                "主要来源"
            } else {
                "Top"
            }
        }
        K::HSession => {
            if zh {
                "会话"
            } else {
                "Session"
            }
        }
        K::HProject => {
            if zh {
                "项目"
            } else {
                "Project"
            }
        }
        K::HClient => {
            if zh {
                "来源"
            } else {
                "Source"
            }
        }
        K::HLast => {
            if zh {
                "最近"
            } else {
                "Last"
            }
        }
        K::DimAgent => "Agent",
        K::DimSession => {
            if zh {
                "会话"
            } else {
                "Sessions"
            }
        }
        K::DimProject => {
            if zh {
                "项目"
            } else {
                "Projects"
            }
        }
        K::DimModel => {
            if zh {
                "模型"
            } else {
                "Models"
            }
        }
        K::HeatTitle => {
            if zh {
                "26 周 · 每日"
            } else {
                "26w · daily"
            }
        }
        K::WMon => {
            if zh {
                "周一"
            } else {
                "Mon"
            }
        }
        K::WWed => {
            if zh {
                "周三"
            } else {
                "Wed"
            }
        }
        K::WFri => {
            if zh {
                "周五"
            } else {
                "Fri"
            }
        }
        K::Less => {
            if zh {
                "少"
            } else {
                "Less"
            }
        }
        K::More => {
            if zh {
                "多"
            } else {
                "More"
            }
        }
        K::ProfileHours => {
            if zh {
                "时段分布"
            } else {
                "By hour"
            }
        }
        K::ProfileWeekday => {
            if zh {
                "星期分布"
            } else {
                "By weekday"
            }
        }
        K::Morning => {
            if zh {
                "清晨"
            } else {
                "Morning"
            }
        }
        K::Daytime => {
            if zh {
                "白天"
            } else {
                "Day"
            }
        }
        K::Evening => {
            if zh {
                "傍晚"
            } else {
                "Evening"
            }
        }
        K::Night => {
            if zh {
                "深夜"
            } else {
                "Night"
            }
        }
        K::Peak => {
            if zh {
                "峰值"
            } else {
                "Peak"
            }
        }
        K::Detail => {
            if zh {
                "明细"
            } else {
                "Detail"
            }
        }
        K::ByModel => {
            if zh {
                "按模型"
            } else {
                "by model"
            }
        }
        K::NoData => {
            if zh {
                "无数据"
            } else {
                "no data"
            }
        }
        K::Loading => {
            if zh {
                "加载中…"
            } else {
                "Loading…"
            }
        }
    }
}

/// "23s 前" / "23s ago".
pub fn age(a: u64) -> String {
    match lang() {
        Lang::Zh => format!("{a}s 前"),
        Lang::En => format!("{a}s ago"),
    }
}

/// "1162 条" / "1162 calls".
pub fn msgs(n: u64) -> String {
    match lang() {
        Lang::Zh => format!("{n} 条"),
        Lang::En => format!("{n} calls"),
    }
}

/// 相对时间:"3h 前" / "3h"(分/时/天三档)。
pub fn rel(ms_ago: i64) -> String {
    let mins = (ms_ago / 60_000).max(0);
    let (n, u) = if mins < 60 {
        (mins, "m")
    } else if mins < 60 * 24 {
        (mins / 60, "h")
    } else {
        (mins / 60 / 24, "d")
    };
    match lang() {
        Lang::Zh => format!("{n}{u} 前"),
        Lang::En => format!("{n}{u}"),
    }
}

const FILE: &str = "lang";

pub fn load(home: &std::path::Path) {
    let path = home.join(".tui-token-monitor").join(FILE);
    if let Ok(name) = std::fs::read_to_string(path) {
        set(name.trim());
    }
}

pub fn save(home: &std::path::Path) {
    let dir = home.join(".tui-token-monitor");
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(
            dir.join(FILE),
            match lang() {
                Lang::Zh => "zh",
                Lang::En => "en",
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_by_idx(i: usize) {
        while lang() != (if i == 0 { Lang::Zh } else { Lang::En }) {
            cycle();
        }
    }

    // 语言是进程级全局状态:两个用例合并为一个,避免并行测试互相干扰。
    #[test]
    fn cycle_set_and_rel() {
        let start = lang();
        cycle();
        assert_ne!(lang(), start);
        cycle();
        assert_eq!(lang(), start);
        assert!(set("en"));
        assert_eq!(lang(), Lang::En);
        assert!(set("中文"));
        assert_eq!(lang(), Lang::Zh);
        assert!(!set("fr"));

        set_by_idx(1); // EN
        assert_eq!(rel(30_000), "0m");
        assert_eq!(rel(3 * 60 * 60_000), "3h");
        assert_eq!(rel(2 * 24 * 60 * 60_000), "2d");
        set_by_idx(0); // ZH
        assert_eq!(rel(3 * 60 * 60_000), "3h 前");
    }
}
