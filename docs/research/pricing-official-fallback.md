# 官方价兜底核验记录(vendor-official pricing fallback)

核验日期:2026-10-02。改动:`src/ledger/pricing.rs`(官方条目兜底 + 反向别名 + 补缺桶)、`src/ledger/model_alias.rs`(剥推理力度尾缀)。动机:本机 29.1% 的 token 因目录缺条目/渠道价打架无法估价;三家均有官方定价,兜底应覆盖。

## 兜底设计(不发明价格)

全部价格取自 LiteLLM 目录缓存里**实际存在的厂商官方条目**,三条路径:

1. **官方前缀表**(`OFFICIAL_PREFIXES`,只收录实测有官方条目的家族):glm-→zai、deepseek-→deepseek、gemini-→gemini、kimi-→moonshot、minimax-→minimax。裸名回落 `<前缀>/<canonical>`,其次裸 canonical 名。
2. **反向别名**(`model_alias::source_names`):目录只有推广短名条目时按短名取价(deepseek-flash ↔ deepseek-v4.1-flash)。
3. **补缺**:主条目缺消耗桶(cache_read/cache_write 无价)时用官方条目补缺桶,渠道价完整的桶一律保留;后缀歧义整体落空时官方条目兜底。

## 厂商官方价逐项核验(2026-10-02)

### DeepSeek — 逐项一致 ✓

来源:<https://api-docs.deepseek.com/quick_start/pricing>

- 官方 `deepseek-flash`(版本注明 **DeepSeek-V4.1-Flash**)峰值价:输入 $0.3/M、输出 $1.2/M、缓存读 $0.006/M —— 与目录条目 `deepseek-flash`(3e-7/1.2e-6/6e-9)**逐项一致**。
- 官方确认 v4-flash 等旧名退役后由 V4.1-Flash 承接、按 Flash 价计 → 反向别名取短名条目即官方价,语义成立。
- **缓存写**:官方无独立缓存写价格,缓存写按 cache-miss 价($0.3/M)计;LiteLLM 条目 `cache_write: 0.0` 与官方语义不同。**实测本机记录 deepseek/glm/gemini 三家族 cache_write 全为 0**(dsh/opencode/zcode 采集侧未上报该桶),故 cw=0 不会少算;若未来采集侧开始上报 cw,需把 DeepSeek 的补缺价改为 miss 价。
- 官方有峰谷价(谷 = 峰一半,01:00–04:00、06:00–10:00 UTC 工作日):估价取峰值(挂牌价),README「不反映时段」已声明。

### GLM-5.3-Flash(Z.ai 官方)— 逐项一致 ✓

来源:<https://docs.z.ai/guides/overview/pricing>

- 官方:输入 $0.15/M、输出 $0.50/M、缓存读 $0.03/M —— 与目录条目 `zai/glm-5.3-flash`(1.5e-7/5e-7/3e-8)**逐项一致**。
- 官方明示「Cached Input Storage: Limited-time Free」→ 条目 `cache_write: 0.0` 对 Z.AI **语义正确**(非少算)。

### Gemini 3.8 Flash(Google 官方)— 逐项一致 ✓

来源:<https://ai.google.dev/gemini-api/docs/pricing>

- 官方(2026-12-31 前):输入 $0.75/M、输出 $3.75/M、缓存读 $0.075/M —— 与目录条目 `gemini-3.8-flash`(7.5e-7/3.75e-6/7.5e-8)**逐项一致**。
- **2027-01-01 起全部翻倍**($1.50/$7.50/$0.15):目录无时间维度,依赖 LiteLLM 24h 后台刷新跟进;届时本兜底自动跟随新缓存。

## 本机验收(2026-10-02)

`cargo run --example unpriced_models`:

- 改动前:4 个模型缺价,884,125,280 tok / 3,035,742,429 tok(**29.1%**);
- 改动后:仅剩 `codex-auto-review`(Codex 内部 review 标签,非真模型,47,835 tok / 3 条),**≈0.1%**,有意忽略。

厂商核验人:Jerry 授权的自动核验(WebFetch 官方 pricing 页,2026-10-02);若厂商调价以官方页为准,本记录仅证明核验当日一致性。
