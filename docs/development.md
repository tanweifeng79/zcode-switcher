# 开发说明

## 安装依赖

```powershell
npm install
```

## 启动开发模式

```powershell
npm run tauri dev
```

## 构建前端

```powershell
npm run build
```

## 构建 Windows 安装包

```powershell
npm run tauri build
```

## 构建 macOS 安装包

macOS 包必须在 Mac 或 GitHub macOS Runner 中构建（Tauri 不支持交叉编译），
对应 `release.yml` 的 `macos-latest` matrix：

```bash
npm ci
npm run tauri build -- \
  --target universal-apple-darwin \
  --bundles dmg
```

构建产物位于 `src-tauri/target/universal-apple-darwin/release/bundle/dmg/`，
同时覆盖 Apple Silicon (arm64) 与 Intel (x86_64)。

## 本地目录约定

- 源码修改、依赖安装、构建和测试应在开发目录进行。
- GitHub 发布目录只保留准备提交的干净文件，不放 `node_modules`、`dist`、`src-tauri/target`、安装包、日志和临时截图。
