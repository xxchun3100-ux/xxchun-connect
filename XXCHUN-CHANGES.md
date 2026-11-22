# xxchunai Connect：相对上游改了什么

上游：https://github.com/superpilot69/supergpt-connect ，tag `v0.1.6`（提交 65c260e）。本分支版本 0.1.9。0.1.9 起显示名是 xxchunai Connect（见文末「0.1.9」），下面 0.1.8 及以前各节里的「xxchun Connect」「xxchun-Connect」「连不上 xxchun（…）」是当时的写法，保留原样。

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
- 商城的密钥是按工具分的（Claude 的密钥读不到 GPT 模型）。拿 Claude 的密钥点「接到 Codex」会得到一句提示，让他回教程页复制 Codex 的密钥（0.1.1 起服务器返回空目录时也是这一句，见下）。

## 0.1.1（2026-11-21）

起因：一位 Windows 用户开着本机代理工具（系统代理 127.0.0.1:7897），密钥是对的，却看到「无法连接 API，请检查网络、证书和 API 地址。」。服务器对同一请求是 200，问题在当时的本机网络环境，而应用把原因藏起来了。

这一版**第一次改了 Rust 的逻辑**（之前只有机械改名），同样没有在这台机器上编译过，要看 GitHub Actions。

- `src-tauri/src/models.rs`：
  - 读取 `/v1/models` 的请求发不出去（不是 HTTP 状态码的那类错误，包括超时）时，用一个 `.no_proxy()` 的连接再试一次；成功了，这次接入后面的请求（Codex 的无参数回退、Claude 的能力探测）也用这个不走代理的连接。两次都失败才报错。最坏情况要等两个 15 秒。
  - 报错是两行：第一行给人看，「连不上 xxchun（api.xxchun.com）。请关掉代理/VPN 再试一次，或换个网络。」，超时是「连接 xxchun（api.xxchun.com）超时了。…」；第二行是技术原因：类别（`request-build` / `timeout` / `proxy` / `dns` / `tls` / `connect` / `request`）加错误链，两次的原因都写上。错误链里的密钥、`Bearer` 后面的内容、任何 `sk-` 开头的串、网址的查询参数和账号密码都会去掉，最长 300 字。类别里 proxy / dns / tls 是按错误文字里的关键词猜的（reqwest 不提供这些判断）。
  - 目录为空（服务器 200 但没有模型）：接 Codex 时说「这把密钥只能用 Claude 的模型，接 Codex 请回教程页复制 Codex 的密钥。」，接 Claude 时反过来。以前是「密钥返回的模型目录为空…」。两种情况都不会写配置（以前也不会）。
  - 新增单元测试 `connection_tests`（清理、分类、文案、空目录），`src-tauri/tests/model_discovery.rs` 加了两个：空目录、连不上的端口。
- `src/quick.ts`：`wrongKeyMessage`（目录不为空但没有这个工具能用的模型时，界面说同一句话）、`splitError`（把报错拆成正文和小字原因）。`tests/quick.test.ts` 各加一个测试。
- `src/App.tsx`、`src/styles.css`：报错框显示小字原因和「再试一次」（接入失败时才有，重试的是刚才那个工具）；改密钥或点「粘贴」不再把报错清掉，只有点关闭或重新接入才消失。
- 版本 0.1.1：`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`（只改根包版本，没有加任何依赖或特性）。`CHANGELOG.md` 顶上加了本分支的一节。
- 删除 `.github/workflows/diag.yml`（临时的连通性诊断）。

没有验证的：Rust 没编译过；没有在真的坏代理环境下试过重试；重试只管「读取模型」这一步，客户端自己之后走不走代理不归这个应用管。

## 0.1.2（2026-11-21）

0.1.1 那位用户的真正原因：第一次粘贴的不是真密钥（多半是网站列表里打了码的 `sk-xc-····xT4d`，中间的点不是 ASCII）。这样的文字放不进请求头，reqwest 在发送前就报「请求拼不出来」，应用却当成了连不上网络。重新复制密钥后就好了。

- 密钥规则（界面 `src/quick.ts` 和 Rust `models.rs` 各写了一份，要一起改）：去掉前后的空白和引号后，必须正好是 `sk-xc-` 加 32 位 `[0-9A-Za-z]`。
  - 不符合：不发请求，「这不是完整的密钥。回到 xxchun 的接入教程，点『复制我的密钥』，再回来点『粘贴密钥』。」；文字里有 `·` `•` `…` `*` 时前面多一句「你复制的是打了码的密钥（中间是点），不能用。」
  - 界面在三处用它：点「粘贴密钥」（不符合就不填进框，提示显示在框下面，红字）、点「接到 …」（不符合就不调用原生层）、框下面的确认行「密钥格式正确（sk-xc-…后四位）」。
  - Rust `discover` 在任何网络请求之前再检查一次（`key_problem` / `normalize_key`）。原来那句「请粘贴完整的 API 密钥，密钥中不能包含空格或换行。」没有了。
- `models.rs`：`reqwest` 的 `is_builder()` 错误直接回上面那句密钥提示，不走「不走代理再试」。**和交代的不同：** `is_request()` 且不是连接错误的情况没有归到密钥提示里——连上以后被对方或代理掐断也是这一类，把它说成密钥不对会误导，所以它仍然重试一次并按网络问题报。密钥现在只可能是字母数字，拼请求头已经不会失败了。
- 空目录和「没有这个工具能用的模型」的提示改成「这把密钥用不了 Codex 的模型。回教程页重新点一次『复制我的密钥』。」（Claude 同理），因为商城现在一把密钥两个工具通用。
- 因为 `discover` 只认商城格式的密钥，测试用的假密钥跟着改了格式：`src-tauri/tests/model_discovery.rs`、`src-tauri/examples/codex_fixture.rs` 和 `scripts/verify-codex-runtime.py` 里的 `FAKE_KEY`（后两处必须一致）。**这也意味着这个应用不能再拿别家格式的密钥去读模型。**
- 测试：Rust `connection_tests` 加了密钥规则（含打码的四种写法），`model_discovery.rs` 加了「不是密钥就不发请求」；`tests/quick.test.ts` 同样的用例。
- 版本 0.1.2：四处，`Cargo.lock` 只改根包版本。

没有验证的：Rust 仍然没在这台机器上编译；界面没有在真应用里点过。

## 0.1.3（2026-11-21）

又有两位 0.1.0 的用户，密钥看得见是对的，还是「无法连接 API」，同一台电脑的浏览器能打开网站。其中一位的录屏里，点下去大约一秒就报错（不是超时，是连接或握手很快失败），而且「疯狂点就会接成」。也就是说：建立连接在他们的线路上时好时坏，单纯重试就有用；另一类常见情况是杀毒软件 / 加速器 / 公司代理做 HTTPS 扫描——它们的根证书在 Windows 证书库里，但 reqwest 用的是打包进去的 webpki 根证书，不认。

### 读取模型怎么试（`src-tauri/src/models.rs`）

- 一共最多 9 次，`attempt_plan()`：四轮，每轮先 `System`（reqwest，按系统设置，含系统代理）再 `Direct`（reqwest，`.no_proxy()`），轮与轮之间等 0 / 0.4 / 1 / 2 秒；最后 `Curl`（系统自带的 curl）一次。
- 只有「根本没拿到 HTTP 回答」才试下一次。拿到任何状态码（401、404、500…）就停，按状态码报。请求拼不出来、读内容失败、内容过大也不再试。
- 时间：第一次最多 15 秒；之后的 reqwest 尝试合起来只用到第 16 秒为止（`attempt_seconds`：剩余不足 2 秒就跳过，直接去 curl），curl 最多 8 秒（10 秒还没结束就杀掉）。所以失败得快的会被多试几轮，一次慢的超时不会被重复。最坏大约 26 秒。
- 每次尝试都新建一个 reqwest 客户端，并且不保留空闲连接（`pool_max_idle_per_host(0)`）。
- 等待用的是 `std::thread::sleep`，curl 也是同步等子进程：这个库的正式依赖里没有 tokio（只在测试依赖里），所以这些等待会占住一个异步工作线程。应用只有一个窗口、一次只做一件事，够用；要改成真正的异步得加依赖。
- curl：Windows 是 `%SystemRoot%\System32\curl.exe`（Windows 10 1803 起自带，走 Schannel 和系统证书库），其他系统是 `/usr/bin/curl`。用 `std::process::Command` 启动，Windows 上带 `CREATE_NO_WINDOW`（和 `compatibility.rs` 里查版本的写法一样）。
  - 命令行参数固定是 `-q --config -`。网址、`Authorization: Bearer …`、`Accept`、`max-time`、`max-filesize`、`proto = "=http,https"`、`globoff`、`silent`、`show-error`、`write-out = "\n%{http_code}"` 都写进子进程的标准输入；配置里的值会转义反斜杠和双引号，换行写成 `\n`。没有 `location`，不跟随跳转。
  - 输出最多读 2 MB + 64 字节，超了就关掉管道，按「模型目录过大」报。
  - 没有 curl 或启动不了：不算一次失败，报错里就没有 curl 那一段。
- 状态码和内容的处理（`catalog_payload`）所有路共用。

### 通了以后

- 哪条路通了就一直用它（`Transport`）。**这个应用里除了读模型目录，只有另外两处联网**：Codex 的无参数回退（目录接口不认 `client_version` 时），和 Claude 的能力探测 `capabilities::discover`（`/v1/sub2api/connect`，签名从 `&reqwest::Client` 改成 `&models::Transport`）。这两处现在都用 `get_retrying`：同一条路，连不上就等 0.4 秒、1 秒各再试一次，超时不重试。`adapter.rs` 的写配置、预览、恢复都不联网。
- `DiscoveredModels` 多了 `system_network`，给界面的是 `viaSystemNetwork`；走 curl 通的，成功页显示「已通过系统网络连接」。

### 报错

- 「连不上 xxchun（api.xxchun.com）。请暂时关掉代理/VPN 和杀毒软件的"HTTPS 扫描/网页防护"再试，或换个网络。」；每一次都是超时的话，开头是「连接 xxchun（api.xxchun.com）超时了。」
- 小字一行：「试了 9 次：connect ×6, tls ×2, timeout ×1。系统网络设置：…；不走代理：…；系统自带 curl：…」——各类原因的次数，加每条路最后一次的类别和原因。原因照旧去掉密钥、`Bearer` 后面的内容、网址参数和账号密码，每段最长 300 字。curl 的类别按退出码定。

### 界面

- 等超过 2.5 秒，按钮上的字从「正在接入…」变成「正在连接，自动重试中…」。原生层没有往界面报进度的通道（没有用事件），所以没有做「第 N 次」。
- 「再试一次」还在。接入进行中所有按钮本来就是禁用的；另外加了一个不依赖重新渲染的标记，连点两下也只发起一次（原生层的 `BusyGuard` 是第二道）。
- `app_status` 多返回 `unextracted`：程序所在路径里有 `\Temp\`，并且带压缩软件的痕迹（`Rar$`、`\7z`、`.zip`、`Temp1_`、`BNZ.`、`360zip`、`haozip`）时为真，窗口上方显示「建议先把压缩包解压到桌面再打开」。只提示，不拦。判断在 `compatibility::runs_from_archive`。

### 测试

- `models.rs` 的 `connection_tests`：尝试计划和时间分配、哪些失败值得再试、原因计数和小字拼接、curl 配置（密钥不在参数里、转义）、curl 输出解析、curl 结果归类、状态处理。
- `compatibility.rs` 的 `archive_tests`：压缩包临时路径的识别。
- `tests/model_discovery.rs`：真跑 curl 打本地假服务器（机器上没有 curl 就跳过）；连不上的端口走一轮加 curl，检查小字。

### 要知道的限制

- **Windows 的 curl 不读「系统代理」设置**（只认 `HTTPS_PROXY` 这类环境变量）。所以最后一条路实际是「不走代理 + 系统证书库」：能解决 HTTPS 扫描，解决不了「必须走代理才能出去」的网络。
- 在做 HTTPS 扫描的机器上，Schannel 可能因为查不到证书吊销信息而失败；没有加 `ssl-revoke-best-effort`（老版本 curl 不认识这个选项）。
- 这些只让「读取模型」更容易通。Claude / Codex 之后自己联网，用的是它们自己的网络栈；同一台机器上它们可能照样时好时坏或被拦。成功页那行小字就是为了这种时候好排查。
- Rust 仍然没在这台机器上编译；curl 这条路没有在真的 HTTPS 扫描环境里试过；界面没有在真应用里点过。

## 0.1.7

和上面的 0.1.3 是同一份代码，只改了版本号：上游自己的 tag 用过 v0.1.3–v0.1.6，所以本分支跳到 0.1.7。

## 0.1.8（2026-11-21）

终于量出来的原因：在中国移动的网络上，`xxchun.com` 这个**名字**（包括 `api.xxchun.com`）的连接会被按 SNI / Host 掐断；同一台服务器换个名字就不会。老板买了 `xxchunai.com`，`https://api.xxchunai.com` 现在提供同一个 API。

### 一组地址（`src/stations.ts` 的 `ENTRIES` 和 `models.rs` 的 `BUILT_IN_ENTRIES`）

- 顺序：`https://api.xxchunai.com`、`https://api.xxchun.com`。两处各写一份，`tests/stations.test.ts` 会读 Rust 源码比对，改一处忘另一处测试会红。
- 界面还是只把第一个地址传给原生层；原生层的规则是：传进来的是内置地址之一，就代表整组地址；传进来的是别的地址（测试、本机调试），就只用那一个。
- 尝试顺序 `attempt_plan(n)`：每个地址先两轮（一轮 = 按系统设置的客户端 + 不走代理的客户端），再换下一个地址；都不行，每个地址再各试后两轮；最后每个地址各用系统 curl 试一次。两个地址一共最多 18 次。
- 时间 `attempt_seconds`：每个地址的第一次最多 10 秒（原来 15 秒），哪怕轮到它时已经过了很久——这样一个卡死的名字吃不掉下一个名字的时间；之后的 reqwest 尝试只在开始后的 20 秒内做；curl 每次最多 6 秒；过了 40 秒什么都不再开始。通了以后的后续请求每次还是最多 15 秒。
- **哪个地址答复了（任何 HTTP 状态码都算），之后全用它**：Codex 的无参数回退、Claude 的能力探测，以及 `DiscoveredModels.root`——写进 Claude 桌面版 3P 配置、Claude Code 设置、Codex TOML 的地址都来自它。没答复的地址不会出现在任何配置里（`tests/model_discovery.rs` 有测试：第一个地址连不上、第二个通，检查写出的 `config.toml`）。
- 注意：第一个地址答复了 401 / 404 / 5xx 也算「答复了」，不会再去问第二个地址。两个名字是同一个服务，所以这样处理；如果以后两个名字背后不是同一个服务，这里要改。
- 界面：密钥框下面那行，接入前写「密钥只发给 api.xxchunai.com 或 api.xxchun.com」，接通后只写答复的那个；成功页多一行小字「接入地址：…」（走了系统 curl 的话后面跟「· 已通过系统网络连接」）。

### 可更新的清单

- 探测之前，原生层会依次向内置地址要 `<地址>/connect-entries.json`（不带密钥，每个最多 3 秒，按系统网络设置走一次，不重试）。第一个给出任何 HTTP 答复的地址就算数：200 就读清单，别的状态码（现在是 404）就安静地用内置清单。两个地址都连不上时最多多等 6 秒。
- 文件格式：`{"entries":["https://api.xxchunai.com","https://…"]}`。只读前 6 个；每个必须是 https、主机名是 `xxchunai.com` / `xxchun.com` 或它们的子域名、不带端口、路径、参数、账号。**有一个不合规，整份清单作废**。
- 清单里的地址排在前面（按清单的顺序），内置地址没被列出的会补在后面——服务器可以加地址、调顺序，但去不掉内置的两个。
- 服务器上现在没有这个文件；要启用时放一个上面格式的静态文件即可。

### 给人看的网站名

- 商城网页还只在 `xxchun.com` 上，所以「密钥在 xxchun.com/account…」没改，只是收进了 `src/stations.ts` 的 `SHOP_SITE` 一个常量里。`README.md` 和两个打包脚本里的说明文字也还写 `xxchun.com`，没有动。

### 报错

- 「连不上 xxchun（api.xxchunai.com、api.xxchun.com）。请暂时关掉代理/VPN 和杀毒软件的"HTTPS 扫描/网页防护"再试，或换个网络。」
- 小字：「试了 18 次：connect ×16, tls ×2。api.xxchunai.com — 系统网络设置：…；不走代理：…；系统自带 curl：…。api.xxchun.com — …」。没轮到的地址不会出现。

### 测试

- `models.rs`：尝试顺序和时间、清单的白名单（各种伪装的主机名）、清单解析和合并、按地址分组的小字。
- `tests/model_discovery.rs`：两个本地端口——第一个连不上时用第二个并写进配置；第一个通时不问第二个；都不通时不选任何地址、两个都被点名。
- `tests/stations.test.ts`：地址顺序、锁那一行的文字、和 Rust 清单一致。

没有验证的：Rust 仍然没在这台机器上编译；没有从中国移动的网络上实际跑过这个版本；`connect-entries.json` 只有单元测试，没有对着真服务器试过。

## 0.1.9：显示名改成 xxchunai Connect

商城从「xxchun」改名「xxchunai」，网站从 xxchun.com 搬到 xxchunai.com。这一版只改人看得见的名字，逻辑没有动。

### 改了的（人看得见的）

- `src-tauri/tauri.conf.json`：`productName` 和窗口 `title` → `xxchunai Connect`（Mac 的 `.app` 名字由 `productName` 决定，所以打出来是 `xxchunai Connect.app`）。`index.html` 的 `<title>` 同。
- `src/App.tsx`：左上角字标、页脚、关于框、成功页「已经接到 xxchunai」、两句报错。`src/quick.ts` 和 `models.rs` 的 `KEY_HELP`（「回到 xxchunai 的接入教程」，两处要一字不差）。
- `src/stations.ts`：`SHOP_SITE` → `https://xxchunai.com`；页面上显示的密钥地址用 `entryHost(SHOP_SITE)`，即 `xxchunai.com/account?tab=api`。`DEFAULT_STATION.name` → `xxchunai`（`id` 仍是 `xxchun`）。`ENTRIES` 没动。
- `src-tauri/src/models.rs`：「连不上 xxchunai（…）」「连接 xxchunai（…）超时了」。`lib.rs`：启动失败弹窗的标题和两句话。`compatibility.rs`：接入后的两句「确认当前服务为 xxchunai Connect」。
- `src-tauri/src/adapter.rs`：写进 Codex `[model_providers.xxchun-connect]` 的 `name` → `xxchunai Connect`。`desktop_config.rs`：写进 Claude 桌面版 `_meta.json` 的配置名 → `xxchunai Connect`。
- `scripts/build-windows-portable.mjs`、`scripts/build-macos.mjs`：包内文件夹、`xxchunai Connect.exe`、`xxchunai Connect.app`、说明文字（网址也换成 xxchunai.com）、压缩包名 `xxchunai-Connect-<版本>-…zip`。
- `.github/workflows/release.yml`、`ci/build.yml`：工作流名、artifact 名、Release 标题。
- `README.md`、`NOTICE.md`、`package.json` 的 description、本文件标题。上游的署名和许可文字没动。

### 故意没改的（改了会让已经装过的那份找不到自己写的东西）

- Tauri 的 `identifier`（`com.xxchun.connect`）：WebView 的数据目录跟着它走。
- Cargo 的包名、可执行文件名、库名（`xxchun-connect`、`xxchun_connect_lib`）：Windows 上看到的 exe 名字是打包脚本复制时起的，和 crate 名无关；Mac 的 `.app` 里面那个可执行文件仍叫 `xxchun-connect`（在 `Contents/MacOS/` 里，一般人看不到）。
- 备份目录 `~/.xxchun-connect`、`xxchun-connect-models.json`、清单里的 `_xxchun_connect`、能力文档的 `xxchun.connect_capabilities`、`localStorage` 的 `xxchun-connect.theme`、测试环境变量 `XXCHUN_CONNECT_TEST_HOME`。
- Codex 的 `model_provider = "xxchun-connect"` 和 `[model_providers.xxchun-connect]`，Claude 桌面版的 profile ID。
- 密钥格式 `sk-xc-` + 32 位；GitHub 仓库名；`public/xxchun.png` 这个文件名；`connect-entries.json` 的白名单（`xxchunai.com`、`xxchun.com`）。
- `CHANGELOG.md` 里 0.1.8 及以前的标题、本文件前面各节：是当时的记录。

### 旧配置（0.1.8 写的）怎么认

查过所有比较：应用**从来不按显示名认自己的配置**。认的是三样，这一版都没动：`~/.xxchun-connect/<客户端>.backup.json` 这份恢复记录（有它就算「接过」，里面存着接入前的原文和接入后内容的哈希）、Codex 的 provider ID `xxchun-connect`（`adapter.rs` 判断是不是同一条线路时用）、Claude 桌面版的 `PROFILE_ID`（`desktop_config.rs` 找自己那一条时用）。所以没有「新旧两个名字都接受」的代码要加——没有任何一处在比名字。

- 0.1.8 接过的电脑直接点「恢复原配置」：记录里的哈希对得上 0.1.8 写的内容，原样恢复。
- 不恢复、直接再点一次「接到…」：同一个 ID 下的 `name` 被改成新名字，不会多出第二个 provider / 第二条桌面配置；恢复记录里「接入前」那份保持最初的，之后恢复回到的仍是最初的配置。

### 测试

- `tests/restore_roundtrip.rs` 新增三个（`connect_as_0_1_8` 把目录摆成 0.1.8 接完之后的样子：文件里是旧名字，恢复记录按旧内容生成）：Codex 旧配置被认出、没有被标成「改动过」、恢复成原文；Codex 旧配置上再接一次，ID 不变、名字换新、恢复点时间不变、恢复成原文；Claude 桌面版同样的一整套（`_meta.json` 里仍是两条，自己那条改名）。
- `tests/config_roundtrip.rs`、`tests/desktop_roundtrip.rs`：新接入写的是新名字 + 旧 ID。
- 跟着文字改的断言：`models.rs` 里「连不上 / 连接 xxchunai（…）」五处，`tests/model_discovery.rs` 两处，`compatibility.rs` 里压缩包路径的样例，`tests/quick.test.ts`、`tests/stations.test.ts`、`tests/brand.test.mjs`。
- `tests/brand.test.mjs` 新增一条：`src/`、`src-tauri/src/`、打包脚本、工作流里不再出现旧显示名（`adapter.rs` 里解释兼容的那句注释除外），并且 ID 和目录名还在。

没有验证的：Rust 仍只在 GitHub Actions 上编译和测试；打出来的两个包没有人实际打开过；没有拿一台真的装过 0.1.8 的电脑试过升级（只有上面的测试）。
