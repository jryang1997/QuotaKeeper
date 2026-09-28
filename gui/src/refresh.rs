//! 刷新引擎:GUI 侧后台采集,复用主 crate 的 backend/goat/ledger。
//!
//! 配额与用量分开采集,经 mpsc 回传,主线程合并最新快照后推给 WebView。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui_token_monitor::{backend, goat, ledger, model};

use model::Snapshot;

pub struct Refreshed {
    pub fetched_at_ms: u64,
    pub quota: Option<Result<Snapshot, String>>,
    /// 配额轮询时的 GOAT 官方账期用量;None = 本次未采集(账本轮询).
    /// 内层 Some/None = 官方汇总成功/失败(失败时前端回退本机口径).
    pub goat_usage: Option<Option<goat::UsageSummary>>,
    pub ledger: Option<Box<ledger::LedgerReport>>,
}

static QUOTA_RUNNING: AtomicBool = AtomicBool::new(false);
static QUOTA_RERUN: AtomicBool = AtomicBool::new(false);
static LEDGER_RUNNING: AtomicBool = AtomicBool::new(false);

struct RefreshGuard(&'static AtomicBool);

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn acquire(flag: &'static AtomicBool) -> Option<RefreshGuard> {
    flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| RefreshGuard(flag))
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 刷新按钮和首次启动时,并行刷新用量与配额。
pub fn spawn(tx: mpsc::Sender<Refreshed>) {
    spawn_quota(tx.clone());
    spawn_ledger(tx);
}

/// 配额服务请求较慢,单独限频,避免跟随用量轮询频繁访问。
pub fn spawn_quota(tx: mpsc::Sender<Refreshed>) {
    let Some(guard) = acquire(&QUOTA_RUNNING) else {
        QUOTA_RERUN.store(true, Ordering::Release);
        return;
    };
    std::thread::spawn(move || {
        loop {
            let quota = backend::fetch_snapshot().map_err(|e| e.to_string());
            // 无 key 时本地秒失败,不发网络请求;失败置 None 由前端回退本机口径.
            let goat_usage = Some(goat::fetch_usage_summary().ok());
            let _ = tx.send(Refreshed {
                fetched_at_ms: now_ms(),
                quota: Some(quota),
                goat_usage,
                ledger: None,
            });
            if !QUOTA_RERUN.swap(false, Ordering::AcqRel) {
                break;
            }
        }
        drop(guard);
        if QUOTA_RERUN.swap(false, Ordering::AcqRel) {
            spawn_quota(tx);
        }
    });
}

/// 本地用量数据支持快速轮询,独立于配额请求运行。
pub fn spawn_ledger(tx: mpsc::Sender<Refreshed>) {
    let Some(guard) = acquire(&LEDGER_RUNNING) else {
        return;
    };
    std::thread::spawn(move || {
        let _guard = guard;
        let rep = ledger::collect(chrono::Local::now());
        let _ = tx.send(Refreshed {
            fetched_at_ms: now_ms(),
            quota: None,
            goat_usage: None,
            ledger: Some(Box::new(rep)),
        });
    });
}

/// 本地文件与 SQLite 轮询节拍。
pub const LEDGER_PERIOD: Duration = Duration::from_secs(5);
/// 配额刷新节拍,避免高频请求远端服务。
pub const QUOTA_PERIOD: Duration = Duration::from_secs(60);

pub fn is_due(next: Instant) -> bool {
    Instant::now() >= next
}
