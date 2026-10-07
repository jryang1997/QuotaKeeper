//! Agent 扫描/更新的线程调度:重活在后台线程跑,结果经 UserEvent 回主线程
//! 再 evaluate_script 推给前端。SCANNING/UPDATING 两把原子闸防重复触发。

use std::sync::atomic::{AtomicBool, Ordering};

use tao::event_loop::EventLoopProxy;

use crate::UserEvent;

static SCANNING: AtomicBool = AtomicBool::new(false);
static UPDATING: AtomicBool = AtomicBool::new(false);

fn payload_of(scans: &[quotakeeper::agents_scan::AgentScan]) -> String {
    serde_json::to_string(scans).unwrap_or_else(|_| "[]".into())
}

pub(crate) fn scan(proxy: EventLoopProxy<UserEvent>) {
    if SCANNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let payload = payload_of(&quotakeeper::agents_scan::scan_all());
        SCANNING.store(false, Ordering::SeqCst);
        let _ = proxy.send_event(UserEvent::AgentsScanned(payload));
    });
}

/// 升级单个工具(id)或全部("all")。命令后端现场重新生成,不信任前端回传;
/// 完成后附一份全量复扫结果,前端以复扫到的版本为准。
pub(crate) fn update(proxy: EventLoopProxy<UserEvent>, tool_id: String) {
    if UPDATING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let specs = quotakeeper::agents_scan::specs();
        let targets: Vec<&_> = match tool_id.as_str() {
            "all" => specs.iter().collect(),
            id => specs.iter().filter(|spec| spec.id == id).collect(),
        };
        let mut results = Vec::new();
        for spec in targets {
            let scan = quotakeeper::agents_scan::scan_one(spec);
            let command = scan
                .default_install()
                .and_then(|install| install.update_command.clone());
            let Some(command) = command else {
                results.push((spec.id.to_string(), false, "no-update-command".to_string()));
                continue;
            };
            let (ok, message) = match quotakeeper::agents_scan::run_update(&command) {
                Ok(tail) => (true, tail),
                Err(message) => (false, message),
            };
            results.push((spec.id.to_string(), ok, message));
            if !ok && tool_id != "all" {
                break;
            }
        }
        let payload = payload_of(&quotakeeper::agents_scan::scan_all());
        UPDATING.store(false, Ordering::SeqCst);
        let _ = proxy.send_event(UserEvent::AgentUpdateFinished {
            results,
            payload,
        });
    });
}
