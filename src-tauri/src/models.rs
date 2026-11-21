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
}

pub struct DiscoveredModels {
    pub client: Client,
    pub api_key: String,
    pub root: String,
    pub payload: Value,
    pub choices: Vec<ModelChoice>,
    pub capabilities: crate::capabilities::RouteCapabilities,
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

fn http_client(direct: bool) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none());
    if direct {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .map_err(|_| "无法初始化网络连接。".to_string())
}

async fn fetch_catalog(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    codex_manifest: bool,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = client
        .get(endpoint)
        .bearer_auth(api_key)
        .header("Accept", "application/json");
    if codex_manifest {
        request = request.query(&[("client_version", CODEX_CATALOG_CLIENT_VERSION)]);
    }
    request.send().await
}

// The first line is for the person; the line after it is the technical cause, shown small.
fn connection_message(host: &str, timed_out: bool, detail: &str) -> String {
    let advice = if timed_out {
        format!("连接 xxchun（{host}）超时了。请关掉代理/VPN 再试一次，或换个网络。")
    } else {
        format!("连不上 xxchun（{host}）。请关掉代理/VPN 再试一次，或换个网络。")
    };
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

fn failure_detail(first: &str, direct: &str) -> String {
    if first == direct {
        format!("{first}（不走代理再试，结果一样）")
    } else {
        format!("按系统网络设置：{first}；不走代理：{direct}")
    }
}

fn describe_error(error: &reqwest::Error, api_key: &str) -> String {
    let chain = error_chain(error);
    let kind = cause_kind(
        error.is_timeout(),
        error.is_connect(),
        error.is_builder(),
        &chain,
    );
    format!("{kind}: {}", sanitize_cause(&chain, api_key))
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

pub async fn discover(input: DiscoveryInput) -> Result<DiscoveredModels, String> {
    // Nothing is sent anywhere until the text has the shape of a key.
    if let Some(problem) = key_problem(&input.api_key) {
        return Err(problem);
    }
    let api_key = normalize_key(&input.api_key).to_string();
    let root = normalize_root(&input.base_url)?;
    let host = reqwest::Url::parse(&root)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| root.clone());
    let mut client = http_client(false)?;
    let endpoint = format!("{root}/v1/models");
    let codex = input.client.is_codex();
    // Credentials stay on this exact origin; redirects and bodies are never echoed.
    let attempt = fetch_catalog(&client, &endpoint, &api_key, codex).await;
    let mut response = match attempt {
        Ok(response) => response,
        // The request could not even be put together: that is the pasted text, not the network.
        Err(first) if first.is_builder() => return Err(KEY_HELP.into()),
        Err(first) => {
            // A system proxy that is switched on but not answering is the usual reason a
            // reachable API cannot be reached. Try once more without any proxy, and keep
            // that direct client for the rest of this discovery when it works.
            client = http_client(true)?;
            let direct = fetch_catalog(&client, &endpoint, &api_key, codex).await;
            match direct {
                Ok(response) => response,
                Err(second) => {
                    let detail = failure_detail(
                        &describe_error(&first, &api_key),
                        &describe_error(&second, &api_key),
                    );
                    return Err(connection_message(&host, second.is_timeout(), &detail));
                }
            }
        }
    };
    if codex && matches!(response.status().as_u16(), 400 | 404 | 405 | 422) {
        // Some third-party gateways reject query parameters. Fall back once,
        // but never retry authentication, rate-limit, redirect, or server errors.
        response = fetch_catalog(&client, &endpoint, &api_key, false)
            .await
            .map_err(|error| {
                if error.is_builder() {
                    return KEY_HELP.to_string();
                }
                let detail = describe_error(&error, &api_key);
                connection_message(&host, error.is_timeout(), &detail)
            })?;
    }
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 => "密钥未通过验证，请检查是否复制完整。".into(),
            403 => "此密钥没有访问权限，请检查分组、有效期或账户状态。".into(),
            404 | 405 => {
                "此地址没有提供模型目录，请检查 API 基础地址。首版需要 /v1/models 接口。".into()
            }
            429 => "请求过于频繁或额度受限，请稍后重试。".into(),
            code => format!("模型目录暂时不可用（HTTP {code}），配置尚未写入。"),
        });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "读取模型目录失败，请重试。")?
    {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("模型目录过大，配置尚未写入。".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let payload: Value =
        serde_json::from_slice(&bytes).map_err(|_| "API 返回的模型目录不是有效 JSON。")?;
    let capabilities = if !input.client.is_codex() {
        crate::capabilities::discover(&client, &root, &api_key).await
    } else {
        Default::default()
    };
    let models = catalog_models(&payload);
    if models.is_empty() {
        return Err(empty_catalog_message(input.client).into());
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
        client: input.client,
        api_key,
        root,
        payload,
        choices,
        capabilities,
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
            "连不上 xxchun（api.xxchun.com）。请关掉代理/VPN 再试一次，或换个网络。\nconnect: refused"
        );
        let timeout = connection_message("api.xxchun.com", true, "");
        assert!(timeout.contains("超时") && !timeout.contains('\n'));
        assert_eq!(
            failure_detail("connect: a", "connect: a"),
            "connect: a（不走代理再试，结果一样）"
        );
        assert_eq!(
            failure_detail("proxy: a", "connect: b"),
            "按系统网络设置：proxy: a；不走代理：connect: b"
        );
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
