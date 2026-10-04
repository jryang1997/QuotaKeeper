# 成就二期:图一乐扩充 + 图标路线

日期:2026-10-04。现状:22 项成就,全部由本地账本纯函数推导,档位 bronze/silver/gold/legend,解锁存 localStorage、新解锁弹 toast,图标为 15 个手写 24×24 内联 SVG path(fill=currentColor,tier 色靠 CSS)。

## 设计方向

一期成就本质是"统计报表的里程碑"。二期往图一乐走,补三类一期没有的东西:

1. **人设成就** — 从数据形状读出使用画风(海王/独宠/夜猫子式的梗),不只是"更多"。
2. **交互彩蛋** — 对工具本身的行为当成就(Ctrl+K、晒卡、切语言),用着用着蹦出来。
3. **Meta 成就** — 解锁行为本身的阶梯,全解锁给一个超越 legend 的白金档。

## 新增 19 项(合计 41)

数值线均为首版拍脑袋,装机后按真实账本校准一轮再定稿。

### A. 行为人设(4) — 数据全在现有 LedgerDto
| id | 名字 | 条件 | 档位 |
|---|---|---|---|
| socializer | 海王 / Socializer | 近 30 天 ≥ 6 个项目且单项目均 < 40% | gold |
| one-trick | 独宠 / One trick | 近 30 天单一项目 ≥ 80%(≥ 3 个项目打底) | bronze |
| long-haul | 千轮马拉松 / Long haul | 单会话 ≥ 1,000 条消息 | silver |
| full-house | 六边形战士 / Full house | 6 个来源全部有记录(Codex/Claude/DSH/OMP/OpenCode/ZCode) | legend |

### B. 时段玩梗(3)
| id | 名字 | 条件 | 档位 |
|---|---|---|---|
| round-the-clock | 全天候 / Round the clock | 近 30 天 0–23 每个小时桶都有记录 | gold |
| lunch-grinder | 午休卷王 / Lunch grinder | 12–14 点累计 500 万 token | silver |
| after-hours | 下班冲刺 / After hours | 18–22 点累计 2,000 万 token | silver |

### C. 模型与思维(3)
| id | 名字 | 条件 | 档位 |
|---|---|---|---|
| deep-thinker | 想得多 / Deep thinker | 本月 reasoning ≥ output(reasoning ≥ 100 万样本) | bronze |
| eloquent | 出口成章 / Eloquent | 累计 output ≥ 1,000 万 | silver |
| sonic | 超音速 / Sonic | 实测某模型 ≥ 300 tok/s(与「疾速 120」成阶梯) | legend |

### D. 钱的爽点(2) — monthly_value 已在 PushDto,壳层传参即可,ipc.rs 不动
| id | 名字 | 条件 | 档位 |
|---|---|---|---|
| break-even | 回本 / Break even | 本月 API 等价 ≥ 套餐基准价(multiple ≥ 1) | bronze |
| x10-value | 十倍血赚 / 10x value | list_price_multiple ≥ 10 | gold |

### E. 交互彩蛋(4) — 需新增 fun-counters.js(localStorage 事件计数),CommandPalette / SharePage / 语言切换 / 悬浮窗置顶各挂一行
| id | 名字 | 条件 | 档位 |
|---|---|---|---|
| keyboard-flow | 键盘流 / Keyboard flow | Ctrl+K 唤起快速跳转 50 次 | bronze |
| card-sharer | 晒卡狂魔 / Card sharer | 分享卡导出 5 次 | bronze |
| polyglot | 国际化 / Polyglot | 中英切换 10 次 | bronze |
| desk-anchor | 镇桌之宝 / Desk anchor | 开启悬浮窗置顶 | bronze |

### F. Meta(3)
| id | 名字 | 条件 | 档位 |
|---|---|---|---|
| collector | 收藏家 / Collector | 解锁 10 项 | silver |
| completionist | 图鉴大师 / Completionist | 解锁 20 项 | gold |
| platinum | 白金殿堂 / Platinum | 全解锁(新 platinum 档,居 legend 之上) | platinum |

## 图标:继续手写 SVG,不用 GPT 画图

现状 15 个 glyph 全是同一网格(24×24、主形 fill=currentColor、辅线 1.6–1.8 stroke)的手写 path。推荐延续:

- **主题系统零破坏**:暗色/亮色、tier 染色全靠 currentColor + CSS 变量;位图直接废掉这套。
- **风格可控**:图像模型批量出图很难锁死笔画粗细/负空间,凑一面墙像"图标超市";手写 path 一套笔意到底,贴 Command Code 风。
- **零成本**:无资产文件、不撑 include_dir 体积、任意缩放。"GPT 生成"若指文字模型直接写 SVG path——那就是现在的方式,新图案(三叉戟、六边形、日不落、皇冠×闪电、奖杯底座等)继续由我手写补齐。
- 折中路线(备选):从 Lucide/Tabler 挑 24×24 stroke path 拼装,网格统一但与现有 fill 风混搭,显"默认观感",不首选。
- 加分项:legend/platinum 的 glyph 外加**同心环徽章底**(纯 SVG/CSS,呼应总览页点阵大数字的同心环母题),档位越高环越多。

## 落地改动面

- `achievements.js`:+19 def;`ledgerFacts` 扩字段(reasoning/output、项目占比、单会话消息、小时覆盖、来源数);`evaluateAchievements(ledger, extras)` 接收 monthly_value + 计数器 + 解锁数。
- `fun-counters.js`(新):localStorage 事件计数;四处调用点各一行。
- `Achievements.jsx` / `app.css`:platinum 档色 + 同心环徽章底;41 项的 modal 滚动确认。
- 测试:`achievements.test.mjs` 补新 case。
- dist 随 exe 打包,改完 `cargo build` 重建。
- 分批提交:feat(achievements)、build(frontend) 各一笔(英文 type + 中文正文)。
