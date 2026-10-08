//! Windows 桌面 toast 通知 + 配额阈值状态机。
//!
//! - toast:PowerShell 调 Windows.UI.Notifications(零新依赖,后台线程发送);
//! - 状态机:每条 (卡, 额度) 记录上一轮等级(0 正常 / 1 ≥80% / 2 ≥100%),
//!   等级跃迁产生 Warn / Exhausted / Reset 事件,原状不产生。

use std::collections::HashMap;

use crate::model::{Card, ProviderId, QuotaSource, Snapshot};
use crate::pace;

#[derive(Default)]
pub struct AlertTracker {
    initialized: bool,
    quota: HashMap<(ProviderId, String), u8>,
    expiry: HashMap<ProviderId, u8>,
    pace: HashMap<(ProviderId, String), bool>,
}

#[derive(Default)]
pub struct AlertBatch {
    pub quota: Vec<QuotaAlert>,
    pub expiry: Vec<ExpiryAlert>,
    pub pace: Vec<PaceAlert>,
}

pub struct QuotaAlert {
    pub title: String,
    pub used_pct: f64,
    pub event: ToastEvent,
}

pub struct ExpiryAlert {
    pub provider: String,
    pub ends_at_ms: i64,
}

pub struct PaceAlert {
    pub title: String,
    pub delta_points: f64,
    pub exhaust_in_ms: i64,
}

impl AlertTracker {
    /// The first successful snapshot establishes a baseline; later snapshots emit transitions.
    pub fn update(&mut self, snapshot: &Snapshot, now_ms: i64) -> AlertBatch {
        let mut batch = AlertBatch::default();
        let mut quota = HashMap::new();
        let mut expiry = HashMap::new();
        let mut pace_risk = HashMap::new();
        for card in &snapshot.cards {
            let Card::Live(report) = card else { continue };
            let provider = report.provider;
            let name = report.provider.title().split(" ·").next().unwrap_or("Token");
            if let Some(ends_at_ms) = report.subscription_ends_at_ms {
                let level = expiry_level(Some(ends_at_ms), now_ms);
                let prior = self.expiry.get(&provider).copied().unwrap_or(0);
                if self.initialized && level > prior {
                    batch.expiry.push(ExpiryAlert {
                        provider: name.to_string(),
                        ends_at_ms,
                    });
                }
                expiry.insert(provider, level);
            }
            if report.source == QuotaSource::CpaLocal {
                continue;
            }
            for limit in &report.limits {
                let key = (provider, limit.label.clone());
                let title = format!("{name} · {}", limit.label);
                let level = level_for(limit.used_pct);
                let prior = self.quota.get(&key).copied().unwrap_or(0);
                if self.initialized
                    && let Some(event) = transition(prior, level)
                {
                    batch.quota.push(QuotaAlert {
                        title: title.clone(),
                        used_pct: limit.used_pct,
                        event,
                    });
                }
                quota.insert(key.clone(), level);

                let forecast = pace::forecast(limit, now_ms);
                let exhaust_in_ms = forecast.and_then(|value| value.exhaust_in_ms);
                let at_risk = exhaust_in_ms.is_some();
                if self.initialized
                    && self.pace.get(&key) == Some(&false)
                    && let (Some(value), Some(exhaust_in_ms)) = (forecast, exhaust_in_ms)
                {
                    batch.pace.push(PaceAlert {
                        title,
                        delta_points: value.delta_points,
                        exhaust_in_ms,
                    });
                }
                pace_risk.insert(key, at_risk);
            }
        }
        self.quota = quota;
        self.expiry = expiry;
        self.pace = pace_risk;
        self.initialized = true;
        batch
    }
}

/// 阈值等级:0 正常,1 ≥80%(警告),2 ≥100%(耗尽)。
pub fn level_for(pct: f64) -> u8 {
    if pct >= 100.0 {
        2
    } else if pct >= 80.0 {
        1
    } else {
        0
    }
}

#[derive(Clone, Copy)]
pub enum ToastEvent {
    Warn,
    Exhausted,
    Reset,
}

impl ToastEvent {
    /// 事件文案(双语)。
    pub fn message(&self) -> &'static str {
        self.message_for(crate::ui::i18n::lang())
    }

    pub fn message_for(&self, lang: crate::ui::i18n::Lang) -> &'static str {
        match self {
            ToastEvent::Warn => match lang {
                crate::ui::i18n::Lang::Zh => "达到 80%",
                crate::ui::i18n::Lang::En => "reached 80%",
            },
            ToastEvent::Exhausted => match lang {
                crate::ui::i18n::Lang::Zh => "已耗尽",
                crate::ui::i18n::Lang::En => "exhausted",
            },
            ToastEvent::Reset => match lang {
                crate::ui::i18n::Lang::Zh => "已重置",
                crate::ui::i18n::Lang::En => "reset",
            },
        }
    }
}

/// 等级跃迁 → 事件;无变化返回 None。
pub fn transition(prev: u8, now: u8) -> Option<ToastEvent> {
    if prev == now {
        return None;
    }
    match now {
        2 => Some(ToastEvent::Exhausted),
        1 => {
            if prev < 1 {
                Some(ToastEvent::Warn)
            } else {
                Some(ToastEvent::Reset) // 2 → 1:重置后回到警告区间
            }
        }
        _ => {
            if prev >= 1 {
                Some(ToastEvent::Reset)
            } else {
                None
            }
        }
    }
}

/// 订阅到期档位:0 正常 / 1 ≤7 天 / 2 ≤3 天 / 3 ≤1 天或已过期。None = 无订阅期。
pub fn expiry_level(ends_at_ms: Option<i64>, now_ms: i64) -> u8 {
    let Some(ends) = ends_at_ms else { return 0 };
    let left = ends - now_ms;
    if left <= 86_400_000 {
        3
    } else if left <= 3 * 86_400_000 {
        2
    } else if left <= 7 * 86_400_000 {
        1
    } else {
        0
    }
}

/// 到期 toast 标题(双语):「GOAT 订阅」/「GOAT subscription」。
pub fn expiry_title(provider: &str) -> String {
    expiry_title_for(provider, crate::ui::i18n::lang())
}

pub fn expiry_title_for(provider: &str, lang: crate::ui::i18n::Lang) -> String {
    match lang {
        crate::ui::i18n::Lang::Zh => format!("{provider} 订阅"),
        crate::ui::i18n::Lang::En => format!("{provider} subscription"),
    }
}

/// 到期 toast 正文(双语):「10-01 到期 · 剩 2 天」;剩 0 天「今日到期」。
pub fn expiry_message(ends_at_ms: i64, now_ms: i64) -> String {
    expiry_message_for(ends_at_ms, now_ms, crate::ui::i18n::lang())
}

pub fn expiry_message_for(ends_at_ms: i64, now_ms: i64, lang: crate::ui::i18n::Lang) -> String {
    let days_left = ((ends_at_ms - now_ms).max(0) + 86_399_999) / 86_400_000;
    let date = chrono::DateTime::from_timestamp_millis(ends_at_ms)
        .map(|dt| dt.with_timezone(&chrono::Local))
        .map(|dt| dt.format("%m-%d").to_string())
        .unwrap_or_default();
    match lang {
        crate::ui::i18n::Lang::Zh => {
            if days_left == 0 {
                format!("{date} 今日到期")
            } else {
                format!("{date} 到期 · 剩 {days_left} 天")
            }
        }
        crate::ui::i18n::Lang::En => {
            if days_left == 0 {
                format!("expires today ({date})")
            } else {
                format!("expires {date} · {days_left} days left")
            }
        }
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
        .replace('‘', "&#x2018;")
        .replace('’', "&#x2019;")
        .replace('‚', "&#x201a;")
        .replace('‛', "&#x201b;")
}

/// 发送 Windows toast(后台线程,不阻塞调用方;失败静默)。
/// 非 Windows 平台暂无系统 toast 通道,空实现(阈值状态机照常运行)。
#[cfg(windows)]
pub fn toast(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        use std::process::{Command, Stdio};
        // XML 文本先编码所有 PS 单引号;format 避免二次替换正文中的占位符。
        let script = format!(
            r#"
$ErrorActionPreference = 'SilentlyContinue'
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$x = New-Object Windows.Data.Xml.Dom.XmlDocument
$x.LoadXml('<toast><visual><binding template="ToastGeneric"><text>{title}</text><text>{body}</text></binding></visual></toast>')
$t = [Windows.UI.Notifications.ToastNotification]::new($x)
$n = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('QuotaKeeper')
$n.Show($t)
"#,
            title = xml_escape(&title),
            body = xml_escape(&body)
        );
        use std::os::windows::process::CommandExt;
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    });
}

#[cfg(not(windows))]
pub fn toast(_title: &str, _body: &str) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Limit, Report, Status};

    #[test]
    fn toast_text_cannot_end_a_powershell_literal_or_add_xml() {
        let escaped = xml_escape("O'Brien ‘quota’ <&> __BODY__");
        assert!(!escaped.contains(['\'', '‘', '’', '<', '>']));
        assert!(escaped.contains("&apos;"));
        assert!(escaped.contains("&lt;&amp;&gt;"));
        assert!(escaped.contains("__BODY__"));
    }

    fn snapshot(used_pct: f64, ends_at_ms: Option<i64>, source: QuotaSource) -> Snapshot {
        Snapshot {
            generated_at_ms: 0,
            cards: vec![Card::Live(Report {
                provider: ProviderId::Codex,
                source,
                plan_type: String::new(),
                limits: vec![Limit {
                    label: "5h".into(),
                    used_pct,
                    resets_in_ms: Some(2 * 3_600_000),
                    status: Status::Ok,
                }],
                subscription_ends_at_ms: ends_at_ms,
                balance: None,
            })],
        }
    }

    #[test]
    fn tracker_baselines_then_emits_quota_expiry_and_pace_transitions() {
        let now = 1_790_000_000_000;
        let mut tracker = AlertTracker::default();
        let first = tracker.update(
            &snapshot(20.0, Some(now + 8 * 86_400_000), QuotaSource::CodexAppServer),
            now,
        );
        assert!(first.quota.is_empty() && first.expiry.is_empty() && first.pace.is_empty());

        let next = tracker.update(
            &snapshot(85.0, Some(now + 7 * 86_400_000), QuotaSource::CodexAppServer),
            now,
        );
        assert!(matches!(next.quota[0].event, ToastEvent::Warn));
        assert_eq!(next.expiry[0].provider, "Codex");
        assert_eq!(next.pace.len(), 1);
        let repeated = tracker.update(
            &snapshot(85.0, Some(now + 7 * 86_400_000), QuotaSource::CodexAppServer),
            now,
        );
        assert!(repeated.quota.is_empty() && repeated.expiry.is_empty() && repeated.pace.is_empty());

        let reset = tracker.update(
            &snapshot(10.0, Some(now + 30 * 86_400_000), QuotaSource::CodexAppServer),
            now,
        );
        assert!(matches!(reset.quota[0].event, ToastEvent::Reset));
        assert!(reset.expiry.is_empty());
    }

    #[test]
    fn expiry_alerts_fire_once_per_tier_as_the_end_date_approaches() {
        let ends = 1_790_000_000_000 + 8 * 86_400_000;
        let at = |days_left: i64| ends - days_left * 86_400_000;
        let observe = |tracker: &mut AlertTracker, days_left: i64| {
            tracker
                .update(&snapshot(20.0, Some(ends), QuotaSource::CodexAppServer), at(days_left))
                .expiry
                .len()
        };
        let mut tracker = AlertTracker::default();
        assert_eq!(observe(&mut tracker, 8), 0);
        assert_eq!(observe(&mut tracker, 7), 1);
        assert_eq!(observe(&mut tracker, 6), 0);
        assert_eq!(observe(&mut tracker, 3), 1);
        assert_eq!(observe(&mut tracker, 2), 0);
        assert_eq!(observe(&mut tracker, 1), 1);
        assert_eq!(observe(&mut tracker, 0), 0);
    }

    #[test]
    fn tracker_ignores_cpa_activity_limits() {
        let mut tracker = AlertTracker::default();
        tracker.update(&snapshot(10.0, None, QuotaSource::CpaLocal), 1_790_000_000_000);
        let alerts = tracker.update(
            &snapshot(100.0, None, QuotaSource::CpaLocal),
            1_790_000_000_000,
        );
        assert!(alerts.quota.is_empty() && alerts.pace.is_empty());
    }

    #[test]
    fn levels_follow_thresholds() {
        assert_eq!(level_for(79.9), 0);
        assert_eq!(level_for(80.0), 1);
        assert_eq!(level_for(99.9), 1);
        assert_eq!(level_for(100.0), 2);
        assert_eq!(level_for(140.0), 2);
    }

    #[test]
    fn transitions_cover_threshold_crossings() {
        assert!(matches!(transition(0, 1), Some(ToastEvent::Warn)));
        assert!(matches!(transition(1, 2), Some(ToastEvent::Exhausted)));
        assert!(matches!(transition(0, 2), Some(ToastEvent::Exhausted)));
        assert!(matches!(transition(2, 0), Some(ToastEvent::Reset)));
        assert!(matches!(transition(2, 1), Some(ToastEvent::Reset)));
        assert!(transition(1, 1).is_none());
        assert!(transition(0, 0).is_none());
        assert!(transition(0, 2).is_some());
    }

    #[test]
    fn expiry_level_follows_day_thresholds() {
        let now = 0i64;
        assert_eq!(expiry_level(None, now), 0);
        assert_eq!(expiry_level(Some(8 * 86_400_000), now), 0);
        assert_eq!(expiry_level(Some(7 * 86_400_000), now), 1);
        assert_eq!(expiry_level(Some(4 * 86_400_000), now), 1);
        assert_eq!(expiry_level(Some(3 * 86_400_000), now), 2);
        assert_eq!(expiry_level(Some(86_400_000), now), 3);
        assert_eq!(expiry_level(Some(0), now), 3);
        // 已过期(负余量)也是最高档
        assert_eq!(expiry_level(Some(-86_400_000), now), 3);
    }

    #[test]
    fn expiry_message_counts_days_up() {
        let now = 0i64;
        let zh_day = expiry_message(2 * 86_400_000, now);
        assert!(zh_day.contains("剩 2 天"), "{zh_day}");
        assert!(expiry_message(0, now).contains("今日到期"));
    }
}
