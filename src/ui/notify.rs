//! Windows 桌面 toast 通知 + 配额阈值状态机。
//!
//! - toast:PowerShell 调 Windows.UI.Notifications(零新依赖,后台线程发送);
//! - 状态机:每条 (卡, 额度) 记录上一轮等级(0 正常 / 1 ≥80% / 2 ≥100%),
//!   等级跃迁产生 Warn / Exhausted / Reset 事件,原状不产生。

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

pub enum ToastEvent {
    Warn,
    Exhausted,
    Reset,
}

impl ToastEvent {
    /// 事件文案(双语)。
    pub fn message(&self) -> &'static str {
        match self {
            ToastEvent::Warn => match crate::ui::i18n::lang() {
                crate::ui::i18n::Lang::Zh => "达到 80%",
                crate::ui::i18n::Lang::En => "reached 80%",
            },
            ToastEvent::Exhausted => match crate::ui::i18n::lang() {
                crate::ui::i18n::Lang::Zh => "已耗尽",
                crate::ui::i18n::Lang::En => "exhausted",
            },
            ToastEvent::Reset => match crate::ui::i18n::lang() {
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

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 发送 Windows toast(后台线程,不阻塞调用方;失败静默)。
/// 非 Windows 平台暂无系统 toast 通道,空实现(阈值状态机照常运行)。
#[cfg(windows)]
pub fn toast(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        use std::process::{Command, Stdio};
        // PS 单引号 here-string 原样保留,占位符由 Rust 侧替换
        let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$x = New-Object Windows.Data.Xml.Dom.XmlDocument
$x.LoadXml('<toast><visual><binding template="ToastGeneric"><text>__TITLE__</text><text>__BODY__</text></binding></visual></toast>')
$t = [Windows.UI.Notifications.ToastNotification]::new($x)
$n = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Token Monitor')
$n.Show($t)
"#
        .replace("__TITLE__", &xml_escape(&title))
        .replace("__BODY__", &xml_escape(&body));
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
}
