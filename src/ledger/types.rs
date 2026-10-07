//! Ledger 统一数据类型:五路 token 拆分 + 归一化记录 + 聚合.
//!
//! 口径:各 collector 在 parse 时把来源语义归一化为互不重叠的 input/output/cache_read/cache_write 四个计数桶;
//! reasoning 是 output 的信息性子集,不另加到 total。
//! (来源差异大:Codex 的 input 含 cached,ZCode 的 total=input+output 即 input 含
//! cache 且 reasoning 含在 output 里;而 Claude/OpenCode/DSH/OMP 本来就不重叠).
//! 归一化后 total/成本直接相加:
//! - 成本 = input×输入价 + output×输出价 + cache_read×读价 + cache_write×写价
//!   (reasoning 已含在 output 里,随 output 计价).

/// 展示顺序与标签.固定顺序,缺数据的客户端在表里不出现.
pub const CLIENTS: [(&str, &str); 6] = [
    ("codex", "Codex"),
    ("omp", "OMP"),
    ("opencode", "OpenCode"),
    ("zcode", "ZCode"),
    ("dsh", "DeepSeek"),
    ("claude", "Claude Code"),
];

pub fn client_label(id: &str) -> &str {
    CLIENTS
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| *v)
        .unwrap_or(id)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenBreakdown {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    /// info only,已含在 output 里.
    pub reasoning: i64,
}

impl TokenBreakdown {
    pub fn total(&self) -> i64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    pub fn is_zero(&self) -> bool {
        self.total() == 0
    }
}

#[derive(Debug, Clone)]
pub struct UsageRecord {
    pub client: &'static str,
    pub model: String,
    /// epoch 毫秒.<=0 视为时间未知,不进任何窗口.
    pub ts_ms: i64,
    pub tokens: TokenBreakdown,
    /// Some = 已有美元成本;None = 无价可算(UI 显示 "—").
    pub cost: Option<f64>,
    /// cost 是否为 LiteLLM 估价(区别于来源方报告价,如 OMP 会话 usage).
    pub cost_is_estimate: bool,
    /// 会话标识(用于会话维度聚合);None = 该源不提供.
    pub session: Option<String>,
    /// 项目/工作目录(用于项目维度聚合).
    pub project: Option<String>,
    /// 一次调用从发起行到最后一个流式快照的跨度;0 = 该源不提供.
    /// (Claude: assistant 首快照前的最近 user 行 → 末快照;Codex: 发起 user 行 → 本轮最后一条 token_count.)
    pub duration_ms: i64,
    /// 发起行到首个流式快照(首字延迟);0 = 不提供.与 duration 同一口径的子段.
    pub ttft_ms: i64,
}

/// 跨度上限:超过视为配对错乱(如子代理交错),宁可不计.
pub const TTFT_CAP_MS: i64 = 15 * 60_000;
pub const DURATION_CAP_MS: i64 = 30 * 60_000;

/// 发起行 → 首/末事件时间戳的跨度;负值或超上限一律 0 = 不计.
/// claude:(first,last) 为同一 message 的首/末快照;codex:单条记录 first=last.
pub fn timed_span(ask_ts: i64, first_ts: i64, last_ts: i64) -> (i64, i64) {
    if ask_ts <= 0 {
        return (0, 0);
    }
    let ttft = first_ts - ask_ts;
    let duration = last_ts - ask_ts;
    let ttft_ms = (ttft >= 0 && ttft <= TTFT_CAP_MS).then_some(ttft).unwrap_or(0);
    let duration_ms = (duration >= 0 && duration <= DURATION_CAP_MS)
        .then_some(duration)
        .unwrap_or(0);
    (ttft_ms, duration_ms)
}

#[cfg(test)]
mod timed_span_tests {
    use super::*;

    #[test]
    fn negative_and_over_cap_spans_are_zeroed() {
        // 首快照早于发起行 = ttft 负跨度,不计;duration 仍有效则独立保留
        assert_eq!(timed_span(2_000, 1_000, 3_000), (0, 1_000));
        // 完全倒挂(发起行晚于末快照):全零
        assert_eq!(timed_span(3_000, 1_000, 2_000), (0, 0));
        // 无发起行
        assert_eq!(timed_span(0, 1_000, 2_000), (0, 0));
        // 超上限不计
        assert_eq!(timed_span(1_000, 1_000 + TTFT_CAP_MS + 1, 2_000), (0, 1_000));
        assert_eq!(timed_span(1_000, 1_500, 1_000 + DURATION_CAP_MS + 1), (500, 0));
        // 恰好在上限边界仍计
        assert_eq!(
            timed_span(1_000, 1_000 + TTFT_CAP_MS, 1_000 + DURATION_CAP_MS),
            (TTFT_CAP_MS, DURATION_CAP_MS)
        );
    }
}

/// 按模型的实测速度样本(仅收录 duration/ttft 可用的记录).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpeedStats {
    /// 有计时时长样本的调用数.
    pub timed_calls: u64,
    /// 有首字延迟样本的调用数(可与 timed_calls 不同:duration 可得而 ttft 被上限裁掉).
    pub ttft_samples: u64,
    pub ttft_ms_total: i64,
    pub duration_ms_total: i64,
    pub output_total: i64,
}

impl SpeedStats {
    pub fn sample(&mut self, r: &UsageRecord) {
        if r.duration_ms <= 0 {
            return;
        }
        self.timed_calls += 1;
        self.duration_ms_total += r.duration_ms;
        if r.ttft_ms > 0 {
            self.ttft_samples += 1;
            self.ttft_ms_total += r.ttft_ms;
        }
        self.output_total += r.tokens.output.max(0);
    }

    /// 平均首字延迟毫秒;无 ttft 样本为 None.只对有 ttft 的样本取均值,不被缺样本稀释.
    pub fn avg_ttft_ms(&self) -> Option<f64> {
        (self.ttft_samples > 0).then(|| self.ttft_ms_total as f64 / self.ttft_samples as f64)
    }

    /// 输出 token / 秒,按计时段合计输出估算.
    pub fn tok_per_sec(&self) -> Option<f64> {
        (self.duration_ms_total > 0)
            .then(|| self.output_total as f64 / (self.duration_ms_total as f64 / 1000.0))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Agg {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub reasoning: i64,
    pub messages: u64,
    /// 来源方报告的成本合计(如 OMP stats.db 的 cost_total).
    pub cost_reported: f64,
    /// LiteLLM 估价的成本合计.
    pub cost_estimated: f64,
}

impl Agg {
    pub fn add(&mut self, t: &TokenBreakdown, cost: Option<f64>, cost_is_estimate: bool) {
        self.input += t.input;
        self.output += t.output;
        self.cache_read += t.cache_read;
        self.cache_write += t.cache_write;
        self.reasoning += t.reasoning;
        self.messages += 1;
        if let Some(c) = cost {
            if cost_is_estimate {
                self.cost_estimated += c;
            } else {
                self.cost_reported += c;
            }
        }
    }

    pub fn merge(&mut self, other: &Agg) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.reasoning += other.reasoning;
        self.messages += other.messages;
        self.cost_reported += other.cost_reported;
        self.cost_estimated += other.cost_estimated;
    }

    pub fn total(&self) -> i64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    pub fn cost(&self) -> f64 {
        self.cost_reported + self.cost_estimated
    }
}
