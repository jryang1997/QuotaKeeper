//! 出站 HTTP 统一入口:共享一个 Agent,自动应用 HTTP_PROXY / HTTPS_PROXY /
//! ALL_PROXY 系统代理(ureq `proxy-from-env` feature,AgentBuilder 默认开启)——
//! 企业代理与镜像网络下价目表与配额接口不再全军覆没。

use std::sync::OnceLock;

static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

fn agent() -> &'static ureq::Agent {
    AGENT.get_or_init(|| ureq::AgentBuilder::new().build())
}

pub fn get(url: &str) -> ureq::Request {
    agent().get(url)
}

pub fn post(url: &str) -> ureq::Request {
    agent().post(url)
}

#[cfg(test)]
mod tests {
    #[test]
    fn agent_initializes_without_proxy_env() {
        // 无代理环境下 Agent 构建成功即可;带代理路径由集成环境验证。
        let _ = super::agent();
    }
}
