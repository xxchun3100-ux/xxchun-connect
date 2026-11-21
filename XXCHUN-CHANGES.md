# xxchun Connect：相对上游改了什么

上游：https://github.com/superpilot69/supergpt-connect ，tag `v0.1.6`（提交 65c260e）。本分支版本 0.1.0。改动没有提交，都在工作区里。

## 没有在这台机器上验证的

这台机器没有 Rust / MSVC，**`cargo test` 和两个安装包一次都没有构建过**。Rust 一侧只做了机械改名（见下），没有改逻辑；能不能编译、上游的 Rust 测试过不过，要等 GitHub Actions 跑。能跑的都跑了：`pnpm check`（tsc）、`pnpm test:ui`（12 个）、`pnpm test:brand`（3 个）、`pnpm build:web`。

已知风险：
- `ci/build.yml`（上游的样例）里有 `cargo fmt --check`；改名后有的行变短，rustfmt 可能要求重排。`release.yml` 里没有放这一步。
- 图标的 `.ico` / `.icns` 是手写的容器（里面是 PNG），没有在真 Windows 资源编译器和 macOS 上验证。
- 「粘贴密钥」按钮用网页的剪贴板接口，系统不让读时会改成提示按 Ctrl+V / ⌘V；在真应用里哪种情况没试过。
- Windows 包需要 WebView2，没有代码签名；Mac 包是 ad-hoc 签名、没有公证，首次打开会被系统拦。这些是上游原样的情况。

## 机械改名（`output/token-build-20261118/connect-rebrand.cjs`，在商城仓库里）

对除 `LICENSE`、`UPSTREAM.md`、`CHANGELOG.md`、`licenses/`、`pnpm-lock.yaml` 以外的文本文件做了同一组替换：

| 上游 | 本分支 |
|---|---|
| `SuperGPT Connect` / `SuperGPT-Connect` | `xxchun Connect` / `xxchun-Connect` |
| `supergpt-connect`、`supergpt_connect`、`SUPERGPT_CONNECT` | `xxchun-connect`、`xxchun_connect`、`XXCHUN_CONNECT` |
| `dev.supergpt.connect` | `com.xxchun.connect` |
| `api.supergpt.dev`、`console.supergpt.dev` | `api.xxchun.com`、`xxchun.com` |
| `supergpt-models.json` | `xxchun-connect-models.json` |
| `supergpt.connect_capabilities`、`_supergpt_connect` | `xxchun.connect_capabilities`、`_xxchun_connect` |

指向上游仓库的链接（`superpilot69/supergpt-connect`）保持原样。受影响的文件：`.gitignore`、`ci/build.yml`、`index.html`、`package.json`、`scripts/*`、`src-tauri/Cargo.toml`、`Cargo.lock`（只有根包名和版本）、`src-tauri/tauri.conf.json`、`src-tauri/src/{adapter,capabilities,codex_context,compatibility,desktop_config,lib,main,models,storage}.rs`、`src-tauri/examples/*.rs`、`src-tauri/tests/*.rs`、`src/styles.css`。

这带来的行为变化：恢复目录是 `~/.xxchun-connect`；写进 Codex 的 provider 叫 `xxchun-connect`，模型清单文件叫 `xxchun-connect-models.json`；写进 Claude 桌面版的配置名是「xxchun Connect」；测试用的环境变量是 `XXCHUN_CONNECT_TEST_HOME`。

## 逐个文件

- `src/App.tsx`：整个重写。上游是四步向导（选中转站 → 选客户端 → 填密钥 → 勾模型 → 导入）。现在是一屏：「粘贴密钥」大按钮 + 一个密钥框 → 两个大按钮「接到 Claude」「接到 Codex」（桌面版）→ 「更多」里是 Claude Code、VS Code、终端里的 Codex → 结果页「好了，完全退出 … 再打开」。点按钮时依次调用上游原有的 `discover_models` → `preview_config` → `import_config`，模型由应用自己选（见 `src/quick.ts`）。没有中转站列表，没有「添加站点」，没有模型勾选和高级选项。右上角「恢复原配置」和关于框保留；关于框写明来自 SuperGPT Connect 和 CC Switch。
- `src/quick.ts`（新）：`looksLikeKey`（剪贴板里的像不像密钥）、`pickModels`（密钥能用且该客户端能用的全部模型；Claude 默认选 Sonnet，没有就选第一个；Codex 选第一个）。
- `src/stations.ts`：只剩一个站点（xxchun，`https://api.xxchun.com`）和 `stationAddress`。上游的自定义站点、本机保存的站点偏好都删了。
- `src/StationEditor.tsx`、`src/SetupOptionsPanel.tsx`：删除（新界面不用）。`src/setup-options.ts`、`src/selection.ts`、`src/clients.ts`、`src/RestorePanel.tsx`、`src/native.ts`、`src/main.tsx` 没动。
- `src/styles.css`：末尾加了 `.xc-*` 样式；上游样式没删（有些已经用不到）。
- `public/supergpt.png` 删除，`public/xxchun.png` 新增；`src-tauri/icons/{32x32,128x128,128x128@2x}.png`、`icon.ico`、`icon.icns` 换成商城的标志（由 `output/token-build-20261118/connect-icons.mjs` 从 `frontend/public/favicon.svg` 生成）。`src-tauri/icons/` 里其余没被配置引用的图（`icon.png`、`64x64.png`、`Square*`、`android/`、`ios/`、`source.svg`）还是上游的，没换。
- `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`：版本 0.1.0；`package.json` 加了 `test:brand`。
- `scripts/build-windows-portable.mjs`、`scripts/build-macos.mjs`：压缩包名 `xxchun-Connect-<版本>-Windows-x64-portable.zip`、`xxchun-Connect-<版本>-macOS-arm64.zip`；包里的说明改成新流程，去掉了指向上游验证记录的链接，保留「基于 SuperGPT Connect（MIT）」。
- `.github/workflows/release.yml`（新）：推 `v*` tag 时在 macOS 14 和 Windows 上各自测试并构建，再由一个发布任务算 `SHA256SUMS.txt` 并用 `gh release create` 建 Release。`actions/download-artifact` 用的是 `@v4` 标签（其余沿用上游钉死的提交）。`.github/workflows/verify.yml` 是上游的，只被改了名。
- `tests/stations.test.ts`：改成只检查唯一站点。`tests/quick.test.ts`（新）、`tests/brand.test.mjs`（新：四处版本一致、界面里没有旧品牌和「添加站点」、保留的许可文件都在、发布流程和包名）。
- `README.md`：重写成本分支的说明。`NOTICE.md`（新）：分支声明。`VERIFICATION.md`、`COMPATIBILITY.md`、`UPSTREAM.md`、`LICENSE`、`licenses/`、`CHANGELOG.md`：上游原样。

## 没做的（以及为什么）

- **预填密钥（自定义网址协议 / 一次性码）没做。** 两种都要新的 Rust 代码：协议要加 tauri 的 deep-link 插件并改 `Cargo.lock`，一次性码要在 Rust 里加一次网络请求（界面的 CSP 不允许网页自己联网）。这里编译不了，不敢盲写。现在的办法是：网站上点「复制我的密钥」，应用里点「粘贴密钥」（只接受长得像密钥的内容）。
- **重启客户端的按钮没有。** 上游没有这个功能，也没有打开网页的插件；结果页只写「完全退出再打开」，网站地址是写出来的字，不是链接。
- 商城的密钥是按工具分的（Claude 的密钥读不到 GPT 模型）。拿 Claude 的密钥点「接到 Codex」会得到一句提示，让他回网站复制 Codex 的密钥。
