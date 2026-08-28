//! ZCode 快捷方式扫描与「无感切换增强」（给快捷方式加 --remote-debugging-port=9229）。
//!
//! 官方仓库缺失本模块源码（lib.rs 引用了它但文件从未提交），这里按前端调用契约重写：
//!   - `zcode_launcher_scan`    -> `ShortcutInfo[]`（path / target / arguments / has_flag）
//!   - `zcode_launcher_enable`  -> `(modified, already, total)`
//!   - `zcode_launcher_disable` -> 恢复的快捷方式数量
//!   - `restart.rs` 用 `find_preferred_shortcut()` 优先按快捷方式拉起 ZCode（保留
//!     `--remote-debugging-port=9229` 等参数，无感切换依赖该端口）。
//!
//! Windows 用 IShellLink COM 直接读写 .lnk，避免拉起 powershell.exe 闪黑框；
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
// Windows 实现：IShellLink COM 读写 .lnk（不启动 powershell.exe）
// --------------------------------------------------------------------------- //

#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::path::{Path, PathBuf};

#[cfg(windows)]
struct ComGuard {
    uninit: bool,
}

#[cfg(windows)]
impl ComGuard {
    fn enter() -> Self {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self { uninit: hr.is_ok() }
    }
}

#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninit {
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

#[cfg(windows)]
fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

#[cfg(windows)]
fn shortcut_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = dirs::desktop_dir() {
        dirs.push(d);
    }
    if let Ok(public) = std::env::var("PUBLIC") {
        dirs.push(PathBuf::from(public).join("Desktop"));
    }
    if let Some(roaming) = dirs::data_dir() {
        dirs.push(roaming.join("Microsoft").join("Windows").join("Start Menu"));
    }
    if let Ok(programdata) = std::env::var("PROGRAMDATA") {
        dirs.push(
            PathBuf::from(programdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu"),
        );
    }
    dirs.into_iter().filter(|d| d.is_dir()).collect()
}

#[cfg(windows)]
fn collect_lnk_files(root: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_lnk_files(&path, out, depth + 1);
        } else if path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("lnk"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

#[cfg(windows)]
fn open_shell_link(
    path: &Path,
    write: bool,
) -> Option<(
    windows::Win32::UI::Shell::IShellLinkW,
    windows::Win32::System::Com::IPersistFile,
)> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ, STGM_READWRITE,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        let path_h = HSTRING::from(path.to_string_lossy().as_ref());
        let mode = if write { STGM_READWRITE } else { STGM_READ };
        persist.Load(&path_h, mode).ok()?;
        Some((link, persist))
    }
}

#[cfg(windows)]
fn read_link_target_args(path: &Path) -> Option<(String, String)> {
    let (link, _persist) = open_shell_link(path, false)?;
    unsafe {
        let mut target = [0u16; 1024];
        link.GetPath(&mut target, std::ptr::null_mut(), 0).ok()?;
        let mut args = [0u16; 2048];
        let _ = link.GetArguments(&mut args);
        Some((from_wide(&target), from_wide(&args)))
    }
}

#[cfg(windows)]
fn is_zcode_target(target: &str) -> bool {
    target
        .rsplit(['\\', '/'])
        .next()
        .map(|name| name.eq_ignore_ascii_case("zcode.exe"))
        .unwrap_or(false)
}

#[cfg(windows)]
fn iter_zcode_shortcuts() -> Vec<(PathBuf, String, String)> {
    let mut lnks = Vec::new();
    for dir in shortcut_search_dirs() {
        collect_lnk_files(&dir, &mut lnks, 0);
    }
    lnks.into_iter()
        .filter_map(|path| {
            let (target, arguments) = read_link_target_args(&path)?;
            is_zcode_target(&target).then_some((path, target, arguments))
        })
        .collect()
}

#[cfg(windows)]
fn with_remote_debug_arg(arguments: &str) -> String {
    if has_remote_debug_arg(arguments) {
        arguments.trim().to_string()
    } else {
        format!("{} {}", arguments.trim(), REMOTE_DEBUG_FLAG)
            .trim()
            .to_string()
    }
}

#[cfg(windows)]
fn strip_remote_debug_arg(arguments: &str) -> String {
    arguments
        .split_whitespace()
        .filter(|tok| *tok != REMOTE_DEBUG_FLAG && !tok.starts_with("--remote-debugging-port"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(windows)]
fn save_link_arguments(path: &Path, arguments: &str) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::TRUE;

    let Some((link, persist)) = open_shell_link(path, true) else {
        return false;
    };
    let path_h = HSTRING::from(path.to_string_lossy().as_ref());
    unsafe {
        link.SetArguments(&HSTRING::from(arguments)).is_ok() && persist.Save(&path_h, TRUE).is_ok()
    }
}

#[cfg(windows)]
fn scan_impl() -> R<Vec<ShortcutInfo>> {
    let _com = ComGuard::enter();
    Ok(iter_zcode_shortcuts()
        .into_iter()
        .map(|(path, target, arguments)| ShortcutInfo {
            has_flag: has_remote_debug_arg(&arguments),
            path: path.to_string_lossy().into_owned(),
            target,
            arguments,
        })
        .collect())
}

#[cfg(windows)]
fn enable_impl() -> R<(usize, usize, usize)> {
    let _com = ComGuard::enter();
    let mut modified = 0;
    let mut already = 0;
    let mut total = 0;
    for (path, _target, arguments) in iter_zcode_shortcuts() {
        total += 1;
        if has_remote_debug_arg(&arguments) {
            already += 1;
            continue;
        }
        if save_link_arguments(&path, &with_remote_debug_arg(&arguments)) {
            modified += 1;
        }
    }
    Ok((modified, already, total))
}

#[cfg(windows)]
fn disable_impl() -> R<usize> {
    let _com = ComGuard::enter();
    let mut restored = 0;
    for (path, _target, arguments) in iter_zcode_shortcuts() {
        if !has_remote_debug_arg(&arguments) {
            continue;
        }
        if save_link_arguments(&path, &strip_remote_debug_arg(&arguments)) {
            restored += 1;
        }
    }
    Ok(restored)
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

