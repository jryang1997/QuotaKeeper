//! 采集侧环境变量注册表(纪律抄 magpie 的 internal/agentenv).
//!
//! 每个 collector 可被环境变量重定向数据目录;测试沙箱若漏清某个变量,
//! 就会把真机上的 `~/.claude` 之类扫进测试结果。这里的注册表 + 源码扫描
//! 测试形成双向闭环:
//! - 源码里新出现的 `env::var("…")` 必须登记,否则测试红;
//! - 注册表里的条目必须在源码里真实用到,防止名单腐化。
//!
//! 测试里凡调用 `scan_paths`/`collect`/`home_dir` 的,都应先把本表变量
//! 指到临时目录(或确认本机未设置)。

/// 各 CLI 数据目录重定向变量(新增 collector 时先登记这里).
pub const AGENT_ENV_VARS: [&str; 4] = [
    "CLAUDE_CONFIG_DIR",
    "CODEX_HOME",
    "DSH_HOME",
    "XDG_DATA_HOME",
];

/// 家目录解析变量(非 agent 专属,单独一组).
pub const HOME_ENV_VARS: [&str; 2] = ["USERPROFILE", "HOME"];

/// 扫描采集相关源码中的 `env::var("…")` 字面量.
#[cfg(test)]
fn env_vars_in(source: &str) -> Vec<&str> {
    let needle = "env::var(\"";
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(pos) = rest.find(needle) {
        let after = &rest[pos + needle.len()..];
        if let Some(end) = after.find('"') {
            out.push(&after[..end]);
            rest = after;
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 参与扫描的采集侧源文件:新 collector 建立时加进这张表.
    const SCANNED: [&str; 7] = [
        include_str!("claude.rs"),
        include_str!("codex.rs"),
        include_str!("dsh.rs"),
        include_str!("omp.rs"),
        include_str!("opencode.rs"),
        include_str!("zcode.rs"),
        include_str!("../paths.rs"),
    ];

    #[test]
    fn every_env_var_in_collectors_is_registered() {
        for (idx, source) in SCANNED.iter().enumerate() {
            for name in env_vars_in(source) {
                assert!(
                    AGENT_ENV_VARS.contains(&name) || HOME_ENV_VARS.contains(&name),
                    "SCANNED[{idx}] 读了未登记的环境变量 {name};请加进 agentenv::AGENT_ENV_VARS 并在测试沙箱里清掉它"
                );
            }
        }
    }

    #[test]
    fn registry_entries_are_actually_used() {
        let all = SCANNED.concat();
        for name in AGENT_ENV_VARS.iter().chain(HOME_ENV_VARS.iter()) {
            assert!(
                all.contains(&format!("env::var(\"{name}\")")),
                "注册表里的 {name} 已无源码使用;移出注册表或核实 collector 改名"
            );
        }
    }
}
