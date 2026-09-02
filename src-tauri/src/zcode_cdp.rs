//! 无感切换后的 CDP 刷新。
//!
//! ZCode 带 `--remote-debugging-port=9229` 启动时（无感切换增强开启），切号写完
//! credentials/config 后通过 Chrome DevTools Protocol 重载 ZCode 渲染页面：
//! 余额面板立即按新账号重新拉取，避免继续显示切换前的快照；聊天会话由主进程
//! 管理（token 缓存在主进程内存），渲染层重载不影响已保存的会话。
//!
//! ZCode 未带调试参数时 9229 端口不可达，静默跳过——由 ZCode 自身的轮询兜底刷新。

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

const CDP_HOST: &str = "127.0.0.1";
const CDP_PORT: u16 = 9229;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// 切号写盘 → ZCode fs.watch 感知 → 再 reload，留一点缓冲
const RELOAD_DELAY: Duration = Duration::from_millis(800);

/// 切号完成后的刷新入口（profile.rs 在写完配置后调用，fire-and-forget）。
pub fn schedule_post_switch_refresh() {
    tauri::async_runtime::spawn(async {
        if let Err(e) = reload_zcode_pages().await {
            eprintln!("[zcode_cdp] 切号后刷新 ZCode 页面失败：{e}");
        }
    });
}

/// 通过 CDP 找到 ZCode 的页面 target 并逐个 reload。
async fn reload_zcode_pages() -> Result<(), String> {
    tokio::time::sleep(RELOAD_DELAY).await;

    let client = reqwest::Client::builder()
        .timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|e| format!("HTTP 客户端创建失败：{e}"))?;

    let targets: Vec<serde_json::Value> = client
        .get(format!("http://{CDP_HOST}:{CDP_PORT}/json/list"))
        .send()
        .await
        .map_err(|e| {
            format!("CDP 端口 {CDP_PORT} 不可达（ZCode 可能未带 --remote-debugging-port=9229 启动）：{e}")
        })?
        .json()
        .await
        .map_err(|e| format!("解析 /json/list 响应失败：{e}"))?;

    // 只刷新真实页面：排除 DevTools 与扩展页，避免误伤
    let ws_urls: Vec<String> = targets
        .iter()
        .filter(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
        .filter(|t| {
            let url = t.get("url").and_then(|v| v.as_str()).unwrap_or("");
            !url.starts_with("devtools://") && !url.starts_with("chrome-extension://")
        })
        .filter_map(|t| {
            t.get("webSocketDebuggerUrl")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();
    if ws_urls.is_empty() {
        return Err("CDP 未发现可刷新的 ZCode 页面".into());
    }

    let mut failures = Vec::new();
    for ws_url in ws_urls {
        if let Err(e) = reload_page(&ws_url).await {
            failures.push(format!("{ws_url}: {e}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("；"))
    }
}

/// 连接单个页面 target，执行 location.reload()。
/// reload 生效后 CDP 连接通常会被立刻断开，断开同样视为已生效。
async fn reload_page(ws_url: &str) -> Result<(), String> {
    let (ws, _) = tokio_tungstenite::connect_async(ws_url)
        .await
        .map_err(|e| format!("WebSocket 连接失败：{e}"))?;
    let (mut sink, mut stream) = ws.split();

    let eval = json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": {
            "expression": "location.reload()",
            "userGesture": true,
            "awaitPromise": false,
            "returnByValue": false
        }
    });
    sink.send(Message::Text(eval.to_string()))
        .await
        .map_err(|e| format!("发送 evaluate 失败：{e}"))?;

    let deadline = tokio::time::Instant::now() + CONNECT_TIMEOUT;
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => break,
            msg = stream.next() => match msg {
                Some(Ok(Message::Text(text))) => {
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                        continue;
                    };
                    // 只关心 id=1 的 evaluate 响应，忽略事件推送
                    if v.get("id").and_then(|i| i.as_u64()) != Some(1) {
                        continue;
                    }
                    if let Some(err) = v.get("error") {
                        return Err(format!("evaluate 失败：{err}"));
                    }
                    break;
                }
                // 连接断开 / 超时：reload 已触发，视为成功
                Some(Err(_)) | None => break,
                Some(Ok(_)) => continue,
            }
        }
    }
    let _ = sink.close().await;
    Ok(())
}
