# 发布说明

发布流程、更新日志、施工日志和公告格式的统一说明见 `docs/publish-and-format-guide.md`。

## 应用内更新

- 软件会从 GitHub Releases 检查更新：
  `https://github.com/git-l-1031/zcode-switcher/releases/latest/download/latest.json`
- 更新验签公钥保存在 `src-tauri/tauri.conf.json`。
- 更新签名私钥不能提交到仓库。
- 请把更新签名私钥保存在本机安全位置，并把私钥全文写入 GitHub Actions Secrets。
- Release 中必须包含 `latest.json`。如果这个文件缺失，软件内检测更新会失败。
- GitHub Actions 已显式开启 `includeUpdaterJson`，并优先使用 Windows NSIS 安装包生成更新清单。

## macOS DMG

- macOS 与 Windows 共用本仓库源码（`src-tauri` 内通过 `#[cfg(target_os = "macos")]` 区分平台实现）。
- 构建流水线在 `.github/workflows/release.yml` 的 `macos-latest` matrix，
  使用 `--target universal-apple-darwin` 同时产出 Apple Silicon (arm64) 与
  Intel (x86_64) 的 `.app` / `.dmg`。
- 本地构建 macOS 包必须在 Mac 或 GitHub macOS Runner 上执行：
  `npm run tauri build -- --target universal-apple-darwin --bundles dmg`。
- 正式公开发布前必须补 Apple Developer ID 签名与公证；当前流水线先用 ad-hoc 签名保证应用包完整性，仍按内测包发布。
- macOS 自动更新接入时，必须合并 `darwin-aarch64` 与 Windows 平台信息，不能用单平台
  `latest.json` 覆盖正式仓库中的更新清单。

## GitHub Secrets

发布前需要在仓库的 Actions Secrets 中配置：

- `TAURI_SIGNING_PRIVATE_KEY`：更新签名私钥全文
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：生成更新签名密钥时使用的密码

## 发布版本

创建并推送版本标签：

```powershell
git tag v1.1.5
git push origin v1.1.5
```

GitHub Actions 会自动创建 Release，并上传安装包和应用内更新所需的签名文件。Release 说明会优先读取 `docs/changelog.md` 中对应版本的小节，应用内检测更新弹窗也会显示这段内容。

## 检测更新失败排查

- 访问 `https://github.com/git-l-1031/zcode-switcher/releases/latest/download/latest.json`。
- 如果返回 `404`，说明当前最新 Release 没有上传更新清单，需要重新发布带 `latest.json` 的版本。
- 如果能打开 JSON，但软件提示验签失败，检查 GitHub Secrets 中的签名私钥是否和 `src-tauri/tauri.conf.json` 中的公钥匹配。
- 如果能打开 JSON 且验签正常，但软件提示已是最新版本，说明当前安装版本不低于 Release 版本。
