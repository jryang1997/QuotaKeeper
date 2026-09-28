# Windows GUI 的 Apple 风格设计研究

核验日期：2026-09-26。目标是把 Apple 的界面原则转译到现有 Windows 桌面应用，而不是逐像素复制苹果平台控件。

## 现状

- GUI 是 React 19 + Vite，使用 Meta 开源的 Astryx 0.6.3；桌面壳为 Wry 0.53 + Tao，Windows 内容运行在 WebView2 中（见 [`gui/frontend/package.json`](../../gui/frontend/package.json)、[`gui/frontend/package-lock.json`](../../gui/frontend/package-lock.json)、[`gui/Cargo.toml`](../../gui/Cargo.toml)）。锁定的 `react` 和 `react-dom` 是 19.3.0。
- 窗口初始尺寸为 1180×760，最小尺寸被设为 860×560；CSS 在 900px 以下仍保留 128px 侧栏，卡片只做部分变列，没有手机宽度的完整导航和纵向阅读模式（见 [`gui/src/main.rs`](../../gui/src/main.rs)、[`gui/frontend/src/app.css`](../../gui/frontend/src/app.css)）。因此当前原生窗口下限挡住了目标中的窄屏模式。
- 侧栏已用了半透明和 `backdrop-filter`，但目前是和内容并排的常驻栏；内容并不会从导航下方滚过。若仅增加模糊强度，不会自动形成 Apple 材质的层次感。

## 核心结论与实现建议

### 1. 先让窄窗布局真正成立

Apple 的布局指导强调窗口缩放时保持布局稳定，并在完整布局确实放不下时才切换紧凑模式；对于复杂导航，可让侧栏和标签栏随宽度互换。[Apple Layout HIG](https://developer.apple.com/design/human-interface-guidelines/layout) 和 [Apple Windows HIG](https://developer.apple.com/design/human-interface-guidelines/windows)

建议把应用布局分为宽窗和紧凑窗两种状态：宽窗保留侧栏；约 600px 以下收起侧栏，改成清楚标注的底部主导航；页面内容改为单列纵向阅读，统计卡、趋势、维度列表和热力图都能在窄窗完整显示，并给滚动内容留出导航栏空间。将 Tao 最小宽度降到足以容纳窄屏布局的范围（可先以 360–400 CSS px 的设备宽度做验收），同时检查 resize 过程中不会突然跳版。导航项目数量有限时，使用常驻的三项以内主导航，避免藏在“更多”里；Apple 的标签栏指导也强调它用于切换顶层区域、应保持可见且控制数量。[Apple Tab Bars HIG](https://developer.apple.com/design/human-interface-guidelines/tab-bars)

### 2. 做“玻璃导航”，不要把整页都做成玻璃

Apple 将 Liquid Glass 定位为浮在内容之上的**功能层**，适合导航和关键控件；正文/数据卡属于内容层，应使用普通表面。应少量使用玻璃，并优先选能保持文字可读的较实材质；高透明的 clear 只适合视觉丰富的背景。[Apple Materials HIG](https://developer.apple.com/design/human-interface-guidelines/materials)

紧凑模式可让底部导航成为单独的悬浮材质层，页面在其后滚动；只在这一处做模糊、饱和度、轻微描边和阴影。统计卡继续用 Astryx 的语义表面色，避免整屏“磨砂卡片”。玻璃本身要有可靠的高不透明度背景回退，并检查浅/深主题及对比度。Apple 所说的真 Liquid Glass 还包括实时折射/聚光的 lensing、随背景适应以及专门的光学效果；CSS `backdrop-filter` 只能做近似的模糊与饱和处理，不应宣称实现了苹果原生 Liquid Glass。[Apple WWDC25: Meet Liquid Glass](https://developer.apple.com/videos/play/wwdc2025/219/)；[CSS Filter Effects Level 2](https://drafts.fxtf.org/filter-effects-2/#BackdropFilterProperty)

保留 Windows 系统标题栏和缩放行为，让 Apple 风格集中在应用内容区。WebView2 使用 Edge 的浏览器引擎，官方建议优先使用会自动更新的 Evergreen Runtime；性能指导建议动画优先使用 CSS、减少重型 JavaScript，并保留 GPU 渲染。[Microsoft WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)；[WebView2 Performance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)

### 3. Astryx 是设计系统；Motion 才是可选的动画工具

用户提到的名字是 **Astryx**（不是动画空间库）。它是 Meta 源自内部实践的 React 设计系统，提供组件、主题、tokens 和 StyleX 样式；官方目前标为 Beta，并要求 React 19+。Astryx 支持 CSS transition/keyframes，并有针对 `prefers-reduced-motion` 的组件规范；它不是物理动画引擎。[Astryx 官方仓库](https://github.com/facebook/astryx)；[Astryx 动画与无障碍规范](https://github.com/facebook/astryx/wiki/Component-Authoring-Guide)；[Astryx Accessibility Checklist](https://github.com/facebook/astryx/wiki/Accessibility-Checklist)

仓库现有 Astryx 0.6.3 与 React 19.3.0 满足官方要求；继续沿用 Astryx 负责控件/主题，不需要为了动效替换它。已有组件动画继续用 CSS；如果页面切换、布局重排、可中断弹层或拖动交互需要连贯弹簧，再加免费的 `motion` npm 包并从 `motion/react` 导入。官方安装文档支持 React 18.2 及以上，Vite 不需要特殊配置；全局设置 `<MotionConfig reducedMotion="user">` 能尊重系统减少动态设置，必要时用 `useReducedMotion` 为大幅位移提供淡入替代。[Motion 安装与 React 支持](https://motion.dev/docs/react-installation)；[MotionConfig](https://motion.dev/docs/react-motion-config)；[Motion useReducedMotion](https://motion.dev/docs/react-use-reduced-motion)

注意：官方 `AnimateView` 是单独入口，要求 React 与 React DOM **19.3 或更新版本**；当前锁文件是 19.3.0，满足，但 package.json 的 `^19.0.0` 也允许较早的 19.x，因此使用它前应保证安装锁定到 19.3+。普通 `motion` / `AnimatePresence` 不受这个 19.3 限制。[AnimateView 要求](https://motion.dev/docs/react-animate-view)

### 4. 动效应轻、短、可打断，并响应减少动态设置

Apple HIG 建议动效有目的、简短准确，不要把动画用于每个频繁操作，也不应让用户等待动画结束才能继续；Apple 的 Fluid Interfaces 演讲强调持续反馈、与手势同步、空间连续性和自然的动量。[Apple Motion HIG](https://developer.apple.com/design/human-interface-guidelines/motion)；[WWDC18: Designing Fluid Interfaces](https://developer.apple.com/videos/play/wwdc2018/803/)

落到本应用：刷新数字只在真实数据变化时轻微强调；导航切页做快速、克制的淡化/小距离位移；不要对实时数字持续做滚动计数、呼吸光效或全屏视差。已在 [`gui/frontend/src/app.css`](../../gui/frontend/src/app.css) 加入减少动态的 CSS 回退，并用 Motion 的 `MotionConfig reducedMotion="user"` 关闭系统要求减少的位移动画；页面切换组件也监听偏好变化，改为无位移的短淡入。原生 WebView2 中模拟减少动态后，transition 缩至 0.00001s，页面切换 transform 为 `none`；减少透明偏好也切换成不透明表面。

## 建议实施顺序

1. 先改窗口最小宽度和紧凑断点，完成 360–400px 窄窗的单列阅读与可见主导航。
2. 再统一字体、间距、圆角、表面色和交互态，先把层级和可读性做扎实。
3. 把单一玻璃材质留给浮动主导航/少数关键控件，并验证主题、长内容、键盘焦点与不支持模糊时的回退。
4. 最后只对确有空间关系或需要打断/反向的动画引入 Motion；CSS 继续负责简单 hover/press，统一启用 reduced-motion。
5. 用真实 Windows 窗口缩放检查宽屏、中等窗口、窄屏、最小高度、中文长标签、键盘导航及系统减少动态状态；重点查看玻璃层上的文本对比、滚动遮挡、热力图在窄屏的密度与手动交互命中区域。

## 2026-09-26 实装记录

- Tao 最小窗口已降到 360×560 CSS px；720px 以下收起侧栏，换成三项悬浮底部导航，统计/配额/分析改为单列，滚动区为导航预留空间。
- 半透明模糊只用于桌面侧栏与紧凑模式底部导航；数据卡保持内容表面色，并为不支持模糊和减少透明效果提供实色回退。
- 安装 Motion 13.4.4，只给顶层页面切换添加短淡入/位移动画；Motion 动效单独按需加载，尊重系统“减少动态”偏好。简单悬停/按压反馈仍由 CSS 处理。
- 用临时合成数据在 Edge Chromium 和实际 Wry/WebView2 原生窗口中做了渲染截图检查：最小客户区 360×560（该 Windows 缩放下 WebView CSS 视口读数为 361×561）下逐一查看配额、用量、热力图，默认客户区 1180×760 下检查三页。均无横向溢出；26 周热力图列完整显示，窄屏导航固定在底部，长用量页可纵向滚动。
- 截图检查发现窄屏 CSS 把浮动导航规则写给了不存在的 `.mobile-nav`，导致它被全局规则隐藏；已改成实际使用的 `.primary-nav-mobile`，并同步修正减少透明效果时的回退选择器。修复后截图确认底部导航出现。
- 原生窄屏下打开月费编辑表单复核输入框和操作按钮，编辑行宽度 160px，未发生裁切或横向溢出。
- WebView2 原生窗口中还检查了暗色外观、`prefers-reduced-motion` 与 `prefers-reduced-transparency`；三项偏好均生效，暗色表面和图表层级可读。原生标题栏保留，最小窗口尺寸实际可调整到设定值。动态切换减少动态偏好后再切页，已确认页面只淡入、不移动。
- 界面截图用合成数据，不涉及真实记录；前端构建、Windows Release 构建及此前 Rust/IPC 回归检查通过。ChatGPT 单独复核本轮未能通过内置浏览器发起；本地 Wry/WebView2 渲染验证已完成。
