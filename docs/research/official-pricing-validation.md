# 官方订阅与模型价格核验

核验日期：2026-09-26（UTC/环境日期）。本文仅记录当日可见的官方页面和仓库现有价格计算方式。服务价格会变动；用户实际扣款以账号结账页和账单为准。

## 结论

仓库当前把 OpenCode Go $10、ChatGPT Plus $20、Command Code GOAT $10 相加为固定 **$40/月**（见 [`src/model.rs`](../../src/model.rs)）。这三个标价能在官方页面找到，但 $40 只能描述三个美元标价的简单相加：不能确认是每位用户实际支付的含税总价，也不能代表所有地区、付款渠道和账期。尤其 GOAT 页面写明另有 processing fee；ChatGPT 网页订阅提供多币种本地定价，手机应用内订阅由 Apple/Google 以当地货币收费。

旧 $10/$20/$10 不应作为普遍或固定事实：ChatGPT Plus 的 $20 是官方美元月标价；OpenCode Go 和 GOAT 的标价目前也分别是 $10/月，但用户最终金额、促销/额度和税费条件不同。价格页和合同条款还可能更新或对额度做不同说明。

模型价格方面，代码**没有内置并逐项核验过的一份官方价目表**。未由来源直接给出成本的记录，会按 LiteLLM 的公开模型价目数据估价，文件缓存 7 天；只有未命中本地缓存/过期时才尝试联网刷新。它是第三方聚合数据，不是各模型厂商的官方价格承诺。现有计算只取常规输入、输出、缓存读写单价，不足以还原每次请求的实际账单。

## 套餐价格：官方页面核验

| 套餐 | 截至核验日官方页面能确认的内容 | 地区、税费和可变项 |
|---|---|---|
| OpenCode Go | [OpenCode Go 页面](https://dev.opencode.ai/go)标为 **$10/月**。[OpenCode 官方 changelog](https://dev.opencode.ai/changelog) 的 v1.18.22（2026-08-24）称已移除“首月优惠”的过期文案。因此当前不能再把旧文档搜索摘要中的“首月 $5”当作有效价格。 | 官方 Go 页面列美元月价；本次核对的该页面未给出通用含税/手续费说明。结账页的地区可用性、当地税费和最终扣款金额仍须以用户账号为准。套餐提供的模型/额度列表会变化。 |
| ChatGPT Plus | [OpenAI 帮助中心](https://help.openai.com/en/articles/6950777-what-is-chatgpt-plus)写明 **$20/月，按月付费**，且 API 用量另行计费。 | [OpenAI 多币种说明](https://help.openai.com/en/articles/10421635-multicurrency-billing)确认网页订阅支持按地区本地货币定价；通过 iOS/Android 订阅由 Apple/Google 管理并以当地货币收费。该帮助页指向结账页查询具体价格；因此 $20 USD 不能视为全球统一的最终扣款额。帮助中心另有 VAT 豁免申请入口。 |
| Command Code GOAT | [Command Code 官方价格页](https://commandcode.ai/pricing)写明 **$10/月 + processing fee**，页面列出 $70 credits；[GOAT 计划说明](https://commandcode.ai/docs/plans/goat)列有计划额度。同一价格页另有单独的 Go $1/月档，不能和 GOAT 混为一谈。 | [官方条款](https://commandcode.ai/terms)写明款项以 USD 支付、付款须在美国，并由用户承担适用税费；条款同时称不会保证超过 1:1 的 API credits/订阅费比例，实际用量可能受模型价格、供应商费率、税费和运营成本影响。附加额度/促销可调整、撤回或到期。因此页面 $70 credits/优惠不能当成不变的现金价值保证。价格页没有给出 processing fee 的统一金额。 |

**可确认的边界：**按三个页面当前显示的基本美元月标价相加是 $40；本次没有确认一个适用于所有用户的 $40 含税、含手续费实际月支出。Command Code 的条款还写了付款须在美国，单看 GOAT 价格页不能确认所有国家/地区都可完成购买。套餐费用应以用户账单核算，不能仅用模型 API 列价反推。

## API 模型价格：官方来源与会影响估价的条件

以下是官方定价入口和核验到的计价事实，用来确认价格有厂商一手来源；不是对 LiteLLM 当前每一行、每一个模型版本的逐项审计。

| 提供方 / 官方来源 | 核验到的计价事实（USD，除非注明） | 对按 token 估价的影响 |
|---|---|---|
| [OpenAI API Pricing](https://developers.openai.com/api/docs/pricing) | 页面按模型、上下文长度、标准/批量/Flex/Fast 等处理方式列输入、缓存输入/写入和输出价格。例如标准 GPT-6 Sol 页面列价会随上下文档位变化；符合条件的数据驻留端点可能有 10% 加价，FedRAMP 端点也另有加价。ChatGPT Plus 费用不包含 API。 | 单个模型 ID 的一组固定 input/output 单价无法表示不同上下文、处理档位、数据驻留端点或工具调用费用。 |
| [Anthropic Claude API Pricing](https://platform.claude.com/docs/en/about-claude/pricing) | 价格以 USD 列示，分普通输入/输出、5 分钟和 1 小时缓存写入、缓存命中；页面当前列出不同模型的不同单价。Anthropic 的 Google Cloud / Bedrock 区域及多区域端点对适用模型可能比 global 高 10%。 | 缓存读写不能简单按普通输入价计；通过云平台使用时也不能默认一律套用 Anthropic global 价。 |
| [Google Gemini Developer API Pricing](https://ai.google.dev/gemini-api/docs/pricing) | 按模型列免费档和付费档；另有 Standard、Batch、Flex、Priority 等价格。页面当前列 Gemini 3.8 Flash 标准价至 2026-12-31 为输入 $0.75、输出 $3.75/百万 token，并列出 2027-01-01 起的调整价；Google Search / Maps grounding 等工具还可能按调用另计。 | 免费档、服务模式、缓存存储和工具费用不能由 token 数及模型名单独确定；公开价还可能有明确的生效日期。 |
| [Google Cloud Gemini Enterprise Agent Platform Pricing](https://cloud.google.com/gemini-enterprise-agent-platform/generative-ai/pricing)（原 Vertex AI Generative AI Pricing 页面现跳转至此） | 同一个 Gemini 3.8 Flash 在页面列出 Global 与 Non-global 端点价格；当前至 2026-12-31 的输入/输出分别列为 Global $0.75/$3.75、Non-global $0.825/$4.125/百万 token，2027 起再次调整。页面还按区域、模型、请求类型列价。 | Gemini Developer API 与 Google Cloud 平台价格/条款不是可互换的统一价目；仅凭模型名称不能知道端点和区域。 |
| [DeepSeek Models & Pricing](https://api-docs.deepseek.com/quick_start/pricing/) | 当前页面按缓存命中/未命中、输入/输出和高峰/非高峰分别列价。DeepSeek V4.1 Flash 当前列缓存命中非高峰/高峰 $0.003/$0.006，未命中 $0.15/$0.30，输出 $0.60/$1.20；峰值时段为工作日 UTC 01:00–04:00、06:00–10:00。旧 `deepseek-v4-flash` 名称会被兼容路由到 V4.1 Flash。 | 固定价表无法完全跟上时段变化；不同模型别名在官方侧可能指向同一当前模型。 |
| [Alibaba Cloud Model Studio Model Pricing](https://www.alibabacloud.com/help/en/model-studio/model-pricing) | 2026-09-11 更新；页面按模型、部署范围/地区、请求输入长度列价，并说明部分模型有批量折扣、上下文缓存、免费额度和限时优惠。例如 Qwen 3.7 Plus 的国际部署按请求输入长度分档。 | 单价可能取决于单次输入长度、部署地区/范围、缓存、批处理和免费额度；固定模型价不足以精确算账。 |
| [xAI API Pricing](https://docs.x.ai/developers/pricing) | 页面以 USD/百万 token 列模型及上下文长度档、缓存输入与输出价格；带服务端工具的请求还由 token 和工具调用两部分计价。 | 上下文长度和工具调用会改变最终价。 |
| [Kimi API Model Inference Pricing](https://platform.kimi.com/docs/pricing/chat) | 官方页说明按输入/输出 token 计价，并分别说明 K3/K2 的价格入口、自动上下文缓存、缓存写入时长；联网搜索另有功能定价。 | 需要区分缓存和功能调用；模型/产品规则应从当前官方页面读取。 |
| [Z.AI API Pricing](https://docs.z.ai/guides/overview/pricing) | 页面明示美元/百万 token，列 GLM 普通输入、缓存输入、输出和免费模型；Web Search 单独标为每次调用 $0.01。 | 仅按模型 token 计算会漏掉工具调用价；缓存存储也可能单独收费或限时免费。 |
| [MiniMax API / Token Plan](https://platform.minimax.io/subscribe/token-plan?tab=api-enterprise) | 官方页面把 PAYG API 与 Token Plan 分开；PAYG M3 价格按上下文长度档、输入/输出/缓存读分别列价。官方付费服务条款也说明标准 API key 与 Token Plan key 不可互换。 | 套餐用量不等于公有 API 单价下的现金账单；长上下文以及套餐额度须区分。 |
| [Mistral API Pricing](https://mistral.ai/pricing/api/) | 官方定价页提供 Standard、Batch、Priority 和 Regional inference 选项；页面标出区域端点加价、缓存输入折扣，且说明某些 OCR、语音和工具按页/时长/调用计价。 | 同一模型价格随服务档、地域和非 token 计费项变化。 |

## 本仓库当前如何定价

依据 [`src/ledger/pricing.rs`](../../src/ledger/pricing.rs)、[`src/ledger/mod.rs`](../../src/ledger/mod.rs) 和 [`src/ledger/ompstats.rs`](../../src/ledger/ompstats.rs)：

1. 仅当某条记录没有来源提供的成本时，才取 LiteLLM 的 `model_prices_and_context_window.json`；优先 jsDelivr 镜像、再用 GitHub raw，缓存到本机 7 天。失败时回退旧缓存，仍无数据则显示无价格。
2. 用量估算公式为 `输入 token × 输入单价 + 输出 token × 输出单价 + 缓存读取 token × 缓存读取单价 + 缓存写入 token × 缓存写入单价`。推理 token 已在输出中计价。它没有按请求时段、上下文长度分档、国家/区域、批处理/优先档、免费额度、工具费或订阅折扣切换价格，也不保存每条记录当时使用的价格版本。
3. 查价先匹配完整模型名（精确大小写或唯一的大小写不敏感匹配）；完整名不存在时，只有 provider 后缀在价目表中唯一才会匹配。多个 provider 使用同一后缀时会标为无价，不会任选一个费率。模型名本身仍不能确认实际服务端点、地区或价格档位。
4. OMP 记录有 `cost_total` 时仓库直接采用该来源给出的金额，不走 LiteLLM；代码并未将它重新与官方账单核对。因此月度金额可能包含“来源报告的成本”和“LiteLLM 单价估算”两种来源。

因此当前月度金额是**来源报告金额与 LiteLLM 模型价格估算的混合汇总**，可作 API 价格等值参考；它不是统一按官方 API 价目重算的金额，也不是订阅实际支出或已节省/赚回的现金。GUI 月度卡片把来源报告金额与 LiteLLM 估价分开显示在说明气泡中；主数字只作 API 等值和基础标价倍数对比。尤其 OpenCode Go 和 GOAT 都会通过自己的套餐折扣、模型额度或协商条件提供模型，第三方厂商公开 API 单价不等于它们为这些具体请求结算的金额。

## 对后续实现的事实性约束

- 当前 `$40` 仅作三个基础美元月标价的基准。后续若要对比个人真实套餐支出，应允许用户自行填实际套餐和结账金额，并支持税费/处理费；不能暗示 `$40` 是所有人的固定账单。
- “API 等值”应继续标作估算；与真实订阅花费、套餐额度/折扣分开展示。对模型名、提供方或有效单价不确定的记录，不应静默套用可能不同 provider 的同名价格。
- 更可靠的核验记录需要保存价格来源、价格更新时间、模型/提供方与采用的价格档位。要精算，还需从请求记录获得端点/地区、上下文大小、时间/处理档位及工具费用；当前仓库的数据不具备这些字段。

## 2026-09-26 复核与落地

再次打开三家套餐的官方现行页面后，基础美元月标价仍为 OpenCode Go $10、ChatGPT Plus $20、Command Code GOAT $10；因此 $40 可以继续作为**默认标价参照**。GOAT 页面仍注明另加处理费，ChatGPT Plus 页面说明 API 另计；OpenAI 另说明订阅价格会按地区/购买渠道显示本地价格。以上不构成用户实际账单的核验。

模型定价页面也已回看官方入口（OpenAI、Anthropic、Google、DeepSeek、Alibaba、xAI、Kimi、Z.AI、MiniMax、Mistral）。这些页面确实是模型提供方的一手来源；但本仓库仍从 LiteLLM 聚合目录按模型估价，**并未逐行把厂商官网价格同步进应用**。官方页的价格档位可能取决于区域、上下文长度、缓存、处理模式和工具费用，直接硬编码单一费率会制造精确但错误的结果。

针对“固定 $40 不一定等于个人实际月费”的问题，桌面界面现在允许把比较基准改成自己的美元月费，并可恢复为 $40 默认值；桌面 GUI 与 TUI 共用 `%USERPROFILE%\.tui-token-monitor\settings.json` 中的本机设置。API 等值的来源与估算方式没有因此改变。

## 官方来源索引

- OpenCode Go：[计划页](https://dev.opencode.ai/go)、[官方 changelog](https://dev.opencode.ai/changelog)
- ChatGPT Plus：[OpenAI Help Center](https://help.openai.com/en/articles/6950777-what-is-chatgpt-plus)、[多币种计费](https://help.openai.com/en/articles/10421635-multicurrency-billing)
- Command Code GOAT：[价格页](https://commandcode.ai/pricing)、[计划说明](https://commandcode.ai/docs/plans/goat)、[服务条款](https://commandcode.ai/terms)
- API 价目：[OpenAI](https://developers.openai.com/api/docs/pricing)、[Anthropic](https://platform.claude.com/docs/en/about-claude/pricing)、[Gemini API](https://ai.google.dev/gemini-api/docs/pricing)、[Google Cloud Gemini Enterprise Agent Platform](https://cloud.google.com/gemini-enterprise-agent-platform/generative-ai/pricing)、[DeepSeek](https://api-docs.deepseek.com/quick_start/pricing/)、[Alibaba Cloud Model Studio](https://www.alibabacloud.com/help/en/model-studio/model-pricing)、[xAI](https://docs.x.ai/developers/pricing)、[Kimi](https://platform.kimi.com/docs/pricing/chat)、[Z.AI](https://docs.z.ai/guides/overview/pricing)、[MiniMax](https://platform.minimax.io/subscribe/token-plan?tab=api-enterprise)、[Mistral](https://mistral.ai/pricing/api/)
- 仓库选择的第三方价格数据集（不属于模型提供方的官方价目）：[LiteLLM model price catalog](https://github.com/BerriAI/litellm/blob/main/model_prices_and_context_window.json)
