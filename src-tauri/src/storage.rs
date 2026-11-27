use crate::models::Client;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub claude: PathBuf,
    pub claude_desktop: PathBuf,
    pub claude_threep: PathBuf,
    pub vscode: PathBuf,
    pub codex: PathBuf,
    pub state: PathBuf,
    pub sandbox: bool,
}
impl Paths {
    pub fn for_home(home: &Path) -> Self {
        #[cfg(target_os = "macos")]
        let app_data = home.join("Library/Application Support");
        #[cfg(target_os = "windows")]
        let app_data = home.join("AppData/Local");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let app_data = home.join(".config");
        #[cfg(target_os = "windows")]
        let vscode = home.join("AppData/Roaming/Code/User/settings.json");
        #[cfg(not(target_os = "windows"))]
        let vscode = app_data.join("Code/User/settings.json");
        Self {
            home: home.to_path_buf(),
            claude: home.join(".claude"),
            claude_desktop: app_data.join("Claude"),
            claude_threep: app_data.join("Claude-3p"),
            vscode,
            codex: home.join(".codex"),
            state: home.join(".xxchun-connect"),
            sandbox: true,
        }
    }
    pub fn discover() -> Result<Self, String> {
        // Explicit isolated home for native smoke tests. Never fall back to real home if invalid.
        if cfg!(debug_assertions) {
            if let Some(home) = std::env::var_os("XXCHUN_CONNECT_TEST_HOME") {
                let home = PathBuf::from(home);
                if !home.is_absolute() {
                    return Err("测试目录必须是绝对路径。".into());
                }
                return Ok(Self::for_home(&home));
            }
        }
        let home = dirs::home_dir().ok_or("无法找到用户目录，配置尚未写入。")?;
        let override_dir = |key: &str, fallback: PathBuf| -> Result<PathBuf, String> {
            match std::env::var_os(key) {
                Some(v) if !v.is_empty() => {
                    let p = PathBuf::from(v);
                    if !p.is_absolute() {
                        Err(format!("{key} 必须是绝对路径。"))
                    } else {
                        Ok(p)
                    }
                }
                _ => Ok(fallback),
            }
        };
        let mut paths = Self::for_home(&home);
        paths.claude = override_dir("CLAUDE_CONFIG_DIR", paths.claude)?;
        paths.codex = override_dir("CODEX_HOME", paths.codex)?;
        #[cfg(target_os = "windows")]
        {
            let data = override_dir("LOCALAPPDATA", home.join("AppData/Local"))?;
            paths.claude_desktop = data.join("Claude");
            paths.claude_threep = data.join("Claude-3p");
            paths.vscode = override_dir("APPDATA", home.join("AppData/Roaming"))?
                .join("Code/User/settings.json");
        }
        #[cfg(target_os = "linux")]
        {
            let data = override_dir("XDG_CONFIG_HOME", home.join(".config"))?;
            paths.claude_desktop = data.join("Claude");
            paths.claude_threep = data.join("Claude-3p");
            paths.vscode = data.join("Code/User/settings.json");
        }
        paths.sandbox = false;
        Ok(paths)
    }
    /// A configuration folder as it is to be used (0.1.10): where it is a link (a symbolic link, or a junction on
    /// Windows) that can be followed safely, the place it points to; otherwise the path itself. A link that cannot
    /// be followed stays as it is, and reading or writing through it is refused with a plain sentence.
    pub fn dir(&self, raw: &Path) -> PathBuf {
        follow_link(raw, &self.home, true).unwrap_or_else(|_| raw.to_path_buf())
    }
    /// Every read, backup, write and restore goes through here, so all of them use the same resolved place, and the
    /// restore record stores it.
    pub fn target(&self, kind: FileKind) -> PathBuf {
        let raw = match kind {
            FileKind::ClaudeSettings => self.dir(&self.claude).join("settings.json"),
            FileKind::ClaudeDesktopConfig => self
                .dir(&self.claude_desktop)
                .join("claude_desktop_config.json"),
            FileKind::ClaudeThreepConfig => self
                .dir(&self.claude_threep)
                .join("claude_desktop_config.json"),
            FileKind::ClaudeDesktopProfile => self.dir(&self.claude_threep).join(format!(
                "configLibrary/{}.json",
                crate::desktop_config::PROFILE_ID
            )),
            FileKind::ClaudeDesktopMeta => self
                .dir(&self.claude_threep)
                .join("configLibrary/_meta.json"),
            FileKind::ClaudeVscode => self.vscode.clone(),
            FileKind::CodexConfig => self.dir(&self.codex).join("config.toml"),
            FileKind::CodexAuth => self.dir(&self.codex).join("auth.json"),
            FileKind::CodexCatalog => self.dir(&self.codex).join("xxchun-connect-models.json"),
        };
        // The file itself may be the link (config.toml kept elsewhere): the same rule.
        follow_link(&raw, &self.home, false).unwrap_or(raw)
    }
    /// The extension can override CLAUDE_CONFIG_DIR independently of Connect's process.
    /// Use the same resolution for preview, write and restore; never silently choose another home.
    pub fn for_client(&self, client: Client) -> Result<Self, String> {
        let mut paths = self.clone();
        #[cfg(windows)]
        if client == Client::ClaudeDesktop {
            let data = self
                .claude_desktop
                .parent()
                .ok_or("Claude 数据目录无效。")?;
            paths.claude_desktop = claude_desktop_directory(data, false)?;
            paths.claude_threep = claude_desktop_directory(data, true)?;
        }
        if client == Client::ClaudeVscode {
            let doc = crate::vscode_config::read(self)?;
            if let Some(vars) = doc.get("claudeCode.environmentVariables") {
                let vars = vars
                    .as_array()
                    .ok_or("claudeCode.environmentVariables 必须是数组。")?;
                let values: Vec<_> = vars
                    .iter()
                    .filter(|v| v["name"] == "CLAUDE_CONFIG_DIR")
                    .collect();
                if values.len() > 1 {
                    return Err("VS Code 中有重复的 CLAUDE_CONFIG_DIR，请合并后再导入。".into());
                }
                if let Some(v) = values.first() {
                    let value = v["value"]
                        .as_str()
                        .ok_or("VS Code 的 CLAUDE_CONFIG_DIR 必须是字符串。")?;
                    if !value.is_empty() {
                        let path = PathBuf::from(value);
                        if !path.is_absolute() {
                            return Err("VS Code 的 CLAUDE_CONFIG_DIR 必须是绝对路径，不能使用 ~ 或相对路径。".into());
                        }
                        paths.claude = path;
                    }
                }
            }
        }
        Ok(paths)
    }
    pub(crate) fn record(&self, client: Client, pending: bool) -> PathBuf {
        self.state.join(format!(
            "{}.{}.json",
            client.backup_client().id(),
            if pending { "pending" } else { "backup" }
        ))
    }
    pub fn has_backup(&self, client: Client) -> bool {
        self.record(client, true).is_file() || self.record(client, false).is_file()
    }
    pub fn pending(&self, client: Client) -> bool {
        self.record(client, true).is_file()
    }
    pub fn lock(&self) -> Result<fs::File, String> {
        private_dir(&self.state)?;
        let path = self.state.join("write.lock");
        reject_symlink(&path)?;
        let mut opts = fs::OpenOptions::new();
        opts.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let file = opts.open(&path).map_err(|_| "无法创建配置锁。")?;
        file.try_lock_exclusive()
            .map_err(|_| "另一个导入操作正在进行，请稍后重试。")?;
        Ok(file)
    }
}

/// Some Windows builds suffix Claude's data directory. Never pick an arbitrary
/// first directory when multiple installations exist.
pub fn claude_desktop_directory(data: &Path, threep: bool) -> Result<PathBuf, String> {
    let exact = data.join(if threep { "Claude-3p" } else { "Claude" });
    if exact.is_dir() {
        return Ok(exact);
    }
    let candidates: Vec<_> = fs::read_dir(data)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("Claude") && n.contains("-3p") == threep)
        })
        .filter(|p| {
            p.join("claude_desktop_config.json").is_file() || p.join("configLibrary").is_dir()
        })
        .collect();
    match candidates.as_slice() {
        [] => Ok(exact),
        [path] => Ok(path.clone()),
        _ => Err(
            "检测到多个 Claude 桌面配置目录，无法确定正在使用的安装。请先确认客户端数据目录。"
                .into(),
        ),
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum FileKind {
    ClaudeSettings,
    ClaudeDesktopConfig,
    ClaudeThreepConfig,
    ClaudeDesktopProfile,
    ClaudeDesktopMeta,
    ClaudeVscode,
    CodexConfig,
    CodexAuth,
    CodexCatalog,
}
impl FileKind {
    pub fn primary(client: Client) -> Self {
        match client {
            Client::Claude => Self::ClaudeSettings,
            Client::ClaudeDesktop => Self::ClaudeDesktopProfile,
            Client::ClaudeVscode => Self::ClaudeVscode,
            Client::Codex | Client::CodexDesktop => Self::CodexConfig,
        }
    }
    fn belongs_to(self, client: Client) -> bool {
        match client {
            Client::Claude => self == Self::ClaudeSettings,
            Client::ClaudeDesktop => matches!(
                self,
                Self::ClaudeDesktopConfig
                    | Self::ClaudeThreepConfig
                    | Self::ClaudeDesktopProfile
                    | Self::ClaudeDesktopMeta
            ),
            Client::ClaudeVscode => matches!(self, Self::ClaudeSettings | Self::ClaudeVscode),
            Client::Codex | Client::CodexDesktop => matches!(
                self,
                Self::CodexConfig | Self::CodexAuth | Self::CodexCatalog
            ),
        }
    }
}

pub struct Change {
    pub kind: FileKind,
    pub before: Option<String>,
    pub after: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub kind: FileKind,
    pub path: PathBuf,
    pub before: Option<String>,
    pub after_hash: String,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Backup {
    pub version: u8,
    pub client: Client,
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<u64>,
    pub entries: Vec<Entry>,
}

pub(crate) fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

pub(crate) fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn record_time(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|t| t.as_secs())
}

// ---- a configuration folder (or file) that is a link (0.1.10) ----
// A customer kept `C:\Users\<name>\.codex` on another drive behind a link, and the first versions refused any link.
// Now a link is followed (Paths::dir, Paths::target) when its target exists, is of the right kind (a folder for a
// folder, a file for a file), and lies in a place of the user's own: inside the user's folder, or — on Windows — on
// another fixed local drive outside the system folders. A network place, a system folder, another user's folder, a
// dangling link or a loop is not followed, and nothing is changed; the customer reads how to connect by hand.
pub const MOVED_CODEX: &str = "你的 Codex 配置文件夹被放到了别的位置，我们没能安全地找到它，所以没有改动任何东西。可以手动接：接口地址 https://api.xxchunai.com/v1 ，密钥就是你刚才粘贴的那把。";
pub const MOVED_CLAUDE: &str = "你的 Claude 配置文件夹被放到了别的位置，我们没能安全地找到它，所以没有改动任何东西。可以手动接：接口地址 https://api.xxchunai.com ，密钥就是你刚才粘贴的那把。";
fn moved_message(path: &Path) -> String {
    // Which tool the path belongs to is read from its own last parts (.codex, .claude, Claude-3p …), not from
    // whatever folder names come before them.
    for part in path.components().rev().take(3) {
        let name = part.as_os_str().to_string_lossy().to_lowercase();
        if name.contains("xxchun-connect") && !name.ends_with(".json") {
            return "这个小工具自己的备份文件夹是一个链接，为了安全没有改动任何东西。".to_string();
        }
        if name.contains("codex") {
            return MOVED_CODEX.to_string();
        }
        if name.contains("claude") {
            return MOVED_CLAUDE.to_string();
        }
    }
    MOVED_CLAUDE.to_string()
}
fn is_link(path: &Path) -> bool {
    // (on Windows a junction is reported as a symbolic link too)
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}
// What canonicalize answers, as an ordinary path: on Windows `\\?\C:\…` becomes `C:\…`, and anything that is not on
// a drive letter (a network share, a device) is no place to follow a link to.
#[cfg(windows)]
fn plain_path(path: PathBuf) -> Option<PathBuf> {
    use std::path::{Component, Prefix};
    let mut parts = path.components();
    let letter = match parts.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::VerbatimDisk(letter) | Prefix::Disk(letter) => letter,
            _ => return None,
        },
        _ => return None,
    };
    let mut out = PathBuf::from(format!("{}:\\", (letter as char).to_ascii_uppercase()));
    for part in parts {
        if let Component::Normal(name) = part {
            out.push(name);
        }
    }
    Some(out)
}
#[cfg(not(windows))]
fn plain_path(path: PathBuf) -> Option<PathBuf> {
    Some(path)
}
#[cfg(windows)]
fn allowed_place(target: &Path, home: &Path) -> bool {
    let lower = |path: &Path| path.to_string_lossy().to_lowercase().replace('/', "\\");
    let inside = |base: &str, text: &str| {
        let base = base.trim_end_matches('\\');
        !base.is_empty() && (text == base || text.starts_with(&format!("{base}\\")))
    };
    let text = lower(target);
    let home_real = fs::canonicalize(home)
        .ok()
        .and_then(plain_path)
        .unwrap_or_else(|| home.to_path_buf());
    let home_text = lower(&home_real);
    if inside(&home_text, &text) || inside(&lower(home), &text) {
        return true;
    }
    // Outside the user's folder: only another drive, a fixed local one, and not a system folder.
    if text.chars().next() == home_text.chars().next() {
        return false;
    }
    for key in ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)", "ProgramData"] {
        if let Some(value) = std::env::var_os(key) {
            if inside(&lower(Path::new(&value)), &text) {
                return false;
            }
        }
    }
    fixed_drive(target)
}
#[cfg(windows)]
fn fixed_drive(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    let text = path.to_string_lossy();
    let letter = match text.chars().next() {
        Some(letter) if letter.is_ascii_alphabetic() => letter,
        _ => return false,
    };
    let root: Vec<u16> = std::ffi::OsStr::new(&format!("{letter}:\\"))
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // DRIVE_FIXED is 3: not a removable disk, not a network drive, not a disc.
    unsafe { windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(root.as_ptr()) == 3 }
}
// Elsewhere (macOS, Linux): inside the user's own folder only. Whether a volume under /Volumes is a local disk or a
// network share cannot be told without more than the standard library, so such a place is not followed.
#[cfg(not(windows))]
fn allowed_place(target: &Path, home: &Path) -> bool {
    let home_real = fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    target.starts_with(&home_real) || target.starts_with(home)
}
/// The place a link points to, when it may be followed (see above); the path itself when it is no link.
pub(crate) fn follow_link(path: &Path, home: &Path, want_dir: bool) -> Result<PathBuf, String> {
    if !is_link(path) {
        return Ok(path.to_path_buf());
    }
    let target = match fs::canonicalize(path).ok().and_then(plain_path) {
        Some(target) => target,
        None => return Err(moved_message(path)),
    };
    let kind_ok = match fs::metadata(&target) {
        Ok(meta) => {
            if want_dir {
                meta.is_dir()
            } else {
                meta.is_file()
            }
        }
        Err(_) => false,
    };
    if !kind_ok || !allowed_place(&target, home) {
        return Err(moved_message(path));
    }
    Ok(target)
}

/// A link that was not followed (Paths::target leaves it in place) is never read or written through.
pub fn reject_symlink(path: &Path) -> Result<(), String> {
    if is_link(path) {
        return Err(moved_message(path));
    }
    Ok(())
}

pub fn read_optional(path: &Path) -> Result<Option<String>, String> {
    reject_symlink(path)?;
    if let Some(parent) = path.parent() {
        reject_symlink(parent)?;
    }
    match fs::metadata(path) {
        Ok(meta) if meta.len() > 8 * 1024 * 1024 => {
            return Err(format!("{} 超过配置文件大小限制。", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(format!("无法读取 {}。", path.display())),
        _ => {}
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|_| format!("无法读取 {}，请检查文件权限与 UTF-8 编码。", path.display()))
}

fn private_dir(path: &Path) -> Result<(), String> {
    reject_symlink(path)?;
    if !path.exists() {
        fs::create_dir_all(path).map_err(|_| format!("无法创建 {}。", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法设置目录权限。")?;
        }
    }
    Ok(())
}

// Same-directory staged write + atomic replacement, as in CC Switch's config writer.
// tempfile supplies cross-platform replacement and unique, private temporary files.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    reject_symlink(path)?;
    let parent = path.parent().ok_or("配置路径无效。")?;
    private_dir(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| "无法创建临时配置文件。")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "无法保护配置文件权限。")?;
    }
    temp.write_all(bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|_| "写入临时配置失败。")?;
    temp.persist(path)
        .map_err(|_| format!("无法替换 {}，请关闭占用文件的程序后重试。", path.display()))?;
    Ok(())
}

pub fn commit(paths: &Paths, client: Client, changes: Vec<Change>) -> Result<bool, String> {
    let client = client.backup_client();
    if paths.pending(client) {
        return Err("上次导入被中断，请先点击恢复配置。".into());
    }
    let changes: Vec<_> = changes
        .into_iter()
        .filter(|c| c.before.as_deref() != Some(&c.after))
        .collect();
    if changes.is_empty() {
        return Ok(false);
    }
    // Re-check the entire plan before touching any target.
    for c in &changes {
        if read_optional(&paths.target(c.kind))? != c.before {
            return Err("配置刚被其他程序修改，请关闭客户端后重试。".into());
        }
    }
    let backup = Backup {
        version: 1,
        client,
        created_at: Some(timestamp()),
        entries: changes
            .iter()
            .map(|c| Entry {
                kind: c.kind,
                path: paths.target(c.kind),
                before: c.before.clone(),
                after_hash: digest(&c.after),
            })
            .collect(),
    };
    // The pending journal always captures this operation, so rollback returns to
    // the last working import. The lasting restore point keeps the first baseline.
    let previous_path = paths.record(client, false);
    let mut baseline = if previous_path.exists() {
        let (_, mut saved) = read_backup(paths, client, &previous_path)?;
        saved.created_at = saved.created_at.or_else(|| record_time(&previous_path));
        saved
    } else {
        Backup {
            version: 1,
            client,
            created_at: backup.created_at,
            entries: vec![],
        }
    };
    for new_entry in &backup.entries {
        if let Some(old_entry) = baseline
            .entries
            .iter_mut()
            .find(|entry| entry.kind == new_entry.kind)
        {
            old_entry.after_hash.clone_from(&new_entry.after_hash);
        } else {
            baseline.entries.push(new_entry.clone());
        }
    }
    let record = serde_json::to_vec(&backup).map_err(|_| "无法生成配置备份。")?;
    let baseline_record = serde_json::to_vec(&baseline).map_err(|_| "无法生成原配置备份。")?;
    if record.len() > 8 * 1024 * 1024 || baseline_record.len() > 8 * 1024 * 1024 {
        return Err("配置备份超过大小限制，未修改客户端配置。".into());
    }
    let pending = paths.record(client, true);
    atomic_write(&pending, &record)?;
    let result = (|| {
        for c in &changes {
            if read_optional(&paths.target(c.kind))? != c.before {
                return Err("配置被其他程序修改，导入已停止。".to_string());
            }
            atomic_write(&paths.target(c.kind), c.after.as_bytes())?;
        }
        for c in &changes {
            if read_optional(&paths.target(c.kind))?.as_deref() != Some(c.after.as_str()) {
                return Err("写入后的配置校验失败，正在恢复原配置。".to_string());
            }
        }
        atomic_write(&paths.record(client, false), &baseline_record)?;
        fs::remove_file(&pending)
            .map_err(|_| "配置已写入，但完成标记失败；请使用恢复配置检查。".to_string())?;
        Ok(())
    })();
    if let Err(error) = result {
        return match restore_record(paths, client, &pending) {
            Ok(()) => Err(format!("{error} 已恢复导入前的文件。")),
            Err(_) => Err(format!("{error} 备份已保留，请先点击恢复配置。")),
        };
    }
    Ok(true)
}

pub(crate) fn read_backup(
    paths: &Paths,
    client: Client,
    record: &Path,
) -> Result<(String, Backup), String> {
    let text = read_optional(record)?.ok_or("没有可恢复的配置备份。")?;
    let backup: Backup =
        serde_json::from_str(&text).map_err(|_| "备份文件损坏，未修改任何配置。")?;
    if backup.version != 1
        || backup.client != client
        || backup.entries.is_empty()
        || backup.entries.len() > 4
        || backup.entries.iter().any(|e| !e.kind.belongs_to(client))
    {
        return Err("备份文件与当前工具不匹配，未修改任何配置。".into());
    }
    for (index, e) in backup.entries.iter().enumerate() {
        if backup.entries[..index].iter().any(|old| old.kind == e.kind) {
            return Err("备份包含重复文件，未修改任何配置。".into());
        }
        if e.path != paths.target(e.kind) {
            return Err("客户端配置目录已改变，自动恢复已停止。请切回原目录后恢复。".into());
        }
    }
    Ok((text, backup))
}

fn restore_record(paths: &Paths, client: Client, record: &Path) -> Result<(), String> {
    let (_, backup) = read_backup(paths, client, record)?;
    for e in &backup.entries {
        let current = read_optional(&paths.target(e.kind))?;
        if current != e.before && current.as_deref().map(digest).as_deref() != Some(&e.after_hash) {
            return Err("导入后配置又被修改过，自动恢复已停止，以保留你的新改动。备份仍在 .xxchun-connect 目录。".into());
        }
    }
    for e in backup.entries.iter().rev() {
        let path = paths.target(e.kind);
        let current = read_optional(&path)?;
        if current == e.before {
            continue;
        }
        if current.as_deref().map(digest).as_deref() != Some(&e.after_hash) {
            return Err("恢复时检测到其他程序修改了配置，操作已停止。".into());
        }
        match &e.before {
            Some(text) => atomic_write(&path, text.as_bytes())?,
            None => fs::remove_file(&path).map_err(|_| format!("无法恢复 {}。", path.display()))?,
        }
    }
    fs::remove_file(record).map_err(|_| "原配置已恢复，但备份标记未清理。".to_string())?;
    Ok(())
}

pub fn restore(paths: &Paths, client: Client) -> Result<(), String> {
    let resolved = paths.for_client(client)?;
    let paths = &resolved;
    let client = client.backup_client();
    let _lock = paths.lock()?;
    let pending = paths.record(client, true);
    let record = if pending.exists() {
        pending
    } else {
        paths.record(client, false)
    };
    restore_record(paths, client, &record)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_two_file_import_can_be_recovered() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::for_home(dir.path());
        let original = "{\"tokens\":{\"refresh_token\":\"fake-old-token\"}}";
        atomic_write(&paths.target(FileKind::CodexAuth), b"new-auth").unwrap();
        let backup = Backup {
            version: 1,
            client: Client::Codex,
            created_at: None,
            entries: vec![
                Entry {
                    kind: FileKind::CodexAuth,
                    path: paths.target(FileKind::CodexAuth),
                    before: Some(original.into()),
                    after_hash: digest("new-auth"),
                },
                Entry {
                    kind: FileKind::CodexConfig,
                    path: paths.target(FileKind::CodexConfig),
                    before: None,
                    after_hash: digest("new-config"),
                },
            ],
        };
        atomic_write(
            &paths.record(Client::Codex, true),
            &serde_json::to_vec(&backup).unwrap(),
        )
        .unwrap();
        restore(&paths, Client::Codex).unwrap();
        assert_eq!(
            read_optional(&paths.target(FileKind::CodexAuth))
                .unwrap()
                .as_deref(),
            Some(original)
        );
        assert!(!paths.target(FileKind::CodexConfig).exists());
        assert!(!paths.pending(Client::Codex));
    }
    #[test]
    fn concurrent_writer_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::for_home(dir.path());
        let lock = paths.lock().unwrap();
        assert!(paths.lock().is_err());
        drop(lock);
        assert!(paths.lock().is_ok());
    }
    #[test]
    fn stale_plan_does_not_overwrite_the_newer_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::for_home(dir.path());
        atomic_write(&paths.target(FileKind::ClaudeSettings), b"newer").unwrap();
        assert!(commit(
            &paths,
            Client::Claude,
            vec![Change {
                kind: FileKind::ClaudeSettings,
                before: Some("old".into()),
                after: "target".into()
            }]
        )
        .is_err());
        assert_eq!(
            read_optional(&paths.target(FileKind::ClaudeSettings))
                .unwrap()
                .as_deref(),
            Some("newer")
        );
        assert!(!paths.has_backup(Client::Claude));
    }

    // ---- 0.1.10: a configuration folder or file that is a link ----
    // Making a link: a symbolic link; on Windows, where that needs a privilege a CI runner may not have, a junction
    // for a folder (no privilege needed). `false` = this machine cannot make one, and the test says so and stops.
    #[cfg(unix)]
    fn link_dir(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(unix)]
    fn link_file(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    fn link_dir(target: &Path, link: &Path) -> bool {
        if std::os::windows::fs::symlink_dir(target, link).is_ok() {
            return true;
        }
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    fn link_file(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
    fn same_place(a: &Path, b: &Path) -> bool {
        fs::canonicalize(a).unwrap() == fs::canonicalize(b).unwrap()
    }
    #[test]
    fn a_linked_config_folder_is_followed_written_and_restored_through() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let real = home.join("elsewhere").join("codex-real");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("config.toml"), "old").unwrap();
        let link = home.join(".codex");
        if !link_dir(&real, &link) {
            eprintln!("SKIPPED: this machine cannot create a directory link");
            return;
        }
        let paths = Paths::for_home(home);
        let target = paths.target(FileKind::CodexConfig);
        // The resolved place, not the link: that is what is read, backed up, written and recorded.
        assert!(same_place(target.parent().unwrap(), &real));
        assert!(!target.starts_with(&link));
        assert_eq!(read_optional(&target).unwrap().as_deref(), Some("old"));
        assert!(commit(
            &paths,
            Client::Codex,
            vec![Change {
                kind: FileKind::CodexConfig,
                before: Some("old".into()),
                after: "new".into()
            }]
        )
        .unwrap());
        assert_eq!(fs::read_to_string(real.join("config.toml")).unwrap(), "new");
        // The link is still a link (it was not replaced by a folder), and the record names the resolved file.
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        let record = fs::read_to_string(paths.record(Client::Codex, false)).unwrap();
        let saved: Backup = serde_json::from_str(&record).unwrap();
        assert_eq!(saved.entries[0].path, target);
        restore(&paths, Client::Codex).unwrap();
        assert_eq!(fs::read_to_string(real.join("config.toml")).unwrap(), "old");
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    }
    #[test]
    fn a_linked_config_file_is_followed_and_restored_through() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let real = home.join("dotfiles").join("codex-config.toml");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::write(&real, "old").unwrap();
        fs::create_dir_all(home.join(".codex")).unwrap();
        let link = home.join(".codex").join("config.toml");
        if !link_file(&real, &link) {
            eprintln!("SKIPPED: this machine cannot create a file link (on Windows that needs a privilege)");
            return;
        }
        let paths = Paths::for_home(home);
        let target = paths.target(FileKind::CodexConfig);
        assert!(same_place(&target, &real));
        assert!(commit(
            &paths,
            Client::Codex,
            vec![Change {
                kind: FileKind::CodexConfig,
                before: Some("old".into()),
                after: "new".into()
            }]
        )
        .unwrap());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        restore(&paths, Client::Codex).unwrap();
        assert_eq!(fs::read_to_string(&real).unwrap(), "old");
    }
    #[test]
    fn a_link_that_cannot_be_followed_changes_nothing_and_says_how_to_connect_by_hand() {
        // Dangling: the folder it pointed to is gone.
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let gone = home.join("gone");
        fs::create_dir_all(&gone).unwrap();
        if !link_dir(&gone, &home.join(".codex")) || !link_dir(&gone, &home.join(".claude")) {
            eprintln!("SKIPPED: this machine cannot create a directory link");
            return;
        }
        fs::remove_dir(&gone).unwrap();
        let paths = Paths::for_home(home);
        let target = paths.target(FileKind::CodexConfig);
        assert_eq!(read_optional(&target).unwrap_err(), MOVED_CODEX);
        let refused = commit(
            &paths,
            Client::Codex,
            vec![Change {
                kind: FileKind::CodexConfig,
                before: None,
                after: "new".into(),
            }],
        )
        .unwrap_err();
        assert!(refused.contains("你的 Codex 配置文件夹被放到了别的位置，我们没能安全地找到它，所以没有改动任何东西。"));
        assert!(refused.contains("https://api.xxchunai.com/v1"));
        assert!(!gone.exists());
        assert!(!paths.has_backup(Client::Codex));
        assert_eq!(
            read_optional(&paths.target(FileKind::ClaudeSettings)).unwrap_err(),
            MOVED_CLAUDE
        );
        assert!(MOVED_CLAUDE.contains("https://api.xxchunai.com ，"));
        // A place that is not the user's own: a folder outside the user's folder (on the same drive).
        let outside = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let home = other.path();
        fs::write(outside.path().join("config.toml"), "theirs").unwrap();
        if !link_dir(outside.path(), &home.join(".codex")) {
            return;
        }
        let paths = Paths::for_home(home);
        assert_eq!(
            read_optional(&paths.target(FileKind::CodexConfig)).unwrap_err(),
            MOVED_CODEX
        );
        assert_eq!(
            fs::read_to_string(outside.path().join("config.toml")).unwrap(),
            "theirs"
        );
        // A folder link that points at a file is not a configuration folder.
        let third = tempfile::tempdir().unwrap();
        let home = third.path();
        fs::write(home.join("a-file"), "x").unwrap();
        if link_file(&home.join("a-file"), &home.join(".codex")) {
            let paths = Paths::for_home(home);
            assert!(read_optional(&paths.target(FileKind::CodexConfig)).is_err());
        }
    }
}
