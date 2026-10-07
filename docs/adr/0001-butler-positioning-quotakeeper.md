# 0001 - 定位"AI 编程订阅管家",以 QuotaKeeper 之名发布 1.0

项目此前三线并行(自用工具 / 开源产品 / 技术玩具),功能与形态发散。2026-10-07 经全局 grill 定调:定位为**管家**——用量、余额、到期提醒一屏看清,对账心智让给 ccusage,菜单栏让给 CodexBar;**GUI 为主,TUI 彻底冻结**(坏了才修,发布页不提);**Windows 优先**,其他平台代码在但不承诺;成就、热力图、主题套件、Standby 列为**实验功能**,只在 README 与发布帖标注,产品界面不分区;发布前补 GLM/MiniMax/Kimi 三张主流卡,补完全面冻结;**改名 QuotaKeeper**(quota-butler / tokenbutler 已被占用,后者还有加密货币联想;数据目录不写自动迁移,发布日手动改名);私有仓**发布日当天统一转公开**,不做裸奔期。

## Considered Options

- 定位"对账工具"或"用量仪表盘":分别与 ccusage、CodexBar/abtop 正面撞车,弃。
- 双形态并重:两套界面的"半成品感"是项目发散的来源之一,弃。
- 承诺三平台:托盘/通知/悬浮窗均为 Windows 专属,承诺会被三平台 issue 淹死,弃。
- 首启自动迁移数据目录:老用户极少,发布日手动改名 5 秒,弃。
- PlanButler(当时推荐名):检索干净,但 QuotaKeeper 语义更准;同名 0 星 Codex 配额工具(gigitux/quotakeeper)不构成实质冲突。
