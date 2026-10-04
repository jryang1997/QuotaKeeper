# Liquid Glass 落地计划（2026-10-04）

## 0 结论

- 运行时是 WebView2（Chromium），支持 `backdrop-filter: url(#svgFilter)` → 能做**真折射**，不止毛玻璃。该写法非 CSS 标准（Firefox/Safari 不认），用 `@supports` 包住：支持就折射，不支持回退毛玻璃，零风险。
- 现有基础已有半透明材质（工具栏 blur18 / 侧栏 blur24 / ≤720px 底栏 blur28 / 成就 toast），缺的是液态玻璃多出的三层：折射、镜面 rim、微噪点。
- 结构改动：新增**底部液态玻璃 Dock** 作为主因导航；侧栏保留但可收放；工具栏与弹层统一玻璃令牌。

## 1 技术：液态玻璃 = 毛玻璃 + 三层

| 层 | 做法 | 用在哪 |
| --- | --- | --- |
| 折射 lensing | SVG `feImage`（位移贴图）+ `feDisplacementMap`，`backdrop-filter: blur(…) url(#glass-lens)`。贴图由凸透镜轮廓法线生成：R=X 偏移、G=Y 偏移、128=中性，边缘把背景"掰弯"。静态贴图 useMemo 一次生成，仅元素尺寸变化才重算 | 只给 Dock 一个元素（全应用唯一折射位） |
| 镜面 rim | inset box-shadow 上亮下暗 + 1px 渐变描边（顶边最亮，"光打在材质上沿"） | Dock / 工具栏 / 弹层 |
| 质感 | `saturate(170%)` 保背景色鲜艳；feTurbulence data-URI 微噪点叠层防色带；出现动画 blur+scale 同动（materialize，不是纯 fade） | 同上 |

- 性能：backdrop-filter 只给浮动 chrome（Dock/工具栏/弹层），内容卡保持实底——遵守 Apple「轻玻璃不叠轻玻璃」，大面折射不上。
- 可及性：`prefers-reduced-transparency` → 实底（现有媒体查询同款，沿用）；`reduced-motion` → 纯淡入。
- 主题：dark / ink / light / sand 各调 tint 与 rim 强度（暗色高光弱+边框更透，亮色高光白）。

参考：[kube.io 折射推导](https://kube.io)（Snell 折射→位移贴图完整算法）、[LogRocket 教程 2025-12](https://blog.logrocket.com)、[nikdelvin/liquid-glass](https://github.com)（iOS 26 复刻）。

## 2 部位（哪些地方上玻璃）

1. **底部 Dock（新，质感主角）**：底部居中悬浮胶囊，高约 76px，四页图标+文字，active 滑块用现成 `layoutId + CONTROL_SPRING`；胶囊左端一枚数据状态点（侧栏收起后健康度仍可见）。真液态玻璃全特效。≤720px 的现有移动底栏升级为同款，桌面/窄窗一套组件。
2. **侧栏（保留，可收放）**：工具栏最左加侧栏开关按钮；`Ctrl+B` 快捷键 + 命令面板条目；状态存 gui-preferences（`sidebarHidden`）。收起 = 宽度+内容透明度走 `LAYOUT_SPRING` 弹簧，展开走同一路径（对称）；主区自然铺满，底部 padding 给 Dock 留约 90px。收起后标题/状态点随侧栏藏起（Dock 常驻即身份）。
3. **顶部工具栏**：玻璃参数收进 `:root` 令牌（`--glass-blur / --glass-tint / --glass-rim…`），加顶边亮线，与 Dock 同族不同级（更薄）。
4. **弹层家族**：Ctrl+K 面板、状态 popover、配额 ⋯ 菜单、成就 modal 的 scrim 统一玻璃令牌；scrim 上的面板=轻玻璃+粗阴影。
5. **内容卡不动**：实底。LAN 看板是独立 standby.html，不涉及。

## 3 实施（批准后，一次交付）

- **A 材质系统**：玻璃令牌 + `<GlassSurface>` 组件（折射贴图 useMemo，ResizeObserver 重算）+ `@supports` 回退。
- **B Dock**：`DockNav` 组件（桌面+窄窗一套），滑块弹簧、状态点、按压即时反馈。
- **C 侧栏收放**：开关/快捷键/持久化/弹簧动画。
- **D 统一**：工具栏与弹层换玻璃令牌。
- **E 走查**：四主题 × 中英 × 125% 缩放，CDP 截图自查（不抢前台），Dock 滚动帧率抽查；产出下一轮清单。

风险集中在 A 的折射贴图；B–D 是确定的工作量。前端改完照例 `npm build` + `cargo build`（dist 编进 exe）。
