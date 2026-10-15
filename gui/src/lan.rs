//! 局域网手机页只看各窗口还剩多少。默认关,监听 0.0.0.0,不暴露密钥。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::Serialize;

use quotakeeper::model::{status_for, Card, QuotaSource, Snapshot, Status};

#[derive(Default)]
struct LanState {
    snapshot: Option<Snapshot>,
}

static STATE: RwLock<LanState> = RwLock::new(LanState { snapshot: None });

static REQUESTS_SERVED: AtomicU64 = AtomicU64::new(0);

pub fn publish(snapshot: &Snapshot) {
    let snapshot = snapshot.clone();
    let mut state = STATE.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    state.snapshot = Some(snapshot);
}

pub const PORT_BASE: u16 = 8420;
const PORT_MAX_OFFSET: u16 = 9;

/// `stale` 用电脑上的时钟,不用手机时钟。
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
enum Glance {
    Waiting,
    Ready { stale: bool, cards: Vec<GlanceCard> },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct GlanceCard {
    title: String,
    plan: String,
    windows: Vec<RemainingWindow>,
}

/// 色调跟着已用百分比,不跟着剩余。非正的重置时长不出现。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemainingWindow {
    label: WindowLabel,
    remaining_pct: u8,
    level: WindowLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    resets_in_ms: Option<ResetInMs>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum WindowLabel {
    #[serde(rename = "5H")]
    FiveHour,
    #[serde(rename = "WEEK")]
    Week,
    #[serde(rename = "MONTH")]
    Month,
    #[serde(rename = "DAY")]
    Day,
}

impl WindowLabel {
    fn rank(self) -> u8 {
        match self {
            Self::FiveHour => 0,
            Self::Day => 1,
            Self::Week => 2,
            Self::Month => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum WindowLevel {
    Ok,
    Warn,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
struct ResetInMs(u64);

const MANIFEST: &str = r##"{"name":"QuotaKeeper","short_name":"剩余","start_url":"/","display":"standalone","background_color":"#000000","theme_color":"#000000"}"##;

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn glance(snapshot: &Snapshot, now_ms: i64) -> Glance {
    let mut cards = Vec::new();
    for card in &snapshot.cards {
        let Card::Live(report) = card else {
            continue;
        };
        if report.source == QuotaSource::CpaLocal {
            continue;
        }
        let mut keyed = Vec::new();
        for (index, limit) in report.limits.iter().enumerate() {
            if !limit.used_pct.is_finite() {
                continue;
            }
            let used = limit.used_pct.clamp(0.0, 100.0);
            let remaining = (100.0 - used).round().clamp(0.0, 100.0) as u8;
            let level = match status_for(used) {
                Status::Ok => WindowLevel::Ok,
                Status::Warning => WindowLevel::Warn,
                Status::Exhausted => WindowLevel::Out,
            };
            let resets_in_ms = match limit.resets_in_ms {
                Some(ms) if ms > 0 => Some(ResetInMs(ms as u64)),
                _ => None,
            };
            keyed.push((
                index,
                RemainingWindow {
                    label: limit_label(&limit.label),
                    remaining_pct: remaining,
                    level,
                    resets_in_ms,
                },
            ));
        }
        keyed.sort_by(|a, b| {
            a.1.remaining_pct
                .cmp(&b.1.remaining_pct)
                .then(a.1.label.rank().cmp(&b.1.label.rank()))
                .then(a.0.cmp(&b.0))
        });
        let windows: Vec<RemainingWindow> = keyed.into_iter().map(|(_, window)| window).collect();
        if windows.is_empty() {
            continue;
        }
        cards.push(GlanceCard {
            title: report.provider.title().to_string(),
            plan: report.plan_type.clone(),
            windows,
        });
    }
    Glance::Ready {
        stale: now_ms.saturating_sub(snapshot.generated_at_ms) > 120_000,
        cards,
    }
}

fn summary() -> Glance {
    let snapshot = {
        let state = STATE.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.snapshot.clone()
    };
    match snapshot.as_ref() {
        Some(snapshot) => glance(snapshot, now_ms()),
        None => Glance::Waiting,
    }
}

fn limit_label(raw: &str) -> WindowLabel {
    let label = raw.to_lowercase();
    if label.contains("5h") || label.contains("5 hour") || label.contains("5 小时") {
        WindowLabel::FiveHour
    } else if (label.contains("7") && label.contains("day")) || label.contains("week") {
        WindowLabel::Week
    } else if label.contains("month") {
        WindowLabel::Month
    } else {
        WindowLabel::Day
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
        "/manifest.webmanifest" => (
            "200 OK",
            "application/manifest+json",
            MANIFEST.as_bytes().to_vec(),
        ),
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
    use quotakeeper::model::{Balance, Card, Limit, ProviderId, QuotaSource, Report, Snapshot, Status};

    /// STATE 是进程级单例:涉及 publish 的测试与空状态断言互斥执行。
    static STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn limit_labels_map_to_window_slots() {
        assert_eq!(limit_label("5h used"), WindowLabel::FiveHour);
        assert_eq!(limit_label("5 HOUR LIMIT"), WindowLabel::FiveHour);
        assert_eq!(limit_label("7 days"), WindowLabel::Week);
        assert_eq!(limit_label("7 days Opus"), WindowLabel::Week);
        assert_eq!(limit_label("Weekly limit"), WindowLabel::Week);
        assert_eq!(limit_label("Monthly limit"), WindowLabel::Month);
        assert_eq!(limit_label("Today"), WindowLabel::Day);
        assert_eq!(limit_label("nope"), WindowLabel::Day);
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
        assert_eq!(summary(), Glance::Waiting);
        assert_eq!(serde_json::to_string(&summary()).unwrap(), r#"{"state":"waiting"}"#);
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
        let snapshot = Snapshot {
            generated_at_ms: 123,
            cards: vec![Card::Live(Report {
                provider: ProviderId::Codex,
                source: QuotaSource::CodexAppServer,
                plan_type: "plus".into(),
                limits: vec![Limit {
                    label: "5h".into(),
                    used_pct: 42.0,
                    resets_in_ms: Some(3_600_000),
                    status: Status::Ok,
                }],
                subscription_ends_at_ms: None,
                balance: None,
            })],
        };
        publish(&snapshot);
        let Glance::Ready { stale, cards } = summary() else {
            panic!("expected ready");
        };
        assert_eq!(stale, now_ms().saturating_sub(123) > 120_000);
        let Glance::Ready { stale: false, cards: near_cards } = glance(&snapshot, 123) else {
            panic!("near now should not be stale");
        };
        assert_eq!(cards, near_cards);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title, "Codex");
        assert_eq!(cards[0].plan, "plus");
        assert_eq!(
            cards[0].windows,
            vec![RemainingWindow {
                label: WindowLabel::FiveHour,
                remaining_pct: 58,
                level: WindowLevel::Ok,
                resets_in_ms: Some(ResetInMs(3_600_000)),
            }]
        );
        let wire = serde_json::to_string(&summary()).unwrap();
        assert!(!wire.contains("generatedAt"));
        assert!(!wire.contains("monthly"));
        let replacement = Snapshot {
            generated_at_ms: 123,
            cards: vec![Card::Live(Report {
                provider: ProviderId::MiniMax,
                source: QuotaSource::MinimaxRemainsApi,
                plan_type: String::new(),
                limits: vec![Limit {
                    label: "7 days".into(),
                    used_pct: 20.0,
                    resets_in_ms: Some(86_400_000),
                    status: Status::Ok,
                }],
                subscription_ends_at_ms: None,
                balance: None,
            })],
        };
        publish(&replacement);
        let Glance::Ready { cards, .. } = summary() else {
            panic!("expected ready");
        };
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title, "MiniMax Coding");
        assert_eq!(cards[0].windows[0].remaining_pct, 80);
    }

    #[test]
    fn glance_keeps_remaining_sort_omissions_and_passed_resets() {
        let generated_at_ms = 1_700_000_000_000;
        let snapshot = Snapshot {
            generated_at_ms,
            cards: vec![
                Card::Live(Report {
                    provider: ProviderId::Goat,
                    source: QuotaSource::CommandCodeApi,
                    plan_type: "pro".into(),
                    limits: vec![
                        Limit {
                            label: "5 hours".into(),
                            used_pct: 42.0,
                            resets_in_ms: Some(3_600_000),
                            status: Status::Ok,
                        },
                        Limit {
                            label: "7 days Opus".into(),
                            used_pct: 91.0,
                            resets_in_ms: Some(-5_000),
                            status: Status::Warning,
                        },
                        Limit {
                            label: "nope".into(),
                            used_pct: f64::NAN,
                            resets_in_ms: Some(10),
                            status: Status::Ok,
                        },
                    ],
                    subscription_ends_at_ms: Some(9_999_999),
                    balance: Some(Balance { amount: 12.0, currency: "USD".into() }),
                }),
                Card::Missing { provider: ProviderId::Claude },
                Card::Live(Report {
                    provider: ProviderId::Antigravity,
                    source: QuotaSource::CpaLocal,
                    plan_type: "CPA 凭证可用".into(),
                    limits: vec![Limit {
                        label: "Today vs 7-day activity".into(),
                        used_pct: 10.0,
                        resets_in_ms: Some(50_000),
                        status: Status::Ok,
                    }],
                    subscription_ends_at_ms: None,
                    balance: None,
                }),
                Card::Live(Report {
                    provider: ProviderId::Codex,
                    source: QuotaSource::CodexAppServer,
                    plan_type: "plus".into(),
                    limits: vec![
                        Limit {
                            label: "Monthly".into(),
                            used_pct: 100.0,
                            resets_in_ms: Some(0),
                            status: Status::Ok,
                        },
                        Limit {
                            label: "5h".into(),
                            used_pct: 80.4,
                            resets_in_ms: Some(1_000),
                            status: Status::Warning,
                        },
                    ],
                    subscription_ends_at_ms: None,
                    balance: None,
                }),
                Card::Live(Report {
                    provider: ProviderId::MiniMax,
                    source: QuotaSource::MinimaxRemainsApi,
                    plan_type: String::new(),
                    limits: vec![
                        Limit {
                            label: "7 days".into(),
                            used_pct: 20.0,
                            resets_in_ms: Some(86_400_000),
                            status: Status::Ok,
                        },
                        Limit {
                            label: "7 days Opus".into(),
                            used_pct: 20.0,
                            resets_in_ms: Some(86_400_000),
                            status: Status::Ok,
                        },
                    ],
                    subscription_ends_at_ms: None,
                    balance: None,
                }),
            ],
        };
        let Glance::Ready { stale: false, cards } = glance(&snapshot, generated_at_ms) else {
            panic!("fresh sample should be ready and not stale");
        };
        let Glance::Ready { stale: false, cards: edge } = glance(&snapshot, generated_at_ms + 120_000) else {
            panic!("120s is not stale");
        };
        let Glance::Ready { stale: true, cards: late } = glance(&snapshot, generated_at_ms + 120_001) else {
            panic!("120s + 1ms is stale");
        };
        assert_eq!(cards, edge);
        assert_eq!(cards, late);
        assert!(matches!(glance(&snapshot, generated_at_ms - 1), Glance::Ready { stale: false, .. }));
        assert_eq!(
            cards.iter().map(|card| card.title.as_str()).collect::<Vec<_>>(),
            vec!["Command Code GOAT", "Codex", "MiniMax Coding"]
        );
        assert!(cards.iter().all(|card| !card.title.contains("Claude") && !card.title.contains("Antigravity")));
        assert_eq!(
            cards[0].windows,
            vec![
                RemainingWindow {
                    label: WindowLabel::Week,
                    remaining_pct: 9,
                    level: WindowLevel::Warn,
                    resets_in_ms: None,
                },
                RemainingWindow {
                    label: WindowLabel::FiveHour,
                    remaining_pct: 58,
                    level: WindowLevel::Ok,
                    resets_in_ms: Some(ResetInMs(3_600_000)),
                },
            ]
        );
        assert_eq!(
            cards[1].windows,
            vec![
                RemainingWindow {
                    label: WindowLabel::Month,
                    remaining_pct: 0,
                    level: WindowLevel::Out,
                    resets_in_ms: None,
                },
                RemainingWindow {
                    label: WindowLabel::FiveHour,
                    remaining_pct: 20,
                    level: WindowLevel::Warn,
                    resets_in_ms: Some(ResetInMs(1_000)),
                },
            ]
        );
        assert_eq!(
            cards[2].windows,
            vec![
                RemainingWindow {
                    label: WindowLabel::Week,
                    remaining_pct: 80,
                    level: WindowLevel::Ok,
                    resets_in_ms: Some(ResetInMs(86_400_000)),
                },
                RemainingWindow {
                    label: WindowLabel::Week,
                    remaining_pct: 80,
                    level: WindowLevel::Ok,
                    resets_in_ms: Some(ResetInMs(86_400_000)),
                },
            ]
        );
        let ready = Glance::Ready { stale: false, cards };
        let json = serde_json::to_string(&ready).unwrap();
        for needle in ["usedPct", "hourly", "burnRate", "monthly", "subscription", "balance", "Opus"] {
            assert!(!json.contains(needle), "{needle} in {json}");
        }
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value["cards"][0]["windows"][0].get("resetsInMs").is_none());
        assert!(value["cards"][1]["windows"][0].get("resetsInMs").is_none());
        assert_eq!(value["cards"][0]["windows"][1]["resetsInMs"], 3_600_000);
        assert_eq!(value["cards"][1]["windows"][1]["resetsInMs"], 1_000);
    }

    #[test]
    fn standby_page_is_remaining_only() {
        let html = standby_html();
        assert!(html.contains("剩余"));
        assert!(!html.contains("30 天活跃时段"));
        assert!(!html.contains("viewUsage"));
    }
}
