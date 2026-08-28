//! 无感切换后的 CDP 刷新（官方仓库缺失本模块源码，这里保留最小实现）。
//!
//! 原版逻辑：当 ZCode 快捷方式带 `--remote-debugging-port=9229` 时，切号完成后
//! 通过 Chrome DevTools Protocol 触发 ZCode 内部 provider/权益刷新，让余额面板
//! 立即更新、聊天会话立即重建。
//!
//! 开源版保留空实现：ZCode 自身会通过 fs.watch 感知 credentials/config 变化并
//! 重建会话，切号核心流程不受影响；仅余额面板可能要等下一次轮询才刷新。

/// 切号完成后的占位刷新入口（profile.rs / proxy.rs 在写完配置后调用）。
pub fn schedule_post_switch_refresh() {
    // 预留：后续可在此检测 127.0.0.1:9229 并通过 CDP
    // （HTTP /json/new 连接 app-server 页面）主动触发刷新。
    // 当前空实现不影响切号结果，只影响余额面板刷新的即时性。
}
