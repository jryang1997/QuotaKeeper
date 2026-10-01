//! 出站 HTTP 统一入口:按目标共享 Agent,自动应用 HTTP_PROXY / HTTPS_PROXY /
//! ALL_PROXY 系统代理。环回地址强制直连且不跟随重定向,避免 CPA 管理密钥
//! 经代理或重定向发到外部地址(ureq 2.x 不支持 NO_PROXY)。

use std::sync::OnceLock;

static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
static LOCAL_AGENT: OnceLock<ureq::Agent> = OnceLock::new();

fn agent(url: &str) -> &'static ureq::Agent {
    let remote = AGENT.get_or_init(|| ureq::AgentBuilder::new().build());
    let loopback = remote.get(url).request_url().ok().is_some_and(|parsed| {
        parsed.host().eq_ignore_ascii_case("localhost")
            || parsed
                .host()
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if loopback {
        LOCAL_AGENT.get_or_init(|| {
            ureq::AgentBuilder::new()
                .try_proxy_from_env(false)
                .redirects(0)
                .build()
        })
    } else {
        remote
    }
}

pub fn get(url: &str) -> ureq::Request {
    agent(url).get(url)
}

pub fn post(url: &str) -> ureq::Request {
    agent(url).post(url)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    #[test]
    fn loopback_bypasses_environment_proxy() {
        let direct = super::agent("http://127.0.0.1:8317");
        for url in [
            "http://localhost:8317",
            "http://[::1]:8317",
            "http://127.1:8317",
        ] {
            assert!(std::ptr::eq(direct, super::agent(url)));
        }
        for url in [
            "https://example.com",
            "http://127.0.0.1.evil.test",
            "http://localhost:8317@evil.test",
        ] {
            assert!(!std::ptr::eq(direct, super::agent(url)));
        }
        // 子进程单独设置环境,不干扰并发测试或用户的系统代理。
        if std::env::var_os("TOKEN_MONITOR_LOOPBACK_TEST").is_none() {
            let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = format!("http://{}", proxy.local_addr().unwrap());
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "http::tests::loopback_bypasses_environment_proxy",
                    "--nocapture",
                ])
                .env("TOKEN_MONITOR_LOOPBACK_TEST", "1")
                .env("ALL_PROXY", &address)
                .env("HTTP_PROXY", &address)
                .env("HTTPS_PROXY", &address)
                .env("NO_PROXY", "127.0.0.1,localhost")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            proxy.set_nonblocking(true).unwrap();
            assert_eq!(
                proxy.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            return;
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/quota", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            for method in ["GET", "POST"] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut headers = BufReader::new(&stream);
                let mut line = String::new();
                headers.read_line(&mut line).unwrap();
                assert!(line.starts_with(&format!("{method} /quota ")));
                loop {
                    line.clear();
                    assert!(headers.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                }
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
            }
        });
        for request in [super::get(&url), super::post(&url)] {
            assert_eq!(
                request
                    .timeout(Duration::from_secs(2))
                    .set("Authorization", "Bearer audit-test-only")
                    .call()
                    .unwrap()
                    .status(),
                200
            );
        }
        worker.join().unwrap();
    }

    #[test]
    fn agent_initializes_without_proxy_env() {
        // 无代理环境下 Agent 构建成功即可;带代理路径由集成环境验证。
        let _ = super::agent("https://example.com");
    }
}
