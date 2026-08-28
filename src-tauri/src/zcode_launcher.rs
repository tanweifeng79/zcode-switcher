//! ZCode 快捷方式扫描与「无感切换增强」（给快捷方式加 --remote-debugging-port=9229）。
//!
//! 官方仓库缺失本模块源码（lib.rs 引用了它但文件从未提交），这里按前端调用契约重写：
//!   - `zcode_launcher_scan`    -> `ShortcutInfo[]`（path / target / arguments / has_flag）
//!   - `zcode_launcher_enable`  -> `(modified, already, total)`
//!   - `zcode_launcher_disable` -> 恢复的快捷方式数量
//!   - `restart.rs` 用 `find_preferred_shortcut()` 优先按快捷方式拉起 ZCode（保留
//!     `--remote-debugging-port=9229` 等参数，无感切换依赖该端口）。
//!
//! Windows 通过 PowerShell 的 WScript.Shell COM 读写 .lnk（免新增依赖）；
//! macOS 的 .lnk 不存在，相关能力返回空值（前端在 macOS 上走别的分支）。

use serde::{Deserialize, Serialize};

use crate::profile::AppError;

type R<T> = std::result::Result<T, AppError>;

pub const REMOTE_DEBUG_FLAG: &str = "--remote-debugging-port=9229";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutInfo {
    pub path: String,
    pub target: String,
    pub arguments: String,
    pub has_flag: bool,
}

/// 扫描桌面 / 开始菜单里指向 ZCode.exe 的快捷方式。
pub fn scan_zcode_shortcuts() -> R<Vec<ShortcutInfo>> {
    scan_impl()
}

/// 给所有 ZCode 快捷方式加 `--remote-debugging-port=9229`。
/// 返回 (本次修改数, 之前已开启数, 找到的总数)。
pub fn enable_remote_debug() -> R<(usize, usize, usize)> {
    enable_impl()
}

/// 移除所有 ZCode 快捷方式上的 `--remote-debugging-port`，返回恢复数量。
pub fn disable_remote_debug() -> R<usize> {
    disable_impl()
}

/// 重启 ZCode 时优先使用的增强启动入口：带 `--remote-debugging-port` 的快捷方式优先。
pub fn find_preferred_shortcut() -> Option<ShortcutInfo> {
    find_preferred_impl().map(|mut sc| {
        sc.has_flag |= has_remote_debug_arg(&sc.arguments);
        sc
    })
}

fn has_remote_debug_arg(arguments: &str) -> bool {
    arguments.split_whitespace().any(|tok| {
        tok == REMOTE_DEBUG_FLAG || tok.starts_with("--remote-debugging-port=")
    })
}

// --------------------------------------------------------------------------- //
// Windows 实现：PowerShell + WScript.Shell COM
// --------------------------------------------------------------------------- //

#[cfg(windows)]
const SCAN_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$sh = New-Object -ComObject WScript.Shell
$dirs = @(
  [Environment]::GetFolderPath('Desktop'),
  [Environment]::GetFolderPath('CommonDesktopDirectory'),
  [Environment]::GetFolderPath('StartMenu'),
  [Environment]::GetFolderPath('CommonStartMenu')
) | Where-Object { $_ }
$out = New-Object System.Collections.Generic.List[object]
Get-ChildItem -Path $dirs -Filter *.lnk -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
  $p = $_.FullName
  $lnk = $sh.CreateShortcut($p)
  if ($lnk -and $lnk.TargetPath -and $lnk.TargetPath.ToLower().EndsWith('zcode.exe')) {
    $has = $lnk.Arguments -match '--remote-debugging-port'
    $out.Add([PSCustomObject]@{ path = $p; target = $lnk.TargetPath; arguments = $lnk.Arguments; has_flag = [bool]$has })
  }
}
if ($out.Count -eq 0) { '[]' } else { ConvertTo-Json -InputObject $out.ToArray() -Compress }
"#;

#[cfg(windows)]
const ENABLE_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$sh = New-Object -ComObject WScript.Shell
$dirs = @(
  [Environment]::GetFolderPath('Desktop'),
  [Environment]::GetFolderPath('CommonDesktopDirectory'),
  [Environment]::GetFolderPath('StartMenu'),
  [Environment]::GetFolderPath('CommonStartMenu')
) | Where-Object { $_ }
$modified = 0; $already = 0; $total = 0
Get-ChildItem -Path $dirs -Filter *.lnk -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
  $p = $_.FullName
  $lnk = $sh.CreateShortcut($p)
  if ($lnk -and $lnk.TargetPath -and $lnk.TargetPath.ToLower().EndsWith('zcode.exe')) {
    $total++
    if ($lnk.Arguments -match '--remote-debugging-port') { $already++ }
    else {
      $lnk.Arguments = ($lnk.Arguments + ' __REMOTE_DEBUG_FLAG__').Trim()
      $lnk.Save()
      $modified++
    }
  }
}
ConvertTo-Json -InputObject @{ modified = $modified; already = $already; total = $total } -Compress
"#;

#[cfg(windows)]
const DISABLE_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$sh = New-Object -ComObject WScript.Shell
$dirs = @(
  [Environment]::GetFolderPath('Desktop'),
  [Environment]::GetFolderPath('CommonDesktopDirectory'),
  [Environment]::GetFolderPath('StartMenu'),
  [Environment]::GetFolderPath('CommonStartMenu')
) | Where-Object { $_ }
$restored = 0
Get-ChildItem -Path $dirs -Filter *.lnk -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
  $p = $_.FullName
  $lnk = $sh.CreateShortcut($p)
  if ($lnk -and $lnk.TargetPath -and $lnk.TargetPath.ToLower().EndsWith('zcode.exe') -and $lnk.Arguments -match '--remote-debugging-port') {
    $lnk.Arguments = ($lnk.Arguments -replace '--remote-debugging-port(=\d+)?', '').Trim()
    $lnk.Save()
    $restored++
  }
}
Write-Output $restored
"#;

#[cfg(windows)]
fn run_ps_script(script: &str) -> R<String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|e| AppError::Msg(format!("调用 PowerShell 失败：{}", e)))?;
    if !output.status.success() {
        return Err(AppError::Msg(format!(
            "PowerShell 执行失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(windows)]
fn scan_impl() -> R<Vec<ShortcutInfo>> {
    let text = run_ps_script(SCAN_SCRIPT)?;
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| AppError::Msg(format!("解析扫描结果失败：{}", e)))?;
    Ok(match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .filter_map(|item| serde_json::from_value(item).ok())
            .collect(),
        item @ serde_json::Value::Object(_) => serde_json::from_value(item)
            .ok()
            .into_iter()
            .collect(),
        _ => Vec::new(),
    })
}

#[cfg(windows)]
fn enable_impl() -> R<(usize, usize, usize)> {
    let script = ENABLE_SCRIPT.replace("__REMOTE_DEBUG_FLAG__", REMOTE_DEBUG_FLAG);
    let text = run_ps_script(&script)?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| AppError::Msg(format!("解析结果失败：{}", e)))?;
    let get = |key: &str| value.get(key).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    Ok((get("modified"), get("already"), get("total")))
}

#[cfg(windows)]
fn disable_impl() -> R<usize> {
    let text = run_ps_script(DISABLE_SCRIPT)?;
    text.parse::<usize>()
        .map_err(|e| AppError::Msg(format!("解析结果失败：{}", e)))
}

#[cfg(windows)]
fn find_preferred_impl() -> Option<ShortcutInfo> {
    let shortcuts = scan_zcode_shortcuts().ok()?;
    shortcuts
        .iter()
        .find(|s| s.has_flag)
        .cloned()
        .or_else(|| shortcuts.into_iter().next())
}

// --------------------------------------------------------------------------- //
// 非 Windows：.lnk 不存在，返回空值（前端在 macOS 上有独立分支，不依赖这些命令）
// --------------------------------------------------------------------------- //

#[cfg(not(windows))]
fn scan_impl() -> R<Vec<ShortcutInfo>> {
    Ok(Vec::new())
}

#[cfg(not(windows))]
fn enable_impl() -> R<(usize, usize, usize)> {
    Ok((0, 0, 0))
}

#[cfg(not(windows))]
fn disable_impl() -> R<usize> {
    Ok(0)
}

#[cfg(not(windows))]
fn find_preferred_impl() -> Option<ShortcutInfo> {
    None
}

