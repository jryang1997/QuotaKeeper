//! 刷新引擎:GUI 侧后台采集,复用主 crate 的 backend/goat/ledger。
//!
//! 与 TUI main.rs 的 spawn_refresh 同构:一次带回 quota(可失败)与
//! ledger(单源错误记录在案),经 mpsc 回传,主线程经 IPC 推给 WebView。

use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui_token_monitor::{backend, goat, ledger, model};

use model::Snapshot;

pub struct Refreshed {
    pub fetched_at_ms: u64,
    pub quota: Result<Snapshot, String>,
    pub ledger: Box<ledger::LedgerReport>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 采集一次。放在独立线程池线程执行,避免占用事件循环。
pub fn spawn(tx: mpsc::Sender<Refreshed>) {
    std::thread::spawn(move || {
        let t = std::thread::spawn(backend::fetch_snapshot);
        let goat_rep = goat::fetch_report().ok();
        let quota = match t
            .join()
            .unwrap_or_else(|_| Err(anyhow::anyhow!("OMP 采集线程 panic")))
        {
            Ok(mut snap) => {
                if let Some(r) = goat_rep
                    && let Some(slot) = snap.cards.iter_mut().find(|c| {
                        matches!(
                            c,
                            model::Card::Missing {
                                provider: model::ProviderId::Goat
                            }
                        )
                    })
                {
                    *slot = model::Card::Live(r);
                }
                Ok(snap)
            }
            Err(e) => Err(e.to_string()),
        };
        let rep = ledger::collect(chrono::Local::now());
        let _ = tx.send(Refreshed {
            fetched_at_ms: now_ms(),
            quota,
            ledger: Box::new(rep),
        });
    });
}

/// 刷新节拍:60s 周期,由主循环按 next_refresh 驱动(GUI 侧沿用 TUI 语义)。
pub const PERIOD: Duration = Duration::from_secs(60);

pub fn is_due(next: Instant) -> bool {
    Instant::now() >= next
}
