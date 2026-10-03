# 局域网 Standby 看板（手机/平板访问）实施计划（待批准）

调研时间：2026-10-05。设计参照：iPhone Standby（横屏待机、超大数字、轻扫换页）+ apple-design 技能（点按即反馈、scroll-snap 惯性轻扫、弹簧、半透明材质、负字距大标题）。

## 方案概要

GUI 进程内起一个**手写 TcpListener 极简 HTTP 服务**（零新依赖，规避本机 GNU 工具链 raw-dylib 坑），手机/平板浏览器访问 `http://<局域网IP>:8420/` 看实时看板。

- 只有两个只读 GET 端点：`/`（内嵌单文件页面）+ `/api/summary`（JSON 摘要）。无凭证/密钥数据，只有配额百分比、token 量、请求数。
- 监听 0.0.0.0（手机必须），设置页明示「请仅在受信任的局域网使用」；**默认关**，开关打开才监听。
- 端口固定 8420，被占则顺延（最多试到 8429），实际地址显示在设置页 + 一键复制。

## 手机页面（standby.html，单文件内嵌，约 400 行）

- **黑底全屏**，Standby 式布局：横屏 = 网格大卡，竖屏 = 单列滚动。
- **三个视图，左右轻扫切换**（CSS scroll-snap 原生惯性 + rubber-band，Standby 同款交互），底部圆点指示：
  1. **配额页**：每套餐一张卡——SVG 分块环 + 大百分比 + 重置倒计时（沿用应用内环语言）；
  2. **用量页**：今日/本周/本月分段切换（页面内自选时间段），大数字 token + 请求数 + 花费；
  3. **时段页**：24 小时柱状图（今日活跃分布）+ 本月 API 等价价值。
- 数据 30 秒轮询 `/api/summary` 自动刷新；断线显示「等待数据」态。
- 遵循 apple-design：点按即时反馈、负字距大标题、`backdrop-filter` 卡片材质、`prefers-reduced-motion` 降级。

## 后端接线

- `gui/src/lan.rs`（新）：服务线程 + `Arc<RwLock<LanState>>` 共享状态（Snapshot、LedgerReport、goat_usage、deepseek_balance、monthly_value）；主循环每次数据落定时更新（与 cache::save 同点）。
- `settings.rs`：`SettingsFile` 加 `lan_dashboard_enabled`（后端持久化，服务在 Rust 侧）。
- `main.rs`：IPC `setLanDashboard` + 启动时按开关自启；`UserEvent::LanDashboardEnabled(bool)`。
- 局域网 IP：UDP connect 路由表技巧（不发包）取默认出口 IP，拼完整 URL。

## 设置页（桌面常驻区块内一行）

「局域网看板」开关 + 状态行：运行中显示 `http://192.168.x.x:8420/` + 复制按钮；关闭时显示已停用。附一句安全提示。

## 顺序与验证

单笔提交。验证：本机 curl 两个端点 + CDP 以移动视口截图三个视图 + 真机（Jerry 手机）自测。构建全链 + 测试 + clippy 同前例。
