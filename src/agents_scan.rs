//! 本机 AI CLI Agent 扫描与更新(设置页「Agent 管理」)。
//!
//! 思路对齐 cc-switch:先按 PATH 顺序找默认入口(跳过 WindowsApps 执行别名,
//! 防项目目录内同名可执行文件被动执行),再扫原生安装落点兜底;每处安装跑
//! `--version`。npm 来源锚定 `npm install -g <pkg>@latest`,claude 原生装锚定
//! `<exe> update`;其余来源不自动生成命令,提示按原方式更新。执行命令只由
//! 后端生成,前端仅回传工具 id。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::http;

const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub struct AgentSpec {
    pub id: &'static str,
    pub title: &'static str,
    /// 候选可执行名(部分工具注册了多个,如 qodercli/qoder)
    pub exe: &'static [&'static str],
    pub npm_pkg: Option<&'static str>,
    /// PATH 之外补扫的原生安装目录;支持 ~ 与 %ENV% 前缀
    pub extra_dirs: &'static [&'static str],
}

pub fn specs() -> &'static [AgentSpec] {
    &[
        AgentSpec {
            id: "claude",
            title: "Claude Code",
            exe: &["claude"],
            npm_pkg: Some("@anthropic-ai/claude-code"),
            extra_dirs: &["~/.local/bin", "%LOCALAPPDATA%/Programs/claude"],
        },
        AgentSpec {
            id: "codex",
            title: "Codex",
            exe: &["codex"],
            npm_pkg: Some("@openai/codex"),
            extra_dirs: &["%LOCALAPPDATA%/Programs/OpenAI/Codex/bin"],
        },
        AgentSpec {
            id: "gemini",
            title: "Gemini CLI",
            exe: &["gemini"],
            npm_pkg: Some("@google/gemini-cli"),
            extra_dirs: &[],
        },
        AgentSpec {
            id: "qwen",
            title: "Qwen Code",
            exe: &["qwen"],
            npm_pkg: Some("@qwen-code/qwen-code"),
            extra_dirs: &[],
        },
        AgentSpec {
            id: "iflow",
            title: "iFlow CLI",
            exe: &["iflow"],
            npm_pkg: Some("@iflow-ai/iflow-cli"),
            extra_dirs: &[],
        },
        AgentSpec {
            id: "opencode",
            title: "OpenCode",
            exe: &["opencode"],
            npm_pkg: Some("opencode-ai"),
            extra_dirs: &["~/.opencode/bin"],
        },
        AgentSpec {
            id: "crush",
            title: "Crush",
            exe: &["crush"],
            npm_pkg: Some("@charmland/crush"),
            extra_dirs: &[],
        },
        AgentSpec {
            id: "copilot",
            title: "Copilot CLI",
            exe: &["copilot"],
            npm_pkg: Some("@github/copilot"),
            extra_dirs: &[],
        },
        AgentSpec {
            id: "qoder",
            title: "Qoder CLI",
            exe: &["qodercli", "qoder"],
            npm_pkg: Some("@qoder-ai/qodercli"),
            extra_dirs: &[],
        },
    ]
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstall {
    pub path: String,
    /// npm | scoop | native | unknown
    pub source: &'static str,
    pub version: Option<String>,
    pub is_default: bool,
    pub update_command: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentScan {
    pub id: String,
    pub title: String,
    pub installed: bool,
    pub installs: Vec<AgentInstall>,
    pub latest: Option<String>,
    /// 默认安装落后于 latest(npm dist-tags);latest 拿不到时恒为 false
    pub upgradable: bool,
}

impl AgentScan {
    /// 默认安装(PATH 第一个命中);未装或版本读不出时给 None。
    pub fn default_install(&self) -> Option<&AgentInstall> {
        self.installs
            .iter()
            .find(|install| install.is_default && install.version.is_some())
    }

    /// 默认安装是否落后于 latest。两端都能解析成语义化版本就走数值比较
    /// (opencode 二进制 v2.x 而 npm 包 1.x,纯字符串比较会误报),否则退回字符串。
    pub fn upgradable(&self) -> bool {
        let (Some(latest), Some(version)) = (self.latest.as_deref(), self.default_install()) else {
            return false;
        };
        let Some(installed) = version.version.as_deref() else {
            return false;
        };
        match (semver_of(installed), semver_of(latest)) {
            (Some(a), Some(b)) => a < b,
            _ => installed != latest,
        }
    }
}

/// 从 "opencode v2.0.15" / "codex-cli 0.159.2" 里取首个 x[.y][.z] 数字段。
fn semver_of(text: &str) -> Option<Vec<u32>> {
    text.split_whitespace()
        .filter_map(|word| {
            let digits = word.trim_start_matches(|c: char| !c.is_ascii_digit());
            let digits = digits.trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.');
            let trimmed = digits.trim_matches('.');
            if trimmed.is_empty() {
                return None;
            }
            trimmed
                .split('.')
                .map(|part| part.parse::<u32>().ok())
                .collect::<Option<Vec<u32>>>()
        })
        .next()
}

/// PATH 之外按工具补扫的目录(展开 ~ 与 %ENV%)。
fn extra_dirs(spec: &AgentSpec) -> Vec<PathBuf> {
    spec.extra_dirs
        .iter()
        .filter_map(|pattern| expand_dir(pattern))
        .collect()
}

fn expand_dir(pattern: &str) -> Option<PathBuf> {
    let expanded = if let Some(rest) = pattern.strip_prefix("~/") {
        home_dir()?.join(rest)
    } else if let Some(name) = pattern.strip_prefix('%').and_then(|rest| rest.split_once("%/")) {
        std::env::var_os(name.0).map(|value| PathBuf::from(value).join(name.1))?
    } else {
        PathBuf::from(pattern)
    };
    Some(expanded)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

/// 展示用规整:canonicalize 后去掉 \\?\ 前缀,失败退原样。
fn normalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path)
        .map(|canonical| {
            let text = canonical.to_string_lossy();
            match text.strip_prefix(r"\\?\") {
                Some(rest) => PathBuf::from(rest),
                None => canonical,
            }
        })
        .unwrap_or_else(|_| path.to_path_buf())
}

fn path_dirs() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default()
}

fn is_excluded(dir: &Path) -> bool {
    // WindowsApps 是商店的 App Execution Alias,不是真实安装
    dir.to_string_lossy()
        .to_ascii_lowercase()
        .contains(r"\windowsapps")
}

fn exe_candidates(name: &str) -> [String; 4] {
    [
        format!("{name}.exe"),
        format!("{name}.cmd"),
        format!("{name}.bat"),
        name.to_string(),
    ]
}

/// 按工具枚举安装处:PATH 命中在前(顺序即优先级),extra_dirs 兜底在后。
fn find_installs(spec: &AgentSpec) -> Vec<AgentInstall> {
    let mut seen = HashSet::new();
    let mut installs = Vec::new();
    for dir in path_dirs()
        .into_iter()
        .filter(|dir| !is_excluded(dir))
        .chain(extra_dirs(spec))
    {
        for name in spec.exe {
            for candidate in exe_candidates(name) {
                let path = dir.join(&candidate);
                if path.is_file() {
                    let normalized = normalize(&path);
                    if seen.insert(normalized.to_string_lossy().to_ascii_lowercase()) {
                        installs.push(AgentInstall {
                            path: normalized.to_string_lossy().into_owned(),
                            source: classify_source(&normalized),
                            version: None,
                            is_default: false,
                            update_command: None,
                        });
                    }
                    break;
                }
            }
        }
    }
    installs
}

fn classify_source(path: &Path) -> &'static str {
    let text = path.to_string_lossy().to_ascii_lowercase();
    if text.contains(r"\windowsapps\") {
        "store"
    } else if text.contains("node_modules") {
        "npm"
    } else if text.ends_with(".cmd") || text.ends_with(".bat") {
        // npm 全局 shim 脚本(nvm/pnpm 等管理的 prefix 目录同名)
        "npm"
    } else if text.contains(r"\scoop\") {
        "scoop"
    } else {
        "native"
    }
}

/// 生成锚定更新命令;None 表示来源不明,提示按原安装方式更新。
pub fn update_command(spec: &AgentSpec, install: &AgentInstall) -> Option<String> {
    match install.source {
        "npm" => spec.npm_pkg.map(|pkg| format!("npm install -g {pkg}@latest")),
        // 只有 claude 原生安装有可靠自更新;codex 独立装的 update 断网会假成功
        "native" if spec.id == "claude" => Some(format!("\"{}\" update", install.path)),
        _ => None,
    }
}

/// 跑 `--version`,OEM/UTF-8 混编统一 lossy 处理;超时击杀返回 None。
fn probe_version(path: &Path) -> Option<String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut command = if extension == "cmd" || extension == "bat" {
        let mut command = Command::new("cmd");
        command.args(["/C", "call"]);
        command.arg(path);
        command
    } else {
        Command::new(path)
    };
    command.arg("--version");
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    finish_with_timeout(&mut command, VERSION_TIMEOUT)
        .map(|output| parse_version(&output))
        .flatten()
}

fn parse_version(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(64).collect())
}

fn finish_with_timeout(command: &mut Command, timeout: Duration) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().ok()?;
                let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
                if !status.success() && text.trim().is_empty() {
                    text = String::from_utf8_lossy(&output.stderr).into_owned();
                }
                return Some(text);
            }
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    }
}

pub fn fetch_latest(pkg: &str) -> Option<String> {
    let url = format!("https://registry.npmjs.org/-/package/{pkg}/dist-tags");
    let response = http::get(&url)
        .timeout(Duration::from_secs(10))
        .call()
        .ok()?;
    let tags: serde_json::Value = response.into_json().ok()?;
    tags.get("latest")?.as_str().map(str::to_string)
}

pub fn scan_one(spec: &AgentSpec) -> AgentScan {
    let mut installs = find_installs(spec);
    for (index, install) in installs.iter_mut().enumerate() {
        install.version = probe_version(Path::new(&install.path));
        install.is_default = index == 0;
        install.update_command = update_command(spec, install);
    }
    let latest = spec.npm_pkg.and_then(fetch_latest);
    let mut scan = AgentScan {
        id: spec.id.to_string(),
        title: spec.title.to_string(),
        installed: !installs.is_empty(),
        installs,
        latest,
        upgradable: false,
    };
    scan.upgradable = scan.upgradable();
    scan
}

/// 全表并行扫描(每工具一线程,工具内串行探测)。
pub fn scan_all() -> Vec<AgentScan> {
    std::thread::scope(|scope| {
        specs()
            .iter()
            .map(|spec| scope.spawn(move || scan_one(spec)))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| {
                handle.join().unwrap_or_else(|_| AgentScan {
                    id: String::new(),
                    title: String::new(),
                    installed: false,
                    installs: Vec::new(),
                    latest: None,
                    upgradable: false,
                })
            })
            .collect()
    })
}

/// 执行更新命令:写临时 .bat(cmd /C 才能解析 npm shim),静默、限时,
/// 返回输出尾部供前端展示。命令只由后端生成。
pub fn run_update(command: &str) -> Result<String, String> {
    let mut script = std::env::temp_dir();
    script.push(format!("token-monitor-update-{}.bat", std::process::id()));
    std::fs::write(&script, format!("@echo off\r\ncall {command}\r\nif errorlevel 1 exit /b 1\r\n"))
        .map_err(|error| format!("写入临时脚本失败: {error}"))?;
    let result = (|| {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(&script).stdout(Stdio::piped()).stderr(Stdio::piped());
        finish_with_timeout(&mut cmd, UPDATE_TIMEOUT).ok_or_else(|| "更新超时,已终止".to_string())
    })();
    let _ = std::fs::remove_file(&script);
    let text = result?;
    let status_line = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .last()
        .unwrap_or("")
        .trim();
    Ok(status_line.chars().take(200).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_commands_anchor_by_source() {
        let spec = &specs()[0]; // claude
        let npm = AgentInstall {
            path: r"C:\Users\x\AppData\Roaming\npm\claude.cmd".into(),
            source: "npm",
            version: None,
            is_default: true,
            update_command: None,
        };
        let native = AgentInstall {
            path: r"C:\Users\x\.local\bin\claude.exe".into(),
            source: "native",
            version: None,
            is_default: true,
            update_command: None,
        };
        assert_eq!(
            update_command(spec, &npm).as_deref(),
            Some("npm install -g @anthropic-ai/claude-code@latest")
        );
        assert_eq!(
            update_command(spec, &native).as_deref(),
            Some(r#""C:\Users\x\.local\bin\claude.exe" update"#)
        );
    }

    #[test]
    fn unknown_sources_get_no_command() {
        let spec = &specs()[1]; // codex
        let native = AgentInstall {
            path: r"C:\Users\x\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe".into(),
            source: "native",
            version: None,
            is_default: true,
            update_command: None,
        };
        assert_eq!(update_command(spec, &native), None);
    }

    #[test]
    fn classify_prefers_npm_over_native() {
        assert_eq!(
            classify_source(Path::new(r"C:\u\AppData\Roaming\npm\node_modules\@x\y\cli.js")),
            "npm"
        );
        assert_eq!(classify_source(Path::new(r"C:\u\scoop\shims\crush.exe")), "scoop");
        assert_eq!(classify_source(Path::new(r"C:\tools\foo.exe")), "native");
    }

    #[test]
    fn parse_version_takes_first_non_empty_line() {
        assert_eq!(
            parse_version("\r\n2.1.0 (Claude Code)\r\n"),
            Some("2.1.0 (Claude Code)".to_string())
        );
        assert_eq!(parse_version("   \n\t\n"), None);
    }

    #[test]
    fn default_install_skips_versionless_entries() {
        let scan = AgentScan {
            id: "gemini".into(),
            title: "Gemini CLI".into(),
            installed: true,
            installs: vec![AgentInstall {
                path: "a".into(),
                source: "npm",
                version: None,
                is_default: true,
                update_command: None,
            }],
            latest: None,
            upgradable: false,
        };
        assert!(scan.default_install().is_none());
    }

    #[test]
    fn semver_extraction_handles_branded_output() {
        assert_eq!(semver_of("opencode v2.0.15"), Some(vec![2, 0, 15]));
        assert_eq!(semver_of("codex-cli 0.159.2"), Some(vec![0, 159, 2]));
        assert_eq!(semver_of("2.1.288 (Claude Code)"), Some(vec![2, 1, 288]));
        assert_eq!(semver_of("no version here"), None);
    }

    #[test]
    fn upgradable_compares_numerically_when_possible() {
        let scan = |version: &str, latest: Option<&str>| AgentScan {
            id: "opencode".into(),
            title: "OpenCode".into(),
            installed: true,
            installs: vec![AgentInstall {
                path: "a".into(),
                source: "npm",
                version: Some(version.to_string()),
                is_default: true,
                update_command: None,
            }],
            latest: latest.map(str::to_string),
            upgradable: false,
        };
        // 二进制 v2.x 高于 npm 包 latest 1.x:不算可升级
        assert!(!scan("opencode v2.0.15", Some("1.18.34")).upgradable());
        assert!(scan("0.159.2", Some("0.160.0")).upgradable());
        assert!(!scan("1.4.0", Some("1.4.0")).upgradable());
        // 解析不出数字段时退回字符串比较
        assert!(scan("beta", Some("rc")).upgradable());
        assert!(!scan("same", Some("same")).upgradable());
        assert!(!scan("1.0.0", None).upgradable());
    }

    /// 实机冒烟:只跑 --version 与 npm dist-tags,只读不写。
    /// cargo test --lib agents_scan -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_machine_scan_smoke() {
        for scan in scan_all() {
            println!(
                "{:<12} installed={:<5} latest={:?}",
                scan.id, scan.installed, scan.latest
            );
            for install in &scan.installs {
                println!(
                    "    {:<40} {:<7} v{:?} default={} cmd={:?}",
                    install.path,
                    install.source,
                    install.version,
                    install.is_default,
                    install.update_command
                );
            }
        }
    }

    /// 实机冒烟:更新执行链路(临时 .bat + 超时 + 输出捕获),用 echo 不改系统。
    #[test]
    #[ignore]
    fn run_update_echo_smoke() {
        println!("{:?}", run_update("echo hello-from-token-monitor"));
    }
}
