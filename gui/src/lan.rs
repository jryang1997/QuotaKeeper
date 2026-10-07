//! 局域网 Standby 看板:GUI 进程内起只读 HTTP 服务,手机/平板浏览器访问。
//!
//! 仅两个 GET 端点:`/` 返回内嵌单文件页面,`/api/summary` 返回摘要 JSON。
//! 不暴露任何密钥/凭证;监听 0.0.0.0(手机必须),开关默认关。
//! 手写 TcpListener(零新依赖):解析请求行 → 路由 → 写响应,请求体直接丢弃。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::Serialize;

use quotakeeper::model::{Card, QuotaSource, Snapshot};
use quotakeeper::ui::notify::level_for;

/// 主循环每轮数据落定时写入;HTTP 线程只读。
#[derive(Default)]
pub struct LanState {
    pub snapshot: Option<Snapshot>,
    pub ledger: Option<Arc<quotakeeper::ledger::LedgerReport>>,
    pub goat_tokens: Option<u64>,
    pub goat_requests: Option<u64>,
    pub monthly_api_value_usd: Option<f64>,
    pub monthly_multiple: Option<f64>,
    pub generated_at_ms: i64,
}

pub static STATE: RwLock<LanState> = RwLock::new(LanState {
    snapshot: None,
    ledger: None,
    goat_tokens: None,
    goat_requests: None,
    monthly_api_value_usd: None,
    monthly_multiple: None,
    generated_at_ms: 0,
});

static REQUESTS_SERVED: AtomicU64 = AtomicU64::new(0);

/// 主循环调用:整体替换共享状态(零拷贝,只克隆 Arc)。
pub fn publish(
    snapshot: &Snapshot,
    ledger: Option<&Arc<quotakeeper::ledger::LedgerReport>>,
    goat_usage: Option<&quotakeeper::goat::UsageSummary>,
    monthly_api_value_usd: Option<f64>,
    monthly_multiple: Option<f64>,
) {
    let mut state = STATE.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    state.snapshot = Some(snapshot.clone());
    state.ledger = ledger.cloned();
    state.goat_tokens = goat_usage.map(|usage| usage.total_tokens);
    state.goat_requests = goat_usage.map(|usage| usage.total_count as u64);
    state.monthly_api_value_usd = monthly_api_value_usd;
    state.monthly_multiple = monthly_multiple;
    state.generated_at_ms = snapshot.generated_at_ms;
}

pub const PORT_BASE: u16 = 8420;
const PORT_MAX_OFFSET: u16 = 9;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanRing {
    pub title: String,
    pub plan_type: String,
    pub label: &'static str,
    pub used_pct: f64,
    pub resets_in_ms: Option<i64>,
    pub level: u8,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct LanPeriod {
    pub tokens: i64,
    pub messages: u64,
    pub cost: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanSummary {
    pub ok: bool,
    pub generated_at_ms: i64,
    /// 24 小时桶(本地时),今日视图的时段柱
    pub hourly: Vec<i64>,
    pub today: LanPeriod,
    /// 近 7 天(today-6 → today),与主窗用量页「近 7 天」同口径
    pub d7: LanPeriod,
    /// 近 30 天(today-29 → today),与主窗用量页「近 30 天」同口径
    pub d30: LanPeriod,
    pub month: LanPeriod,
    pub burn_rate: Option<f64>,
    pub monthly_api_value_usd: Option<f64>,
    pub monthly_multiple: Option<f64>,
    pub rings: Vec<LanRing>,
}

fn period_of(agg: &quotakeeper::ledger::Agg) -> LanPeriod {
    LanPeriod {
        tokens: agg.total(),
        messages: agg.messages,
        cost: agg.cost(),
    }
}

/// 末尾 N 天聚合(daily_totals 旧→新、空日也保留,故 rev().take(N) 即「含今天的近 N 天」)。
fn period_of_last_days(days: &[quotakeeper::ledger::DailyTotal], count: usize) -> LanPeriod {
    let mut agg = quotakeeper::ledger::Agg::default();
    for day in days.iter().rev().take(count) {
        agg.merge(&day.agg);
    }
    period_of(&agg)
}

pub fn summary() -> LanSummary {
    let state = STATE.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    let zero = LanPeriod { tokens: 0, messages: 0, cost: 0.0 };
    let (hourly, today, d7, d30, month) = if let Some(ledger) = state.ledger.as_ref() {
        (
            ledger.hourly.to_vec(),
            period_of(&ledger.today),
            period_of_last_days(&ledger.daily_totals, 7),
            period_of_last_days(&ledger.daily_totals, 30),
            period_of(&ledger.month),
        )
    } else {
        (
            Vec::new(),
            zero,
            zero,
            zero,
            zero,
        )
    };
    let mut rings = Vec::new();
    if let Some(snapshot) = state.snapshot.as_ref() {
        for card in &snapshot.cards {
            if let Card::Live(report) = card {
                if report.source == QuotaSource::CpaLocal {
                    continue; // CPA 本地活动卡无周期环,同托盘/悬浮窗口径
                }
                for limit in &report.limits {
                    rings.push(LanRing {
                        title: report.provider.title().to_string(),
                        plan_type: report.plan_type.clone(),
                        label: limit_label(&limit.label),
                        used_pct: limit.used_pct,
                        resets_in_ms: limit.resets_in_ms,
                        level: level_for(limit.used_pct),
                    });
                }
            }
        }
    }
    LanSummary {
        ok: true,
        generated_at_ms: state.generated_at_ms,
        hourly,
        today,
        d7,
        d30,
        month,
        burn_rate: state.ledger.as_ref().and_then(|ledger| ledger.burn_rate),
        monthly_api_value_usd: state.monthly_api_value_usd,
        monthly_multiple: state.monthly_multiple,
        rings,
    }
}

/// 环标签归类成 5H/WEEK/MONTH/DAY(与悬浮窗同口径;认不出保留原文)。
fn limit_label(raw: &str) -> &'static str {
    let label = raw.to_lowercase();
    if label.contains("5h") || label.contains("5 hour") || label.contains("5 小时") {
        "5H"
    } else if (label.contains("7") && label.contains("day")) || label.contains("week") {
        "WEEK"
    } else if label.contains("month") {
        "MONTH"
    } else {
        "DAY"
    }
}

/// 默认路由对应的本机局域网 IP(UDP connect 只查路由表,不发包)。
pub fn lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("10.255.255.255:1").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

pub struct LanServer {
    pub port: u16,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl LanServer {
    /// 起服务;端口被占顺延重试。失败返回 None。
    pub fn start() -> Option<LanServer> {
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        for offset in 0..=PORT_MAX_OFFSET {
            let port = PORT_BASE + offset;
            let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) else {
                continue;
            };
            let flag = Arc::clone(&shutdown);
            let handle = std::thread::Builder::new()
                .name("lan-dashboard".into())
                .spawn(move || serve(listener, flag))
                .ok()?;
            return Some(LanServer {
                port,
                shutdown,
                handle: Some(handle),
            });
        }
        None
    }

    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // 唤醒 accept:连一个本地 socket 让它返回一轮
        if let Ok(mut socket) = TcpStream::connect(("127.0.0.1", self.port)) {
            let _ = socket.write_all(b"quit\n");
            drop(socket);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for LanServer {
    fn drop(&mut self) {
        if !self.shutdown.load(Ordering::SeqCst) {
            self.stop();
        }
    }
}

fn serve(listener: TcpListener, shutdown: Arc<std::sync::atomic::AtomicBool>) {
    listener.set_nonblocking(true).ok();
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                std::thread::spawn(move || handle_connection(stream));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(300)),
        }
    }
}

fn handle_connection(stream: TcpStream) {
    let mut reader = BufReader::new(
        match stream.try_clone() {
            Ok(clone) => clone,
            Err(_) => return,
        },
    );
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    // 丢弃请求头与请求体(我们只处理无体 GET)
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Some(value) = line
                    .strip_prefix("Content-Length:")
                    .or_else(|| line.strip_prefix("content-length:"))
                    .and_then(|rest| rest.trim().parse::<usize>().ok())
                {
                    content_length = value;
                }
                if line == "\r\n" || line == "\n" {
                    break;
                }
            }
            Err(_) => return,
        }
    }
    if content_length > 0 {
        let mut sink = vec![0u8; content_length.min(64 * 1024)];
        let _ = reader.read_exact(&mut sink);
    }
    REQUESTS_SERVED.fetch_add(1, Ordering::Relaxed);
    let mut parts = request_line.split_whitespace();
    let _method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    let path = path.split('?').next().unwrap_or("/");
    let path = percent_decode(path);
    let (status, content_type, body) = route(&path);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut stream = stream;
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

fn route(path: &str) -> (&'static str, &'static str, Vec<u8>) {
    match path {
        "/" => ("200 OK", "text/html; charset=utf-8", standby_html().as_bytes().to_vec()),
        "/api/summary" => match serde_json::to_vec(&summary()) {
            Ok(body) => ("200 OK", "application/json", body),
            Err(_) => ("500 Internal Server Error", "application/json", b"{\"ok\":false}".to_vec()),
        },
        "/healthz" => ("200 OK", "text/plain; charset=utf-8", b"ok".to_vec()),
        _ => ("404 Not Found", "text/plain; charset=utf-8", b"not found".to_vec()),
    }
}

fn percent_decode(path: &str) -> String {
    if !path.contains('%') {
        return path.to_string();
    }
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(byte) = hex_pair(bytes.get(index + 1), bytes.get(index + 2))
        {
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_pair(high: Option<&u8>, low: Option<&u8>) -> Option<u8> {
    let hex = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    };
    let high = hex(*high?);
    let low = hex(*low?)?;
    Some(high? * 16 + low)
}

/// 内嵌手机页;构建期编入,改前端须重编 GUI。
fn standby_html() -> &'static str {
    include_str!("standby.html")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// STATE 是进程级单例:涉及 publish 的测试与空状态断言互斥执行。
    static STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn limit_labels_map_to_window_slots() {
        assert_eq!(limit_label("5h used"), "5H");
        assert_eq!(limit_label("5 HOUR LIMIT"), "5H");
        assert_eq!(limit_label("7 days"), "WEEK");
        assert_eq!(limit_label("Weekly limit"), "WEEK");
        assert_eq!(limit_label("Monthly limit"), "MONTH");
        assert_eq!(limit_label("Today"), "DAY");
    }

    #[test]
    fn percent_decode_handles_escapes() {
        assert_eq!(percent_decode("/api/a%20b"), "/api/a b");
        assert_eq!(percent_decode("/plain"), "/plain");
        assert_eq!(percent_decode("/bad%2"), "/bad%2");
    }

    #[test]
    fn summary_is_safe_without_state() {
        let _guard = STATE_LOCK.lock().unwrap();
        *STATE.write().unwrap() = LanState::default();
        let summary = summary();
        assert!(summary.ok);
        assert!(summary.rings.is_empty());
        assert_eq!(summary.today.tokens, 0);
        assert_eq!(summary.d7.tokens, 0);
        assert_eq!(summary.d30.tokens, 0);
        assert!(summary.hourly.is_empty());
    }

    #[test]
    fn period_of_last_days_sums_recent_window() {
        use quotakeeper::ledger::{Agg, DailyTotal};
        let days: Vec<DailyTotal> = (0..10)
            .map(|i| DailyTotal {
                date: format!("2026-10-{i:02}"),
                tokens: (i + 1) * 100,
                cost: (i + 1) as f64,
                messages: 1,
                agg: Agg { input: (i + 1) * 100, messages: 1, cost_reported: (i + 1) as f64, ..Agg::default() },
            })
            .collect();
        // 末尾 7 天:400..1000 → 4900
        assert_eq!(period_of_last_days(&days, 7).tokens, 4_900);
        assert_eq!(period_of_last_days(&days, 7).messages, 7);
        assert_eq!(period_of_last_days(&days, 7).cost, 49.0);
        // 30 天窗口超出范围 = 全部
        assert_eq!(period_of_last_days(&days, 30).tokens, 5_500);
    }

    #[test]
    fn lan_ip_resolves_something_or_none() {
        // 不强制有网环境;只保证不 panic
        let _ = lan_ip();
    }

    #[test]
    fn server_starts_and_stops() {
        let mut server = LanServer::start().expect("server should bind");
        assert!((PORT_BASE..=PORT_BASE + PORT_MAX_OFFSET).contains(&server.port));
        let body = request_blocking(server.port, "/healthz");
        assert_eq!(body, b"ok");
        server.stop();
    }

    fn request_blocking(port: u16, path: &str) -> Vec<u8> {
        for _ in 0..40 {
            if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
                let _ = stream.write_all(format!("GET {path} HTTP/1.1\r\nHost: t\r\n\r\n").as_bytes());
                let mut buffer = Vec::new();
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.read_to_end(&mut buffer);
                if let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                    return buffer[header_end + 4..].to_vec();
                }
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        panic!("no response on {path}");
    }

    #[test]
    fn publish_feeds_summary() {
        let _guard = STATE_LOCK.lock().unwrap();
        use quotakeeper::model::ProviderId;
        let snapshot = Snapshot {
            generated_at_ms: 123,
            cards: vec![Card::Live(quotakeeper::model::Report {
                provider: ProviderId::Codex,
                source: QuotaSource::CodexAppServer,
                plan_type: "plus".into(),
                limits: vec![quotakeeper::model::Limit {
                    label: "5h".into(),
                    used_pct: 42.0,
                    resets_in_ms: Some(3_600_000),
                    status: quotakeeper::model::Status::Ok,
                }],
                subscription_ends_at_ms: None,
            })],
        };
        publish(&snapshot, None, None, Some(108.0), Some(2.7));
        let summary = summary();
        assert_eq!(summary.rings.len(), 1);
        assert_eq!(summary.rings[0].label, "5H");
        assert_eq!(summary.rings[0].used_pct, 42.0);
        assert_eq!(summary.monthly_multiple, Some(2.7));
        assert_eq!(summary.generated_at_ms, 123);
    }
}
