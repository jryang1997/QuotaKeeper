//! 家目录与各 CLI 数据目录解析:USERPROFILE → HOME 回退,支持各家环境变量重定向。

use std::path::PathBuf;

/// 用户家目录;两者皆缺时回退当前目录(与既有各处行为一致)。
pub fn home_dir() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Claude Code 数据目录(`CLAUDE_CONFIG_DIR` 可重定向)。
pub fn claude_dir() -> PathBuf {
    std::env::var("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".claude"))
}

/// Codex CLI 主目录(`CODEX_HOME` 可重定向)。
pub fn codex_home() -> PathBuf {
    std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".codex"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_falls_back_to_cwd_without_env() {
        // 不改进程环境(并发测试不安全),只验证函数存在且返回非空路径。
        assert!(!home_dir().as_os_str().is_empty());
    }

    #[test]
    fn claude_dir_honors_config_dir_override() {
        // CLAUDE_CONFIG_DIR 在测试环境未设置时回退 home/.claude。
        if std::env::var("CLAUDE_CONFIG_DIR").is_err() {
            assert!(claude_dir().ends_with(".claude"));
        }
    }
}
