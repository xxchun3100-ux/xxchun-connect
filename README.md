# xxchun Connect

把这台电脑上的 Claude、Codex 接到 [xxchun](https://xxchun.com) 的小工具：打开 → 粘贴密钥 → 点「接到 Claude」或「接到 Codex」→ 完全退出那个程序再打开。右上角「恢复原配置」可以原样撤回。

- Windows 10 / 11（x64）：免安装 zip，解压后双击 `xxchun Connect.exe`。
- macOS（Apple Silicon）：zip，解压后打开 `xxchun Connect.app`。
- 下载地址和密钥都在 xxchun.com 的「我的 Token › 接入教程」。

它只连 `api.xxchun.com`，只改当前用户的客户端配置，改之前先备份到用户目录的 `.xxchun-connect`。

## 来历与许可

xxchun Connect 是开源项目 [SuperGPT Connect](https://github.com/superpilot69/supergpt-connect)（MIT）的分支，后者的配置引擎来自 CC Switch（MIT）。见 `NOTICE.md`、`UPSTREAM.md`、`LICENSE`、`licenses/`；本分支改了什么见 `XXCHUN-CHANGES.md`。`VERIFICATION.md` 和 `COMPATIBILITY.md` 是上游 v0.1.6 的记录，原样保留，不代表本分支已验证。

## 开发与发布

```
pnpm install --frozen-lockfile
pnpm check && pnpm test:ui && pnpm test:brand
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked
```

推一个 `v0.1.0` 这样的 tag，`.github/workflows/release.yml` 会在 GitHub Actions 上原生构建两个 zip（`xxchun-Connect-<版本>-Windows-x64-portable.zip`、`xxchun-Connect-<版本>-macOS-arm64.zip`），连同 `SHA256SUMS.txt` 挂到 Release。
