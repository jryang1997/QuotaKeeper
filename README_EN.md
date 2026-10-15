<div align="center">

# QuotaKeeper

Subscription quota and local token usage on Windows. See what remains and what you have used. Read-only collection keeps agent configuration untouched.

![GUI quota page](assets/screenshots/gui-overview.png)

[![CI](https://github.com/jryang1997/QuotaKeeper/actions/workflows/ci.yml/badge.svg)](https://github.com/jryang1997/QuotaKeeper/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![TUI Rust 1.88+](https://img.shields.io/badge/TUI%20Rust-1.88%2B-orange)
![Platform: Windows first](https://img.shields.io/badge/platform-Windows%20first-lightgrey)

</div>

> 中文文档见 [README.md](README.md)。

## What it does

For heavy users of AI coding CLIs (Claude Code, Codex, OpenCode, z.ai GLM, Kimi, DeepSeek…): quotas are pulled directly from each provider's official/login-state endpoints, the token ledger reads local session files, and USD pricing uses LiteLLM's public price list (refreshed in the background every 24 hours, with offline cache). Everything stays on your machine — no telemetry, no upload.

The Windows desktop app uses Rust and WebView2. Navigation has three destinations: Subscriptions, Token usage, and Settings.

- **Subscriptions** shows every quota window, with remaining quota by default and an optional used view. Reset times and subscription expiry stay separate. Old readings retain their observation time; a passed reset waits for refresh without refilling the quota. Unconnected and hidden providers are folded together.
- **Token usage** starts with local today, week, and month totals. The detail page applies the same date and source filters to totals, requests, API equivalents, trends, token buckets, and source distribution. CSV and JSON exports use those filters. API equivalents are reference amounts, not subscription charges; unpriced records stay visible.
- **Desktop presence** includes the tray, quota overlay, quota threshold and reset alerts, and expiry reminders. Settings can enable a read-only LAN quota page.
- Chinese and English, with System, Light, Dark, and Cassette Future themes. Removed theme preferences fall back to System. Quotas refresh every 60 seconds; local records refresh every 5 seconds. One provider failure does not block the others.

Achievements, PNG share cards, editable module boards, work-habit analysis, and CLI upgrades have been removed. The [product research](docs/research/product-focus-2026-10-15.md) records the scope decision and Magpie primary sources. The existing TUI remains available with frozen functionality.

## Data sources

### Quota (collected independently)

| Card | Path | Credentials |
|---|---|---|
| OpenCode Go | official usage API | Windows Credential Manager / `OPENCODE_GO_API_KEY` / OpenCode login |
| Codex | official `app-server` (fallback: wham endpoint → OMP) | Codex CLI login (`CODEX_HOME` honored) |
| Command Code GOAT | billing API | Credential Manager / `COMMAND_CODE_API_KEY` / `cmdc login` |
| Antigravity | Google upstream via [CPA](https://github.com/luispater/CLIProxyAPI) | `~/.quotakeeper/cpa-key` |
| Claude Pro/Max | official `oauth/usage` endpoint | Claude Code login (`~/.claude/.credentials.json`) |
| Cursor | usage-summary (undocumented endpoint) | Cursor login (state.vscdb / `cursor-agent login`) |
| GLM Coding | z.ai / bigmodel monitor endpoint | `GLM_API_KEY`; global region also needs `GLM_API_BASE=https://api.z.ai` |
| Kimi Coding | coding usage endpoint | `KIMI_CODE_API_KEY` (Kimi For Coding, not a Moonshot platform key) |
| MiniMax Coding | official coding_plan remains endpoint | Credential Manager `TuiTokenMonitor/MiniMax` / `MINIMAX_CODING_API_KEY` |

GLM/Kimi key inputs are not yet wired into Settings; set the environment variable and restart. DeepSeek API balance is displayed separately from subscription quota.

Cards with a billing cycle (GOAT / Codex / Cursor) show their **subscription end date** and fire tiered system reminders 7/3/1 days before expiry (deduplicated; silent after renewal). See the [subscription-period research notes](docs/research/subscription-period-sources.md).

### Token ledger (six sources, purely local)

Codex sessions, OMP stats.db, OpenCode db, ZCode db, DeepSeek Harness zstd sessions, Claude Code transcripts — see the Chinese README for the path table.

> **Unofficial endpoints**: Codex wham, GOAT `/alpha/billing`, and Google `retrieveUserQuotaSummary` are undocumented internal APIs with no stability guarantee. Failures degrade to a Missing card only. Primary-source research notes live in [docs/research/](docs/research/).

External requests honor `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY`. Loopback requests, including local CPA, bypass environment proxies and do not follow redirects.

## Install

### TUI (existing version, feature-frozen)

Requires Rust 1.88+ (edition 2024).

```sh
cargo install --path .    # or cargo build --release
```

### Windows GUI

Requires Rust 1.90+ and Node.js.

```sh
cd gui/frontend && npm ci && npm run build && cd ../..
cargo build --release --manifest-path gui/Cargo.toml
run_gui.bat
```

Needs the WebView2 Runtime (preinstalled on Win10/11). Frontend assets are embedded into the binary via `include_dir!`. The GUI exe carries the app icon; `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/new-gui-shortcut.ps1` creates a desktop shortcut that launches the exe directly (no console flash).

## Platform support

| | TUI | Desktop GUI |
|---|---|---|
| Windows | ✅ first-class (MSVC & GNU both CI-verified) | ✅ WebView2 |
| macOS / Linux | ✅ builds + full ledger; quota cards limited to env-var/CLI-login credentials, no system toast | ❌ |

## Privacy

All data stays local. No telemetry. API keys go only to the Windows Credential Manager. The app never mutates CLI-owned state (login files are read-only; OAuth tokens are never refreshed on your behalf).
The LAN display is off by default. When enabled, devices on that local network can read remaining quota; keys are not exposed.

## Verification

```sh
cargo test                                   # fixture replay, in-memory SQLite, multi-frame zstd, headless rendering
cargo test --manifest-path gui/Cargo.toml    # GUI shell tests
npm --prefix gui/frontend test                # quota boundaries, filtered usage, preferences and currency
cargo run --example ledger_smoke
```

## Acknowledgements and sources

The [2026-10-01 code audit](docs/research/codebase-audit-2026-10-01.md) and [competitor comparison](docs/research/codebase-audit-landscape-2026-10-01.md) record fixes and priorities. Pricing refreshes in the background every 24 hours; offline reads use the existing cache. Consumed cache buckets without prices are filled from the vendor's official catalog entry first (complete reseller buckets are kept) and only remain unpriced when no official entry covers them. Bare or suffix-ambiguous model names fall back to official entries (zai/deepseek/gemini/moonshot/minimax families), and promo short names resolve through reverse aliases (e.g. deepseek-flash ↔ deepseek-v4.1-flash). All ledger views stop at collection time. CSV prefixes formula-like text with an apostrophe for spreadsheet viewing; JSON preserves the original text. Spreadsheet re-saving may change CSV escaping.

Research for quota collection and presentation drew on the public implementations and documentation of [CodexBar](https://github.com/steipete/CodexBar), [CodeBurn](https://github.com/getagentseal/codeburn), [tokscale](https://github.com/junhoyeo/tokscale), and [ccusage](https://github.com/ccusage/ccusage). See the [research notes](docs/research/) for data sources and tradeoffs, and the [logo provenance](gui/frontend/public/logos/README.md) for brand assets.

## License

[MIT](LICENSE)
