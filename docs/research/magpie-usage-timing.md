# 调用计时口径:从会话文件只读推导 TTFT 与时长

核验日期:2026-10-02。来源:yetone/magpie 的会话解析实现(只读旁路,与本应用同为「不碰流量、不改配置」的采集者)。本文记录其口径与我们采纳的规则,作为 `src/ledger/types.rs`(`timed_span`)、`claude.rs`、`codex.rs`、`ompstats.rs` 计时改动的依据。

## 背景

magpie 的 usage 记录(网关日志)带 TTFT 与毫秒时长;它对**不走网关的历史数据**同样能算出这两个数——从各家 agent 自己写的会话文件时间戳推导。这证明 TTFT/时长不需要网关在手,纯只读可测,值得搬进我们的账本。

参考源码(shallow clone @ 2026-10-02,主分支):

- 仓库:<https://github.com/yetone/magpie>
- `Call` 结构与口径注释:<https://github.com/yetone/magpie/blob/main/internal/sessions/calls.go>(`TTFT`、`Millis` 字段)
- Claude 解析:<https://github.com/yetone/magpie/blob/main/internal/sessions/claude.go>
- Codex 解析:<https://github.com/yetone/magpie/blob/main/internal/sessions/codex.go>

## magpie 的口径(calls.go 原注释摘要)

- **Codex**:TTFT/时长取 Codex 自报值;文件时间戳法只在多调用轮次上「末行减发起行」近似。
- **Claude**:Millis = 「the call's last line less the line that asked for it (a prompt, a tool's result)」,即**回应最后一行时间戳 − 发起行(prompt 或工具结果)时间戳**;自述「a little more or less than it was」——是近似值。
- 发起行按 assistant 行之前的最近 user 行配对,子代理(sidechain)各自成链,不串。

## 我们采纳的规则

1. **Claude**(`claude.rs`):按 `isSidechain` 分链,维护每链最近 user 行时间;`(message.id, requestId)` 组的首快照 − 发起行 = ttft,末快照 − 发起行 = duration。
2. **Codex**(`codex.rs`):`response_item` 的 `message role=user` 与 `event_msg` 的 `user_message` 都算发起行;本轮首条被接受的 `token_count` 记 ttft,末条记 duration(与 Claude 同一几何意义)。
3. **OMP**(`ompstats.rs`):stats.db 本身就有 `duration`/`ttft` 整数列(见我们既有测试建表语句),直接读;旧库缺列时 pragma 探测降级为 NULL。
4. **防臆造**(`types.rs::timed_span`):负跨度、无发起行、超上限(ttft 15 分钟 / duration 30 分钟)一律 0 = 不计;两段独立判断(一个无效不连坐另一个)。
5. **聚合**(`SpeedStats`):tok/s = 计时段合计输出 ÷ 计时总长;TTFT 只对有 ttft 样本的调用取均值(`ttft_samples` 单独计数),避免被「有时长无首字」的样本稀释。

## 明确不做

- Codex 自报 TTFT(其 rollout 文件里无此字段,magpie 是从网关日志拿的)。
- 增量解析续扫(content-hash+偏移):活跃文件重析毫秒级,收益不抵复杂度。
- 任何网关侧字段(status/error/request-id):我们看不到流量,不装作看得到。

## 对用户口径的影响

- GUI「实测速度」模块与 TUI 模型表新增 tok/s / 首字列为**估算值**(发起行含网络往返与本地处理,略高于厂商口径);界面上不标「精确」。
- 缓存命中率 = cache_read ÷ (input + cache_read);input 口径为采集侧已扣缓存的不重叠桶(types.rs 顶部口径注释)。
