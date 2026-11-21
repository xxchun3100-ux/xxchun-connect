use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    Claude,
    Codex,
    #[serde(rename = "claude-desktop")]
    ClaudeDesktop,
    #[serde(rename = "claude-vscode")]
    ClaudeVscode,
    #[serde(rename = "codex-desktop")]
    CodexDesktop,
}
impl Client {
    pub const ALL: [Self; 5] = [
        Self::CodexDesktop,
        Self::Codex,
        Self::ClaudeDesktop,
        Self::Claude,
        Self::ClaudeVscode,
    ];
    pub fn is_codex(self) -> bool {
        matches!(self, Self::Codex | Self::CodexDesktop)
    }
    // Both Codex entry points share one configuration and one restore point.
    pub fn backup_client(self) -> Self {
        if self == Self::CodexDesktop {
            Self::Codex
        } else {
            self
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::ClaudeDesktop => "claude-desktop",
            Self::ClaudeVscode => "claude-vscode",
            Self::CodexDesktop => "codex-desktop",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code 终端版",
            Self::Codex => "Codex 终端版",
            Self::ClaudeDesktop => "Claude 桌面版",
            Self::ClaudeVscode => "Claude Code · VS Code",
            Self::CodexDesktop => "Codex 桌面版",
        }
    }
}

// Values containing credentials deliberately do not implement Debug or Serialize.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryInput {
    pub client: Client,
    pub api_key: String,
    pub base_url: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionInput {
    pub discovery_id: String,
    pub selected_models: Vec<String>,
    pub default_model: String,
    #[serde(default)]
    pub tool_search: crate::capabilities::ToolSearchMode,
    #[serde(default)]
    pub options: crate::setup_options::SetupOptions,
}
#[derive(Clone, Debug, Serialize)]
pub struct ModelChoice {
    pub id: String,
    pub name: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResponse {
    pub discovery_id: String,
    pub client: Client,
    pub base_url: String,
    pub models: Vec<ModelChoice>,
    pub capabilities: crate::capabilities::RouteCapabilities,
    pub via_system_network: bool,
}

pub struct DiscoveredModels {
    pub client: Client,
    pub api_key: String,
    pub root: String,
    pub payload: Value,
    pub choices: Vec<ModelChoice>,
    pub capabilities: crate::capabilities::RouteCapabilities,
    // The catalog could only be read through the operating system's own HTTP stack.
    pub system_network: bool,
}
pub struct ValidatedInput {
    pub client: Client,
    pub api_key: String,
    pub root: String,
    pub model: String,
    pub selected_models: Vec<String>,
    pub catalog: Option<String>,
    pub capabilities: crate::capabilities::RouteCapabilities,
    pub tool_search: crate::capabilities::ToolSearchMode,
    pub options: crate::setup_options::SetupOptions,
}
pub struct DiscoverySession {
    id: String,
    created: Instant,
    data: DiscoveredModels,
}
impl DiscoverySession {
    pub fn new(data: DiscoveredModels) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            id: format!("{now:x}-{:x}", NEXT.fetch_add(1, Ordering::Relaxed)),
            created: Instant::now(),
            data,
        }
    }
    pub fn response(&self) -> DiscoveryResponse {
        DiscoveryResponse {
            discovery_id: self.id.clone(),
            client: self.data.client,
            base_url: self.data.root.clone(),
            models: self.data.choices.clone(),
            capabilities: self.data.capabilities.clone(),
            via_system_network: self.data.system_network,
        }
    }
    pub fn prepare(&self, input: &SelectionInput) -> Result<ValidatedInput, String> {
        if input.discovery_id != self.id || self.created.elapsed() > Duration::from_secs(15 * 60) {
            return Err("模型列表已失效，请重新获取后再导入。".into());
        }
        let mut selected =
            prepare_selection(&self.data, &input.selected_models, &input.default_model)?;
        selected.tool_search = input.tool_search;
        input.options.validate(&selected)?;
        selected.options = input.options.clone();
        Ok(selected)
    }
}

pub fn normalize_root(input: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(input.trim())
        .map_err(|_| "API 地址格式不正确，请填写完整的 https:// 地址。")?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err("API 地址需要使用 HTTPS；本机调试地址可以使用 HTTP。".into());
    }
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("API 地址不能包含账号、密码、查询参数或锚点。".into());
    }
    let path = url.path().trim_end_matches('/').to_string();
    if path.ends_with("/responses")
        || path.ends_with("/messages")
        || path.ends_with("/chat/completions")
    {
        return Err("请填写 API 基础地址，不要包含 /responses 或 /messages 等接口路径。".into());
    }
    url.set_path(path.strip_suffix("/v1").unwrap_or(&path));
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub fn catalog_models(payload: &Value) -> Vec<String> {
    let rows = payload
        .get("data")
        .or_else(|| payload.get("models"))
        .and_then(Value::as_array);
    let mut models = Vec::new();
    if let Some(rows) = rows {
        for row in rows.iter().take(5000) {
            if let Some(id) = row
                .get("id")
                .or_else(|| row.get("slug"))
                .and_then(Value::as_str)
            {
                let id = id.trim();
                if !id.is_empty()
                    && id.len() <= 200
                    && !id.chars().any(char::is_control)
                    && !models.iter().any(|s| s == id)
                {
                    models.push(id.to_string());
                }
            }
        }
    }
    models
}

// Match the catalog protocol exercised by verify-codex-runtime.py. A plain
// model list can contain names only, omitting image and reasoning capabilities.
const CODEX_CATALOG_CLIENT_VERSION: &str = "0.162.0";

const BODY_LIMIT: usize = 2 * 1024 * 1024;
const READ_FAILED: &str = "读取模型目录失败，请重试。";
const TOO_LARGE: &str = "模型目录过大，配置尚未写入。";
// The addresses of this shop's API, in the order they are tried. Some networks reset
// connections by the name asked for, so the same service answers under more than one name.
// src/stations.ts lists the same addresses for the window (a test compares the two).
pub const BUILT_IN_ENTRIES: [&str; 2] = ["https://api.xxchunai.com", "https://api.xxchun.com"];
// A list fetched from the service may only name hosts under these domains, so an answer
// that was tampered with on the way cannot send anybody's key somewhere else.
const ENTRY_DOMAINS: [&str; 2] = ["xxchunai.com", "xxchun.com"];
const ENTRY_LIST_SECONDS: u64 = 3;
const ENTRY_LIST_LIMIT: usize = 6;
// The longest the first attempt at an entry may take.
const HTTP_SECONDS: u64 = 10;
const FOLLOW_UP_SECONDS: u64 = 15;
// After the first attempt at an entry, further reqwest attempts are made only inside this
// much time since the start, so that fast failures are retried several times while slow
// attempts are not repeated. Nothing at all is started after TOTAL_BUDGET_MS.
const HTTP_BUDGET_MS: u64 = 20_000;
const TOTAL_BUDGET_MS: u64 = 40_000;
const CURL_SECONDS: u64 = 6;
// Waits before each round (a round is the system-settings client, then the direct one).
// Establishing the connection is flaky on some customers' paths and a plain retry works.
const ROUND_WAITS_MS: [u64; 4] = [0, 400, 1000, 2000];
// Later requests of a discovery are retried on the transport that already worked.
const FOLLOW_UP_WAITS_MS: [u64; 3] = [0, 400, 1000];
// The key is never an argument (other programs can read arguments): the whole request is
// a config read from standard input. -q keeps a personal .curlrc out of it.
const CURL_ARGS: [&str; 3] = ["-q", "--config", "-"];

// The ways a request is made: as the system is set up (its proxy included), without any
// proxy, and through the operating system's own curl, which trusts the system certificate
// store (HTTPS scanning by antivirus adds its certificate there, and the bundled roots
// used by the first two do not know it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    System,
    Direct,
    Curl,
}
// The entry (by its place in the list) and route that were tried, the kind of the cause,
// and the sanitized cause.
type Tried = (usize, Route, &'static str, String);

// Every attempt of a discovery, in order: the wait before it in milliseconds, the entry
// and the route. A round is the system-settings client, then the direct one. Each entry
// gets two rounds before the next entry is tried, then each gets its two later rounds,
// and at the end the system's curl is tried once for each entry.
pub fn attempt_plan(entries: usize) -> Vec<(u64, usize, Route)> {
    let mut plan = Vec::new();
    for rounds in [0..2, 2..4] {
        for entry in 0..entries {
            for round in rounds.clone() {
                plan.push((ROUND_WAITS_MS[round], entry, Route::System));
                plan.push((0, entry, Route::Direct));
            }
        }
    }
    for entry in 0..entries {
        plan.push((0, entry, Route::Curl));
    }
    plan
}

// How long an attempt may take, or None when it is skipped because the time is used up.
// `first` is the first reqwest attempt at its entry: every entry gets one, so a name that
// hangs cannot use up the time of the next name. `elapsed_ms` is the time since the
// discovery began, the wait included.
fn attempt_seconds(route: Route, first: bool, elapsed_ms: u64) -> Option<u64> {
    let allowed = if route == Route::Curl {
        CURL_SECONDS
    } else if first {
        HTTP_SECONDS
    } else {
        (HTTP_BUDGET_MS.saturating_sub(elapsed_ms) / 1000).min(HTTP_SECONDS)
    };
    let seconds = allowed.min(TOTAL_BUDGET_MS.saturating_sub(elapsed_ms) / 1000);
    if seconds < 2 {
        None
    } else {
        Some(seconds)
    }
}

// An address a fetched list may name: https, a host under one of this shop's domains,
// and nothing else in it. Returned in the form the built-in entries have.
fn allowed_entry(text: &str) -> Option<String> {
    let url = reqwest::Url::parse(text.trim()).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let on_list = ENTRY_DOMAINS
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")));
    if url.scheme() != "https"
        || !on_list
        || url.port().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(format!("https://{host}"))
}

// {"entries":["https://…", …]}. One address that is not allowed and the whole list is
// dropped; only the first ENTRY_LIST_LIMIT are read.
fn parse_entries(body: &[u8]) -> Option<Vec<String>> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let rows = value.get("entries")?.as_array()?;
    let mut entries = Vec::new();
    for row in rows.iter().take(ENTRY_LIST_LIMIT) {
        let entry = allowed_entry(row.as_str()?)?;
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }
    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

// A fetched list goes first, in its own order; the built-in entries always stay in.
fn merged_entries(fetched: Option<Vec<String>>) -> Vec<String> {
    let mut entries = fetched.unwrap_or_default();
    for entry in BUILT_IN_ENTRIES {
        if !entries.iter().any(|known| known.as_str() == entry) {
            entries.push(entry.to_string());
        }
    }
    entries
}

// Best effort and short: the first built-in entry that gives any HTTP answer settles it
// (today the file does not exist and the answer is 404). The key is not sent.
async fn fetched_entries() -> Option<Vec<String>> {
    let Ok(Transport::Http(client)) = Transport::open(Route::System) else {
        return None;
    };
    for entry in BUILT_IN_ENTRIES {
        let sent = client
            .get(format!("{entry}/connect-entries.json"))
            .timeout(Duration::from_secs(ENTRY_LIST_SECONDS))
            .header("Accept", "application/json")
            .send()
            .await;
        let Ok(mut response) = sent else {
            continue;
        };
        if !response.status().is_success() {
            return None;
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if body.len() + chunk.len() > 16 * 1024 {
                return None;
            }
            body.extend_from_slice(&chunk);
        }
        return parse_entries(&body);
    }
    None
}

fn host_of(root: &str) -> String {
    reqwest::Url::parse(root)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| root.to_string())
}

// A failure that came back quickly may be gone a moment later; a timeout is not repeated.
fn worth_retrying(outcome: &Result<Reply, Failure>) -> bool {
    matches!(outcome, Err(Failure::Unreachable { kind, .. }) if *kind != "timeout")
}

// The crate has no async timer among its dependencies; the waits are short.
fn pause(milliseconds: u64) {
    if milliseconds > 0 {
        std::thread::sleep(Duration::from_millis(milliseconds));
    }
}

impl Route {
    fn label(self) -> &'static str {
        match self {
            Self::System => "系统网络设置",
            Self::Direct => "不走代理",
            Self::Curl => "系统自带 curl",
        }
    }
}

// Whatever reached the API first is used for every later request of the same discovery.
pub enum Transport {
    Http(reqwest::Client),
    Curl,
}
#[derive(Debug, PartialEq)]
pub(crate) struct Reply {
    pub(crate) status: u16,
    // Read only for a successful status on the reqwest routes; never more than BODY_LIMIT.
    pub(crate) body: Vec<u8>,
}
#[derive(Debug, PartialEq)]
pub(crate) enum Failure {
    // No HTTP answer at all. The detail is already sanitized; the kind "timeout" is a timeout.
    Unreachable { kind: &'static str, detail: String },
    // This computer has no system curl.
    Unavailable,
    // The request could not even be put together: that is the pasted text, not the network.
    Builder,
    Message(&'static str),
}
impl Failure {
    fn into_message(self, host: &str, route: Route) -> String {
        match self {
            Self::Builder => KEY_HELP.to_string(),
            Self::Message(message) => message.to_string(),
            Self::Unavailable => connection_message(host, false, ""),
            Self::Unreachable { kind, detail } => {
                unreachable_message(&[host.to_string()], &[(0, route, kind, detail)])
            }
        }
    }
}
impl Transport {
    // A new client for every attempt, and no idle connections kept: nothing is reused
    // from a connection that may have gone bad.
    fn open(route: Route) -> Result<Self, String> {
        let direct = match route {
            Route::System => false,
            Route::Direct => true,
            Route::Curl => return Ok(Transport::Curl),
        };
        let mut builder = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .redirect(reqwest::redirect::Policy::none());
        if direct {
            builder = builder.no_proxy();
        }
        builder
            .build()
            .map(Transport::Http)
            .map_err(|_| "无法初始化网络连接。".to_string())
    }
    // Credentials stay on this exact origin; redirects are never followed.
    async fn get(&self, url: &str, api_key: &str, seconds: u64) -> Result<Reply, Failure> {
        match self {
            Transport::Http(client) => http_get(client, url, api_key, seconds).await,
            // This waits for the child process (at most `seconds` + 2).
            Transport::Curl => curl_get(url, api_key, seconds),
        }
    }
    // For the requests that follow the first answer: the same transport, tried again
    // after a short wait when the connection could not be made.
    pub(crate) async fn get_retrying(&self, url: &str, api_key: &str) -> Result<Reply, Failure> {
        let mut outcome = Err(Failure::Unavailable);
        for wait in FOLLOW_UP_WAITS_MS {
            pause(wait);
            outcome = self.get(url, api_key, FOLLOW_UP_SECONDS).await;
            if !worth_retrying(&outcome) {
                break;
            }
        }
        outcome
    }
}

async fn http_get(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    seconds: u64,
) -> Result<Reply, Failure> {
    let sent = client
        .get(url)
        .timeout(Duration::from_secs(seconds))
        .bearer_auth(api_key)
        .header("Accept", "application/json")
        .send()
        .await;
    let mut response = match sent {
        Ok(response) => response,
        Err(error) if error.is_builder() => return Err(Failure::Builder),
        Err(error) => {
            let (kind, detail) = describe_error(&error, api_key);
            return Err(Failure::Unreachable { kind, detail });
        }
    };
    let status = response.status().as_u16();
    let mut body = Vec::new();
    if response.status().is_success() {
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| Failure::Message(READ_FAILED))?
        {
            if body.len() + chunk.len() > BODY_LIMIT {
                return Err(Failure::Message(TOO_LARGE));
            }
            body.extend_from_slice(&chunk);
        }
    }
    Ok(Reply { status, body })
}

fn curl_program() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    let path = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
        .join("System32")
        .join("curl.exe");
    #[cfg(not(windows))]
    let path = std::path::PathBuf::from("/usr/bin/curl");
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}
pub fn system_curl_available() -> bool {
    curl_program().is_some()
}

// A quoted value of a curl config file: backslash and double quote are escaped.
fn curl_quote(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// One GET without redirects. The status code is printed after the body on a line of its own.
fn curl_config(url: &str, api_key: &str, seconds: u64) -> String {
    let mut lines = vec![
        format!("url = {}", curl_quote(url)),
        format!(
            "header = {}",
            curl_quote(&format!("Authorization: Bearer {api_key}"))
        ),
        format!("header = {}", curl_quote("Accept: application/json")),
        format!("max-time = {seconds}"),
        format!("max-filesize = {BODY_LIMIT}"),
        format!("proto = {}", curl_quote("=http,https")),
        format!("write-out = {}", curl_quote("\n%{http_code}")),
    ];
    for flag in ["globoff", "silent", "show-error"] {
        lines.push(flag.to_string());
    }
    lines.join("\n") + "\n"
}

fn parse_curl_output(stdout: &[u8]) -> Option<Reply> {
    let at = stdout.iter().rposition(|byte| *byte == b'\n')?;
    let code = &stdout[at + 1..];
    if code.len() != 3 || !code.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let status: u16 = std::str::from_utf8(code).ok()?.parse().ok()?;
    if status < 100 {
        // curl prints 000 when it got no answer.
        return None;
    }
    Some(Reply {
        status,
        body: stdout[..at].to_vec(),
    })
}

fn curl_kind(code: Option<i32>) -> &'static str {
    match code {
        Some(28) | None => "timeout",
        Some(5 | 97) => "proxy",
        Some(6) => "dns",
        Some(7) => "connect",
        Some(35 | 51 | 53 | 54 | 58 | 59 | 60 | 66 | 77 | 80 | 82 | 83 | 90 | 91) => "tls",
        Some(_) => "request",
    }
}

// `code` is None when curl had to be stopped.
fn curl_outcome(
    code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
    api_key: &str,
) -> Result<Reply, Failure> {
    if stdout.len() > BODY_LIMIT + 16 || code == Some(63) {
        return Err(Failure::Message(TOO_LARGE));
    }
    if code == Some(0) {
        if let Some(reply) = parse_curl_output(stdout) {
            return Ok(reply);
        }
    }
    let said = String::from_utf8_lossy(stderr);
    let said = said.trim();
    let cause = match (said.is_empty(), code) {
        (false, _) => said.to_string(),
        (true, Some(code)) => format!("curl exit {code}"),
        (true, None) => "curl did not finish".to_string(),
    };
    Err(Failure::Unreachable {
        kind: curl_kind(code),
        detail: sanitize_cause(&cause, api_key),
    })
}

fn read_limited<R: std::io::Read>(source: Option<R>, limit: usize) -> Vec<u8> {
    use std::io::Read;
    let mut bytes = Vec::new();
    if let Some(source) = source {
        // Dropping the pipe at the limit makes the child stop instead of filling memory.
        let _ = source.take(limit as u64).read_to_end(&mut bytes);
    }
    bytes
}

fn curl_get(url: &str, api_key: &str, seconds: u64) -> Result<Reply, Failure> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let Some(program) = curl_program() else {
        return Err(Failure::Unavailable);
    };
    let mut command = Command::new(program);
    command
        .args(CURL_ARGS)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let Ok(mut child) = command.spawn() else {
        return Err(Failure::Unavailable);
    };
    let config = curl_config(url, api_key, seconds);
    // The pipe is closed when the closure returns, so curl sees the end of its config.
    let written = child
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(config.as_bytes()).is_ok())
        .unwrap_or(false);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let body = std::thread::spawn(move || read_limited(stdout, BODY_LIMIT + 64));
    let errors = std::thread::spawn(move || read_limited(stderr, 4096));
    let start = Instant::now();
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if written && start.elapsed() < Duration::from_secs(seconds + 2) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let body = body.join().unwrap_or_default();
    let errors = errors.join().unwrap_or_default();
    curl_outcome(code, &body, &errors, api_key)
}

// The same reading of an answer whichever way it arrived.
fn catalog_payload(reply: &Reply) -> Result<Value, String> {
    if !(200..300).contains(&reply.status) {
        return Err(match reply.status {
            401 => "密钥未通过验证，请检查是否复制完整。".into(),
            403 => "此密钥没有访问权限，请检查分组、有效期或账户状态。".into(),
            404 | 405 => {
                "此地址没有提供模型目录，请检查 API 基础地址。首版需要 /v1/models 接口。".into()
            }
            429 => "请求过于频繁或额度受限，请稍后重试。".into(),
            code => format!("模型目录暂时不可用（HTTP {code}），配置尚未写入。"),
        });
    }
    serde_json::from_slice(&reply.body)
        .map_err(|_| "API 返回的模型目录不是有效 JSON。".to_string())
}

// The first line is for the person; the line after it is the technical cause, shown small.
fn connection_message(host: &str, timed_out: bool, detail: &str) -> String {
    let what = if timed_out {
        format!("连接 xxchun（{host}）超时了。")
    } else {
        format!("连不上 xxchun（{host}）。")
    };
    let advice = format!(
        "{what}请暂时关掉代理/VPN 和杀毒软件的\"HTTPS 扫描/网页防护\"再试，或换个网络。"
    );
    if detail.is_empty() {
        advice
    } else {
        format!("{advice}\n{detail}")
    }
}

fn empty_catalog_message(client: Client) -> &'static str {
    if client.is_codex() {
        "这把密钥用不了 Codex 的模型。回教程页重新点一次『复制我的密钥』。"
    } else {
        "这把密钥用不了 Claude 的模型。回教程页重新点一次『复制我的密钥』。"
    }
}

const KEY_HELP: &str =
    "这不是完整的密钥。回到 xxchun 的接入教程，点『复制我的密钥』，再回来点『粘贴密钥』。";
const KEY_MASKED: &str = "你复制的是打了码的密钥（中间是点），不能用。";

// Spaces and quotes around a key come from copying it; they are not part of it.
pub fn normalize_key(input: &str) -> &str {
    input.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '“' | '”' | '‘' | '’')
    })
}

// A key of this shop is exactly sk-xc- and 32 letters or digits. Anything else is answered
// here, before any request: the masked form shown in lists cannot even be sent as a header.
pub fn key_problem(input: &str) -> Option<String> {
    let key = normalize_key(input);
    let valid = key
        .strip_prefix("sk-xc-")
        .is_some_and(|rest| rest.len() == 32 && rest.chars().all(|c| c.is_ascii_alphanumeric()));
    if valid {
        None
    } else if key.chars().any(|c| matches!(c, '·' | '•' | '…' | '*')) {
        Some(format!("{KEY_MASKED}{KEY_HELP}"))
    } else {
        Some(KEY_HELP.to_string())
    }
}

// "connect ×3, tls ×1": each kind of cause with how often it was met, in the order met.
fn cause_summary(kinds: &[&str]) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for kind in kinds {
        match counts.iter().position(|(seen, _)| seen == kind) {
            Some(at) => counts[at].1 += 1,
            None => counts.push((*kind, 1)),
        }
    }
    counts
        .iter()
        .map(|(kind, count)| format!("{kind} ×{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

// One line: how many attempts and which kinds of cause, then for each entry that was
// tried the last cause on each route.
fn failure_detail(hosts: &[String], failures: &[Tried]) -> String {
    if failures.is_empty() {
        return String::new();
    }
    let kinds: Vec<&str> = failures.iter().map(|tried| tried.2).collect();
    let mut parts = Vec::new();
    for (entry, host) in hosts.iter().enumerate() {
        let mut last = Vec::new();
        for route in [Route::System, Route::Direct, Route::Curl] {
            let found = failures
                .iter()
                .rev()
                .find(|tried| tried.0 == entry && tried.1 == route);
            if let Some(tried) = found {
                last.push(format!("{}：{}: {}", route.label(), tried.2, tried.3));
            }
        }
        if !last.is_empty() {
            parts.push(format!("{host} — {}", last.join("；")));
        }
    }
    format!(
        "试了 {} 次：{}。{}",
        failures.len(),
        cause_summary(&kinds),
        parts.join("。")
    )
}

fn unreachable_message(hosts: &[String], failures: &[Tried]) -> String {
    let timed_out = !failures.is_empty() && failures.iter().all(|tried| tried.2 == "timeout");
    connection_message(&hosts.join("、"), timed_out, &failure_detail(hosts, failures))
}

fn describe_error(error: &reqwest::Error, api_key: &str) -> (&'static str, String) {
    let chain = error_chain(error);
    let kind = cause_kind(
        error.is_timeout(),
        error.is_connect(),
        error.is_builder(),
        &chain,
    );
    (kind, sanitize_cause(&chain, api_key))
}

fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(" | ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

fn cause_kind(timeout: bool, connect: bool, builder: bool, chain: &str) -> &'static str {
    fn has(text: &str, words: &[&str]) -> bool {
        words.iter().any(|word| text.contains(*word))
    }
    let text = chain.to_ascii_lowercase();
    if builder {
        "request-build"
    } else if timeout {
        "timeout"
    } else if has(&text, &["proxy", "tunnel", "socks"]) {
        "proxy"
    } else if has(&text, &["dns", "lookup address", "no such host"]) {
        "dns"
    } else if has(&text, &["certificate", "tls", "handshake"]) {
        "tls"
    } else if connect {
        "connect"
    } else {
        "request"
    }
}

// What may be shown or pasted into a support chat: no key, no URL query, no URL credentials.
fn sanitize_cause(text: &str, api_key: &str) -> String {
    fn strip_url(word: &str) -> String {
        let Some(scheme) = word.find("://") else {
            return word.to_string();
        };
        let (head, rest) = word.split_at(scheme + 3);
        let authority = rest
            .find(|c: char| matches!(c, '/' | '?' | '#'))
            .unwrap_or(rest.len());
        let rest = match rest[..authority].rfind('@') {
            Some(at) => &rest[at + 1..],
            None => rest,
        };
        let Some(query) = rest.find(|c: char| matches!(c, '?' | '#')) else {
            return format!("{head}{rest}");
        };
        // Keep the bracket or full stop that closed the URL in the sentence.
        let closing = rest
            .trim_end_matches(|c: char| matches!(c, ')' | ']' | '"' | '>' | '.' | ',' | ';' | ':'))
            .len()
            .max(query);
        format!("{head}{}{}", &rest[..query], &rest[closing..])
    }
    fn mask_keys(word: &str) -> String {
        let mut out = String::new();
        let mut rest = word;
        while let Some(start) = rest.find("sk-") {
            let tail = &rest[start..];
            let inside_word = rest[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric());
            let len = if inside_word {
                3
            } else {
                tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                    .unwrap_or(tail.len())
            };
            out.push_str(&rest[..start]);
            out.push_str(if inside_word { "sk-" } else { "***" });
            rest = &tail[len..];
        }
        out.push_str(rest);
        out
    }
    let text = if api_key.is_empty() {
        text.to_string()
    } else {
        text.replace(api_key, "***")
    };
    let mut words = Vec::new();
    let mut after_bearer = false;
    for word in text.split_whitespace() {
        let clean = if after_bearer {
            "***".to_string()
        } else {
            mask_keys(&strip_url(word))
        };
        after_bearer = word.eq_ignore_ascii_case("bearer");
        words.push(clean);
    }
    let clean: String = words
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    if clean.chars().count() > 300 {
        let mut short: String = clean.chars().take(300).collect();
        short.push('…');
        short
    } else {
        clean
    }
}

fn catalog_url(root: &str, codex: bool) -> String {
    if codex {
        format!("{root}/v1/models?client_version={CODEX_CATALOG_CLIENT_VERSION}")
    } else {
        format!("{root}/v1/models")
    }
}

pub async fn discover(input: DiscoveryInput) -> Result<DiscoveredModels, String> {
    let DiscoveryInput {
        client,
        api_key,
        base_url,
    } = input;
    // Nothing is sent anywhere until the text has the shape of a key.
    if let Some(problem) = key_problem(&api_key) {
        return Err(problem);
    }
    let root = normalize_root(&base_url)?;
    // Any of the shop's own addresses stands for the whole list of them.
    let entries = if BUILT_IN_ENTRIES.contains(&root.as_str()) {
        merged_entries(fetched_entries().await)
    } else {
        vec![root]
    };
    let plan = attempt_plan(entries.len());
    discover_entries(client, &api_key, &entries, &plan).await
}

// One address, and `plan` is the attempts to make at it, each with the wait before it.
pub async fn discover_with(
    input: DiscoveryInput,
    plan: &[(u64, Route)],
) -> Result<DiscoveredModels, String> {
    let DiscoveryInput {
        client,
        api_key,
        base_url,
    } = input;
    let steps: Vec<(u64, usize, Route)> = plan.iter().map(|(wait, route)| (*wait, 0, *route)).collect();
    discover_entries(client, &api_key, &[base_url], &steps).await
}

// `entries` are tried as `plan` says (wait, place of the entry in the list, route). The
// entry that answers is the one everything else uses: the later requests, and the address
// that is written into the client's configuration. An entry that did not answer is never
// written anywhere.
pub async fn discover_entries(
    client: Client,
    key: &str,
    entries: &[String],
    plan: &[(u64, usize, Route)],
) -> Result<DiscoveredModels, String> {
    // Nothing is sent anywhere until the text has the shape of a key.
    if let Some(problem) = key_problem(key) {
        return Err(problem);
    }
    let api_key = normalize_key(key).to_string();
    let mut roots = Vec::new();
    for entry in entries {
        roots.push(normalize_root(entry)?);
    }
    if roots.is_empty() {
        return Err("API 地址格式不正确，请填写完整的 https:// 地址。".into());
    }
    let hosts: Vec<String> = roots.iter().map(|root| host_of(root)).collect();
    let codex = client.is_codex();
    // A name that is reset on this network, a flaky connection, a system proxy that is
    // switched on but not answering, or HTTPS scanning by antivirus is the usual reason a
    // reachable API cannot be reached: go through the plan until one attempt gets an HTTP
    // answer. Any HTTP answer ends the attempts, whatever its status.
    let started = Instant::now();
    let mut failures: Vec<Tried> = Vec::new();
    let mut asked = vec![false; roots.len()];
    let mut reached = None;
    for (wait, entry, route) in plan.iter().copied() {
        let Some(entry_root) = roots.get(entry) else {
            continue;
        };
        let first = route != Route::Curl && !asked[entry];
        let elapsed = started.elapsed().as_millis() as u64 + wait;
        let Some(seconds) = attempt_seconds(route, first, elapsed) else {
            continue;
        };
        pause(wait);
        if route != Route::Curl {
            asked[entry] = true;
        }
        let url = catalog_url(entry_root, codex);
        let transport = Transport::open(route)?;
        let outcome = transport.get(&url, &api_key, seconds).await;
        match outcome {
            Ok(reply) => {
                reached = Some((transport, route, entry, reply));
                break;
            }
            Err(Failure::Unreachable { kind, detail }) => {
                failures.push((entry, route, kind, detail))
            }
            // No system curl on this computer: the causes found so far stand.
            Err(Failure::Unavailable) => {}
            Err(failure) => return Err(failure.into_message(&hosts[entry], route)),
        }
    }
    let Some((transport, route, entry, mut reply)) = reached else {
        return Err(unreachable_message(&hosts, &failures));
    };
    let root = roots[entry].clone();
    let host = hosts[entry].clone();
    let endpoint = format!("{root}/v1/models");
    if codex && matches!(reply.status, 400 | 404 | 405 | 422) {
        // Some third-party gateways reject query parameters. Fall back once,
        // but never retry authentication, rate-limit, redirect, or server errors.
        let plain = transport.get_retrying(&endpoint, &api_key).await;
        reply = plain.map_err(|failure| failure.into_message(&host, route))?;
    }
    let payload = catalog_payload(&reply)?;
    let capabilities = if !client.is_codex() {
        crate::capabilities::discover(&transport, &root, &api_key).await
    } else {
        Default::default()
    };
    let models = catalog_models(&payload);
    if models.is_empty() {
        return Err(empty_catalog_message(client).into());
    }
    let rows = payload
        .get("data")
        .or_else(|| payload.get("models"))
        .and_then(Value::as_array);
    let choices = models
        .into_iter()
        .map(|id| {
            let label = rows
                .and_then(|rows| {
                    rows.iter().find(|row| {
                        row.get("id")
                            .or_else(|| row.get("slug"))
                            .and_then(Value::as_str)
                            .is_some_and(|v| v.trim() == id)
                    })
                })
                .and_then(|row| row.get("display_name").or_else(|| row.get("name")))
                .and_then(Value::as_str)
                .filter(|name| {
                    !name.is_empty() && name.len() <= 200 && !name.chars().any(char::is_control)
                })
                .unwrap_or(&id)
                .to_string();
            ModelChoice { id, name: label }
        })
        .collect();
    Ok(DiscoveredModels {
        client,
        api_key,
        root,
        payload,
        choices,
        capabilities,
        system_network: route == Route::Curl,
    })
}

pub fn prepare_selection(
    data: &DiscoveredModels,
    selected: &[String],
    default_model: &str,
) -> Result<ValidatedInput, String> {
    if selected.is_empty() || selected.len() > 200 {
        return Err("请勾选 1–200 个需要导入的模型。".into());
    }
    let available: HashSet<&str> = data.choices.iter().map(|m| m.id.as_str()).collect();
    let mut seen = HashSet::new();
    for id in selected {
        if !available.contains(id.as_str()) || !seen.insert(id) {
            return Err(
                "所选模型不在本次密钥返回的目录中，或存在重复选择。请重新获取模型。".into(),
            );
        }
    }
    if !selected.iter().any(|id| id == default_model) {
        return Err("请从已勾选模型中指定一个默认模型。".into());
    }
    if selected.iter().any(|id| {
        [
            "gpt-image",
            "dall-e",
            "whisper-",
            "tts-",
            "text-embedding-",
            "sora-",
        ]
        .iter()
        .any(|prefix| id.to_ascii_lowercase().starts_with(prefix))
    }) {
        return Err("图片生成、语音或嵌入专用模型不能作为对话模型导入，请取消勾选。".into());
    }
    let catalog = if data.client.is_codex() {
        Some(build_codex_catalog(&data.payload, selected)?)
    } else {
        None
    };
    Ok(ValidatedInput {
        client: data.client,
        api_key: data.api_key.clone(),
        root: data.root.clone(),
        model: default_model.into(),
        selected_models: selected.to_vec(),
        catalog,
        capabilities: data.capabilities.clone(),
        tool_search: Default::default(),
        options: Default::default(),
    })
}

// CC Switch's native Responses catalog template; gateway-supplied metadata takes precedence.
// The selected list is the complete external catalog, so unchecked models never leak into it.
pub fn build_codex_catalog(payload: &Value, selected: &[String]) -> Result<String, String> {
    let template: Value = serde_json::from_str(include_str!(
        "resources/codex_native_responses_template.json"
    ))
    .map_err(|_| "模型目录模板不可用。")?;
    // Exact model IDs only. These reasoning options come from released Codex catalogs;
    // an unknown alias must get its capabilities from the gateway, not a name heuristic.
    let model_profiles: Value =
        serde_json::from_str(include_str!("resources/codex_model_profiles.json"))
            .map_err(|_| "模型能力配置不可用。")?;
    let mut context_sources = json!({});
    let mut gateway_context_fields = json!({});
    let entries: Vec<Value> = selected
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let mut row = template.clone();
            // Unknown capacity stays unknown until import can consult an existing local repair
            // or Codex cache. Never invent a limit that drives premature compaction.
            row["context_window"] = Value::Null;
            row["max_context_window"] = Value::Null;
            row["auto_compact_token_limit"] = Value::Null;
            context_sources[id] = json!("unknown");
            row["input_modalities"] = json!(["text"]);
            row["default_reasoning_level"] = Value::Null;
            row["supported_reasoning_levels"] = json!([]);
            row["supports_reasoning_summaries"] = json!(false);
            row["display_name"] = json!(id);
            row["description"] = json!(id);
            if let Some(profile) = model_profiles.get(id) {
                for key in crate::codex_context::FIELDS {
                    row[key] = profile[key].clone();
                }
                context_sources[id] = json!("bundled");
                row["default_reasoning_level"] = profile["default"].clone();
                row["supported_reasoning_levels"] = Value::Array(
                    profile["levels"]
                        .as_array()
                        .ok_or("思考强度配置不可用。")?
                        .iter()
                        .map(|effort| json!({
                            "effort": effort,
                            "description": format!("{} reasoning effort", effort.as_str().unwrap_or_default())
                        }))
                        .collect(),
                );
                // Codex also uses this flag to decide whether to send reasoning.effort.
                row["supports_reasoning_summaries"] = json!(true);
                if let Some(effort) = profile.get("multi_agent_reasoning_effort") {
                    row["multi_agent_reasoning_effort"] = effort.clone();
                }
            }
            let mut gateway_levels = false;
            let mut gateway_summaries = false;
            let mut context_fields = Vec::<&str>::new();
            for field in ["data", "models"] {
                if let Some(original) = payload.get(field)
                    .and_then(Value::as_array)
                    .and_then(|rows| rows.iter().find(|m| {
                        m.get("id").or_else(|| m.get("slug"))
                            .and_then(Value::as_str).is_some_and(|s| s.trim() == id)
                    }))
                    .and_then(Value::as_object)
                {
                    gateway_levels |= original.contains_key("supported_reasoning_levels");
                    gateway_summaries |= original.contains_key("supports_reasoning_summaries");
                    for (key, value) in original {
                        // Null capacities mean unknown; they must not erase a known profile.
                        if matches!(key.as_str(), "context_window" | "max_context_window") && value.is_null() {
                            continue;
                        }
                        if let Some(field) = crate::codex_context::FIELDS.iter().find(|field| **field == key) {
                            context_fields.push(field);
                        }
                        row[key] = value.clone();
                    }
                    let has_window = original.get("context_window").is_some_and(|v| !v.is_null());
                    let has_max = original.get("max_context_window").is_some_and(|v| !v.is_null());
                    if has_window && !has_max {
                        row["max_context_window"] = row["context_window"].clone();
                        context_fields.push("max_context_window");
                    } else if has_max && !has_window {
                        if let Some(max) = row["max_context_window"].as_i64() {
                            row["context_window"] = json!(row["context_window"].as_i64().unwrap_or(max).min(max));
                            context_fields.push("context_window");
                        }
                    }
                }
            }
            context_fields.sort_unstable();
            context_fields.dedup();
            if !context_fields.is_empty() {
                context_sources[id] = json!("gateway");
            }
            gateway_context_fields[id] = json!(context_fields);
            // Catalogs encode custom numeric efforts as strings, even when the model API
            // advertises a number. Codex decides how to encode that value in the request.
            normalize_reasoning(&mut row)?;
            let levels = row["supported_reasoning_levels"].as_array()
                .ok_or("模型的思考强度格式不正确，配置尚未写入。")?;
            if levels.iter().any(|level| !level["effort"].is_string() && !level["effort"].is_number()) {
                return Err("模型的思考强度格式不正确，配置尚未写入。".into());
            }
            let supports_reasoning = !levels.is_empty();
            let default = if !supports_reasoning {
                Value::Null
            } else if levels.iter().any(|level| level["effort"] == row["default_reasoning_level"]) {
                row["default_reasoning_level"].clone()
            } else {
                levels.iter().find(|level| level["effort"] == "medium")
                    .unwrap_or(&levels[0])["effort"].clone()
            };
            row["default_reasoning_level"] = default;
            if gateway_levels && !gateway_summaries {
                row["supports_reasoning_summaries"] = json!(supports_reasoning);
            }
            if row["supports_reasoning_summaries"] == false
                && row["supported_reasoning_levels"].as_array().unwrap().iter()
                    .any(|level| level["effort"] != "none")
            {
                return Err("模型目录的思考档位与能力标记不一致，Codex 无法发送所选档位；请检查中转站模型目录，配置尚未写入。".into());
            }
            // Models APIs sometimes omit descriptions on otherwise valid effort entries.
            if let Some(levels) = row["supported_reasoning_levels"].as_array_mut() {
                for level in levels {
                    if level.get("description").is_none() {
                        let label = level["effort"].as_str().map(str::to_owned)
                            .unwrap_or_else(|| level["effort"].to_string());
                        level["description"] = json!(format!("{label} reasoning effort"));
                    }
                }
            }
            row["slug"] = json!(id);
            row["visibility"] = json!("list");
            row["supported_in_api"] = json!(true);
            row["priority"] = json!(index);
            row["upgrade"] = Value::Null;
            crate::codex_context::validate(&row, false)?;
            Ok(row)
        })
        .collect::<Result<_, String>>()?;
    serde_json::to_string_pretty(&json!({"models":entries,"_xxchun_connect":{
        "context_sources":context_sources,"gateway_context_fields":gateway_context_fields
    }}))
    .map_err(|_| "无法生成模型目录。".into())
}

fn normalize_reasoning(row: &mut Value) -> Result<(), String> {
    fn effort(value: &mut Value) -> Result<(), String> {
        if value.is_number() {
            *value = json!(value.to_string());
        }
        if !value
            .as_str()
            .is_some_and(|s| !s.trim().is_empty() && !s.chars().any(char::is_control))
        {
            return Err("模型的思考强度格式不正确，配置尚未写入。".into());
        }
        Ok(())
    }
    for key in ["default_reasoning_level", "multi_agent_reasoning_effort"] {
        if !row[key].is_null() {
            effort(&mut row[key])?;
        }
    }
    if !row["supports_reasoning_summaries"].is_boolean() {
        return Err("模型的思考能力标记必须为布尔值，配置尚未写入。".into());
    }
    let levels = row["supported_reasoning_levels"]
        .as_array_mut()
        .ok_or("模型的思考强度格式不正确，配置尚未写入。")?;
    for level in levels {
        if !level.is_object() {
            return Err("模型的思考强度格式不正确，配置尚未写入。".into());
        }
        effort(&mut level["effort"])?;
        if level["description"].is_null() {
            level["description"] = json!(format!(
                "{} reasoning effort",
                level["effort"].as_str().unwrap()
            ));
        } else if !level["description"].is_string() {
            return Err("模型的思考强度说明必须为文本，配置尚未写入。".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod session_tests {
    use super::*;
    #[test]
    fn expired_discovery_requires_a_new_fetch_even_with_the_matching_id() {
        let mut session = DiscoverySession::new(DiscoveredModels {
            system_network: false,
            capabilities: Default::default(),
            client: Client::Claude,
            api_key: "fake-only".into(),
            root: "https://example.test".into(),
            payload: json!({}),
            choices: vec![ModelChoice {
                id: "a".into(),
                name: "a".into(),
            }],
        });
        let input = SelectionInput {
            tool_search: Default::default(),
            options: Default::default(),
            discovery_id: session.response().discovery_id,
            selected_models: vec!["a".into()],
            default_model: "a".into(),
        };
        assert!(session.prepare(&input).is_ok());
        session.created = Instant::now() - Duration::from_secs(901);
        assert!(session.prepare(&input).is_err());
    }
}

#[cfg(test)]
mod connection_tests {
    use super::*;
    #[test]
    fn causes_are_shown_without_the_key_the_query_or_url_credentials() {
        let key = "sk-test-not-a-real-key";
        let clean = sanitize_cause(
            "error sending request for url (https://user:pass@api.example.test/v1/models?client_version=0.162.0&key=sk-test-not-a-real-key) | client error (Connect)\n| tcp connect error:\tAuthorization: Bearer sk-test-not-a-real-key refused",
            key,
        );
        assert_eq!(
            clean,
            "error sending request for url (https://api.example.test/v1/models) | client error (Connect) | tcp connect error: Authorization: Bearer *** refused"
        );
        // A different key that somehow appears in a cause is masked as well.
        assert_eq!(
            sanitize_cause("token sk-other_KEY-0123456789, task-queue kept", key),
            "token ***, task-queue kept"
        );
        assert_eq!(
            sanitize_cause("http://127.0.0.1:7897/#frag. next", ""),
            "http://127.0.0.1:7897/. next"
        );
        let long = sanitize_cause(&"连".repeat(400), key);
        assert_eq!(long.chars().count(), 301);
        assert!(long.ends_with('…'));
    }
    #[test]
    fn causes_are_named_by_kind() {
        for (timeout, connect, builder, chain, kind) in [
            (false, false, true, "builder error", "request-build"),
            (true, true, false, "operation timed out", "timeout"),
            (false, true, false, "client error (Connect) | proxy connect failed", "proxy"),
            (false, true, false, "dns error | No such host is known", "dns"),
            (false, true, false, "invalid peer certificate: UnknownIssuer", "tls"),
            (false, true, false, "tcp connect error | os error 10061", "connect"),
            (false, false, false, "connection closed before message completed", "request"),
        ] {
            assert_eq!(cause_kind(timeout, connect, builder, chain), kind, "{chain}");
        }
    }
    #[test]
    fn connection_failures_say_what_to_do_and_keep_the_cause_on_its_own_line() {
        assert_eq!(
            connection_message("api.xxchun.com", false, "connect: refused"),
            "连不上 xxchun（api.xxchun.com）。请暂时关掉代理/VPN 和杀毒软件的\"HTTPS 扫描/网页防护\"再试，或换个网络。\nconnect: refused"
        );
        let timeout = connection_message("api.xxchun.com", true, "");
        assert!(timeout.starts_with("连接 xxchun（api.xxchun.com）超时了。请暂时关掉代理/VPN"));
        assert!(!timeout.contains('\n'));
    }
    #[test]
    fn the_plan_gives_each_entry_two_rounds_then_the_later_rounds_then_the_systems_curl() {
        // One entry: four rounds of both clients, then the system's curl once.
        let plan = attempt_plan(1);
        let waits: Vec<u64> = plan.iter().map(|step| step.0).collect();
        assert_eq!(waits, vec![0, 0, 400, 0, 1000, 0, 2000, 0, 0]);
        assert!(plan.iter().all(|step| step.1 == 0));
        for round in plan[..8].chunks(2) {
            assert_eq!((round[0].2, round[1].2), (Route::System, Route::Direct));
        }
        assert_eq!(plan[8], (0, 0, Route::Curl));
        // Two entries: the second is asked after two rounds at the first, not after four.
        let plan = attempt_plan(2);
        let order: Vec<(usize, u64)> = plan
            .iter()
            .filter(|step| step.2 == Route::System)
            .map(|step| (step.1, step.0))
            .collect();
        assert_eq!(
            order,
            vec![
                (0, 0),
                (0, 400),
                (1, 0),
                (1, 400),
                (0, 1000),
                (0, 2000),
                (1, 1000),
                (1, 2000)
            ]
        );
        assert_eq!(plan.len(), 18);
        assert_eq!(plan[16..], [(0, 0, Route::Curl), (0, 1, Route::Curl)]);
        assert!(attempt_plan(0).is_empty());
        // The first attempt at an entry may take its full time whenever it comes; later
        // ones share what is left; nothing starts once the whole time is used up.
        assert_eq!(attempt_seconds(Route::System, true, 0), Some(10));
        assert_eq!(attempt_seconds(Route::Direct, false, 1_000), Some(10));
        assert_eq!(attempt_seconds(Route::System, false, 14_400), Some(5));
        assert_eq!(attempt_seconds(Route::Direct, false, 18_500), None);
        assert_eq!(attempt_seconds(Route::System, true, 18_500), Some(10));
        assert_eq!(attempt_seconds(Route::System, true, 39_000), None);
        assert_eq!(attempt_seconds(Route::Curl, false, 30_000), Some(6));
        assert_eq!(attempt_seconds(Route::Curl, false, 36_500), Some(3));
        assert_eq!(attempt_seconds(Route::Curl, false, 60_000), None);
    }
    #[test]
    fn a_fetched_list_may_only_name_https_hosts_of_this_shop() {
        for (text, entry) in [
            ("https://api.xxchunai.com", "https://api.xxchunai.com"),
            (" https://API2.XXCHUNAI.COM/ ", "https://api2.xxchunai.com"),
            ("https://xxchun.com", "https://xxchun.com"),
            ("https://a.b.xxchun.com", "https://a.b.xxchun.com"),
        ] {
            assert_eq!(allowed_entry(text).as_deref(), Some(entry), "{text}");
        }
        for text in [
            "http://api.xxchunai.com",
            "https://evil.example",
            "https://evilxxchunai.com",
            "https://api.xxchunai.com.evil.example",
            "https://api.xxchunai.com@evil.example",
            "https://user:pass@api.xxchunai.com",
            "https://api.xxchunai.com:8443",
            "https://api.xxchunai.com/v1",
            "https://api.xxchunai.com/?next=https://evil.example",
            "https://api.xxchunai.com#x",
            "https://api.xxchunai.com.",
            "api.xxchunai.com",
            "",
        ] {
            assert_eq!(allowed_entry(text), None, "{text}");
        }
        assert_eq!(
            parse_entries(br#"{"entries":["https://api3.xxchunai.com","https://api.xxchun.com","https://api3.xxchunai.com/"]}"#),
            Some(vec![
                "https://api3.xxchunai.com".to_string(),
                "https://api.xxchun.com".to_string()
            ])
        );
        // One address off the list and the whole answer is dropped.
        let bad: [&[u8]; 7] = [
            br#"{"entries":["https://api.xxchunai.com","https://evil.example"]}"#,
            br#"{"entries":["https://api.xxchunai.com",7]}"#,
            br#"{"entries":[]}"#,
            br#"{"entries":"https://api.xxchunai.com"}"#,
            br#"{"other":[]}"#,
            b"<html>not found</html>",
            b"",
        ];
        for body in bad {
            assert_eq!(parse_entries(body), None);
        }
        // Only the first six are read.
        let many: Vec<String> = (0..9).map(|n| format!("\"https://a{n}.xxchunai.com\"")).collect();
        let many = format!("{{\"entries\":[{}]}}", many.join(","));
        assert_eq!(parse_entries(many.as_bytes()).unwrap().len(), 6);
    }
    #[test]
    fn the_built_in_entries_are_ordered_and_always_stay_in_the_list() {
        assert_eq!(
            BUILT_IN_ENTRIES,
            ["https://api.xxchunai.com", "https://api.xxchun.com"]
        );
        for entry in BUILT_IN_ENTRIES {
            assert_eq!(allowed_entry(entry).as_deref(), Some(entry));
            assert_eq!(normalize_root(entry).as_deref(), Ok(entry));
        }
        assert_eq!(merged_entries(None), BUILT_IN_ENTRIES);
        assert_eq!(
            merged_entries(Some(vec![
                "https://api.xxchun.com".to_string(),
                "https://api3.xxchunai.com".to_string()
            ])),
            [
                "https://api.xxchun.com",
                "https://api3.xxchunai.com",
                "https://api.xxchunai.com"
            ]
        );
        assert_eq!(
            catalog_url(BUILT_IN_ENTRIES[0], true),
            "https://api.xxchunai.com/v1/models?client_version=0.162.0"
        );
        assert_eq!(
            catalog_url(BUILT_IN_ENTRIES[1], false),
            "https://api.xxchun.com/v1/models"
        );
        assert_eq!(host_of("https://api.xxchunai.com"), "api.xxchunai.com");
    }
    #[test]
    fn only_a_quick_failure_to_connect_is_tried_again() {
        let failed = |kind: &'static str| -> Result<Reply, Failure> {
            Err(Failure::Unreachable {
                kind,
                detail: String::new(),
            })
        };
        assert!(worth_retrying(&failed("connect")));
        assert!(worth_retrying(&failed("tls")));
        assert!(!worth_retrying(&failed("timeout")));
        assert!(!worth_retrying(&Err(Failure::Builder)));
        assert!(!worth_retrying(&Err(Failure::Unavailable)));
        assert!(!worth_retrying(&Err(Failure::Message(TOO_LARGE))));
        assert!(!worth_retrying(&Ok(Reply {
            status: 500,
            body: Vec::new()
        })));
    }
    #[test]
    fn the_causes_are_counted_and_the_last_one_of_each_route_is_kept() {
        assert_eq!(
            cause_summary(&["connect", "tls", "connect", "connect"]),
            "connect ×3, tls ×1"
        );
        assert_eq!(cause_summary(&[]), "");
        let hosts = vec!["api.xxchunai.com".to_string(), "api.xxchun.com".to_string()];
        let failures: Vec<Tried> = vec![
            (0, Route::System, "proxy", "a".to_string()),
            (0, Route::Direct, "connect", "b".to_string()),
            (0, Route::System, "proxy", "c".to_string()),
            (1, Route::Direct, "connect", "d".to_string()),
            (1, Route::Curl, "tls", "e".to_string()),
        ];
        assert_eq!(
            failure_detail(&hosts, &failures),
            "试了 5 次：proxy ×2, connect ×2, tls ×1。api.xxchunai.com — 系统网络设置：proxy: c；不走代理：connect: b。api.xxchun.com — 不走代理：connect: d；系统自带 curl：tls: e"
        );
        // An entry that was never reached in the plan is not named.
        assert_eq!(
            failure_detail(&hosts, &failures[..2]),
            "试了 2 次：proxy ×1, connect ×1。api.xxchunai.com — 系统网络设置：proxy: a；不走代理：connect: b"
        );
        assert_eq!(failure_detail(&hosts, &[]), "");
        assert!(unreachable_message(&hosts, &failures)
            .starts_with("连不上 xxchun（api.xxchunai.com、api.xxchun.com）。"));
        let slow: Vec<Tried> = vec![
            (0, Route::System, "timeout", "a".to_string()),
            (1, Route::Curl, "timeout", "b".to_string()),
        ];
        assert!(unreachable_message(&hosts, &slow).starts_with("连接 xxchun"));
        assert!(unreachable_message(&hosts, &[]).starts_with("连不上 xxchun"));
        let message = Failure::Unreachable {
            kind: "tls",
            detail: "c".to_string(),
        }
        .into_message("api.xxchun.com", Route::Curl);
        assert!(message.ends_with("\n试了 1 次：tls ×1。api.xxchun.com — 系统自带 curl：tls: c"));
        assert_eq!(
            Failure::Builder.into_message("api.xxchun.com", Route::System),
            KEY_HELP
        );
    }
    #[test]
    fn the_curl_request_keeps_the_key_out_of_the_arguments_and_escapes_its_values() {
        let key = format!("sk-xc-{}", "A1b2".repeat(8));
        let url = "https://api.example.test/v1/models?client_version=0.162.0";
        let config = curl_config(url, &key, 8);
        assert_eq!(CURL_ARGS, ["-q", "--config", "-"]);
        assert!(CURL_ARGS.iter().all(|arg| !arg.contains("sk-")));
        assert!(config.contains(&format!("header = \"Authorization: Bearer {key}\"\n")));
        assert!(config.contains(&format!("url = \"{url}\"\n")));
        assert!(config.contains("write-out = \"\\n%{http_code}\"\n"));
        for line in [
            "silent",
            "show-error",
            "globoff",
            "max-time = 8",
            "max-filesize = 2097152",
            "header = \"Accept: application/json\"",
        ] {
            assert!(config.lines().any(|found| found == line), "{line}");
        }
        // Redirects are not followed unless asked for.
        assert!(!config.contains("location"));
        assert_eq!(curl_quote("a\"b\\c\nd\u{7}"), "\"a\\\"b\\\\c\\nd\"");
        // A value cannot end its line and start another option.
        let hostile = curl_config("https://x.test/\"\nlocation", &key, 8);
        assert_eq!(hostile.lines().next(), Some("url = \"https://x.test/\\\"\\nlocation\""));
    }
    #[test]
    fn curl_output_is_a_body_and_a_status_line() {
        assert_eq!(
            parse_curl_output(b"{\"models\":[]}\n200"),
            Some(Reply {
                status: 200,
                body: b"{\"models\":[]}".to_vec()
            })
        );
        let two_lines = parse_curl_output(b"line one\nline two\n401").unwrap();
        assert_eq!(two_lines.status, 401);
        assert_eq!(two_lines.body, b"line one\nline two".to_vec());
        assert_eq!(parse_curl_output(b"\n302").unwrap().body.len(), 0);
        let bad: [&[u8]; 6] = [
            b"",
            b"200",
            b"body\n000",
            b"body\n20",
            b"body\n2000",
            b"body\nabc",
        ];
        for output in bad {
            assert_eq!(parse_curl_output(output), None);
        }
    }
    #[test]
    fn a_curl_run_becomes_an_answer_or_a_sanitized_cause() {
        let key = format!("sk-xc-{}", "A1b2".repeat(8));
        assert_eq!(
            curl_outcome(Some(0), b"{}\n200", b"", &key),
            Ok(Reply {
                status: 200,
                body: b"{}".to_vec()
            })
        );
        // An HTTP error is still an answer: it goes to the same status handling.
        assert_eq!(curl_outcome(Some(0), b"\n401", b"", &key).unwrap().status, 401);
        let unreachable = |kind: &'static str, detail: &str| -> Result<Reply, Failure> {
            Err(Failure::Unreachable {
                kind,
                detail: detail.to_string(),
            })
        };
        assert_eq!(
            curl_outcome(Some(60), b"\n000", b"curl: (60) SSL certificate problem\r\n", &key),
            unreachable("tls", "curl: (60) SSL certificate problem")
        );
        assert_eq!(
            curl_outcome(Some(7), b"\n000", b"", &key),
            unreachable("connect", "curl exit 7")
        );
        assert_eq!(
            curl_outcome(Some(28), b"", b"", &key),
            unreachable("timeout", "curl exit 28")
        );
        assert_eq!(
            curl_outcome(None, b"", b"", &key),
            unreachable("timeout", "curl did not finish")
        );
        let said = format!("failed for Bearer {key}");
        assert_eq!(
            curl_outcome(Some(5), b"", said.as_bytes(), &key),
            unreachable("proxy", "failed for Bearer ***")
        );
        assert_eq!(
            curl_outcome(Some(63), b"", b"", &key),
            Err(Failure::Message(TOO_LARGE))
        );
        let flood = vec![b'a'; BODY_LIMIT + 64];
        assert_eq!(
            curl_outcome(Some(23), &flood, b"", &key),
            Err(Failure::Message(TOO_LARGE))
        );
    }
    #[test]
    fn an_answer_is_read_the_same_way_whichever_way_it_arrived() {
        let reply = |status: u16, body: &str| Reply {
            status,
            body: body.as_bytes().to_vec(),
        };
        assert_eq!(
            catalog_payload(&reply(200, "{\"models\":[]}")).unwrap(),
            json!({"models":[]})
        );
        assert!(catalog_payload(&reply(401, "")).unwrap_err().contains("验证"));
        assert!(catalog_payload(&reply(302, "")).unwrap_err().contains("302"));
        assert!(catalog_payload(&reply(200, "<html>")).unwrap_err().contains("JSON"));
    }
    #[test]
    fn an_empty_catalog_names_the_key_that_is_needed() {
        for client in Client::ALL {
            let wants = if client.is_codex() { "Codex" } else { "Claude" };
            let message = empty_catalog_message(client);
            assert!(message.contains(&format!("用不了 {wants} 的模型")));
            assert!(message.contains("重新点一次『复制我的密钥』"));
        }
    }
    #[test]
    fn only_a_complete_shop_key_passes_and_a_masked_one_is_named() {
        let key = format!("sk-xc-{}", "A1b2".repeat(8));
        let short = &key[..key.len() - 1];
        let long = format!("{key}0");
        let underscored = format!("sk-xc-{}", "A1b_".repeat(8));
        let prefixed = format!("Bearer {key}");
        assert_eq!(key_problem(&key), None);
        assert_eq!(key_problem(&format!("  \"{key}\"\r\n")), None);
        assert_eq!(normalize_key(&format!(" “{key}” ")), key);
        for text in [
            "",
            "hello",
            "sk-xc-short",
            "sk-test-not-a-real-key",
            "https://xxchun.com/account?tab=api",
            short,
            long.as_str(),
            underscored.as_str(),
            prefixed.as_str(),
        ] {
            assert_eq!(key_problem(text).as_deref(), Some(KEY_HELP), "{text}");
        }
        for text in [
            "sk-xc-····xT4d",
            "sk-xc-••••xT4d",
            "sk-xc-…xT4d",
            "sk-xc-****xT4d",
        ] {
            let problem = key_problem(text).unwrap();
            assert!(problem.starts_with(KEY_MASKED), "{text}");
            assert!(problem.ends_with(KEY_HELP), "{text}");
        }
    }
}
