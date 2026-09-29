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
    /// cost 是否为 LiteLLM 估价(区别于来源方报告价,如 OMP stats.db).
    pub cost_is_estimate: bool,
    /// 会话标识(用于会话维度聚合);None = 该源不提供.
    pub session: Option<String>,
    /// 项目/工作目录(用于项目维度聚合).
    pub project: Option<String>,
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
