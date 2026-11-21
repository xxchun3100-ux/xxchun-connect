// xxchun Connect: the upstream wizard (pick a relay → pick a client → key → pick models → import) cut down to what a
// beginner of this shop needs: paste the key, press 「接到 Claude」 or 「接到 Codex」, restart the client. The relay is
// always this shop's; the models are fetched and chosen by the app (every model the key offers that the client can
// use). The native side (discover → preview → import, restore) is upstream's, unchanged.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Check, CircleAlert, ClipboardPaste, Eye, EyeOff, KeyRound, LoaderCircle, LockKeyhole, Moon, RotateCcw, Sun, Terminal, X } from "lucide-react";
import { discoverModels, importConfig, nativeAvailable, previewConfig, readStatus, type Client, type ImportResult, type SelectionInput, type Status } from "./native";
import { DEFAULT_STATION, stationAddress } from "./stations";
import { clients, clientFamily } from "./clients";
import { cleanKey, keyConfirmation, keyProblem, pickModels, splitError, wrongKeyMessage } from "./quick";
import RestorePanel from "./RestorePanel";
import { DEFAULT_OPTIONS, selectedOptions } from "./setup-options";
import { version } from "../package.json";

export const SHOP_KEY_PAGE = "xxchun.com/account?tab=api";
const MAIN: Client[] = ["claude-desktop", "codex-desktop"];
const MORE: Client[] = ["claude", "claude-vscode", "codex"];
const PASTE_TIP = "在下面的框里点一下，按 Ctrl+V（Mac 按 ⌘V）粘贴。";
const shortName = (client: Client) => (clientFamily(client) === "codex" ? "Codex" : "Claude");
function BrandIcon() {
  return <img className="brand-icon" src="/xxchun.png" alt="" draggable={false} />;
}
function ClientIcon({ client, size = 26 }: { client: Client; size?: number }) {
  return clientFamily(client) === "codex" ? <Terminal size={size} strokeWidth={1.6} /> : (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true">
      {[0, 30, 60, 90, 120, 150].map((angle) => <path key={angle} d="M12 2.5V21.5" stroke="currentColor" strokeWidth="1.8" transform={`rotate(${angle} 12 12)`} />)}
    </svg>
  );
}
function Modal({ title, children, close }: { title: string; children: ReactNode; close: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { const node = ref.current!; node.showModal(); return () => node.close(); }, []);
  return (
    <dialog ref={ref} className="station-dialog" aria-label={title} onCancel={close}>
      <div className="modal-title"><h2>{title}</h2><button className="icon-button" onClick={close} aria-label="关闭对话框"><X size={18} /></button></div>
      {children}
    </dialog>
  );
}
function initialTheme(): "light" | "dark" {
  try { return localStorage.getItem("xxchun-connect.theme") === "dark" ? "dark" : "light"; } catch { return "light"; }
}

export default function App() {
  const [theme, setTheme] = useState(initialTheme);
  const [apiKey, setApiKey] = useState("");
  const [visible, setVisible] = useState(false);
  const [busy, setBusy] = useState<Client | null>(null);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState<Client | null>(null);
  const [hint, setHint] = useState("");
  const [result, setResult] = useState<ImportResult | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [more, setMore] = useState(false);
  const [showRestore, setShowRestore] = useState(false);
  const [restored, setRestored] = useState(false);
  const [about, setAbout] = useState(false);
  const field = useRef<HTMLInputElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const disabled = busy !== null;
  const ready = apiKey.trim().length > 0;
  const pending = status?.clients.some((item) => item.pending) ?? false;

  async function refresh() { if (nativeAvailable) setStatus(await readStatus()); }
  useEffect(() => { refresh().catch(() => setError("无法读取电脑上的状态，请重新打开 xxchun Connect。")); }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem("xxchun-connect.theme", theme); } catch { /* this session still has the theme */ }
  }, [theme]);
  useEffect(() => { window.scrollTo({ top: 0 }); heading.current?.focus({ preventScroll: true }); }, [result]);

  // 「粘贴」: the key copied on the shop's page. Where the system does not let the app read the clipboard, the field
  // is focused and the person presses Ctrl+V / ⌘V themselves.
  async function paste() {
    setHint("");
    try {
      const text = (await navigator.clipboard.readText()).trim();
      if (!text) { setHint("剪贴板是空的。先到网站点「复制我的密钥」。"); field.current?.focus(); return; }
      const problem = keyProblem(text);
      if (problem) { setHint(problem); return; }
      setApiKey(cleanKey(text)); setResult(null); setRestored(false);
    } catch { setHint(PASTE_TIP); field.current?.focus(); }
  }
  async function connect(client: Client) {
    if (disabled || !ready) return;
    // Nothing is sent until the text has the shape of a key (the native side checks again).
    const problem = keyProblem(apiKey);
    if (problem) { setHint(problem); field.current?.focus(); return; }
    setError(""); setRetry(null); setHint(""); setResult(null); setRestored(false);
    if (!nativeAvailable) { setError("这是界面预览，要在 xxchun Connect 应用里才能接入。"); return; }
    setBusy(client);
    try {
      const found = await discoverModels({ client, baseUrl: DEFAULT_STATION.baseUrl, apiKey: cleanKey(apiKey) });
      const chosen = pickModels(client, found.models.map((model) => model.id).filter((id) => clientFamily(client) === "codex" || found.capabilities?.models[id]?.messages.status !== "unsupported"));
      if (!chosen.ids.length) { setError(wrongKeyMessage(client)); return; }
      const input: SelectionInput = { discoveryId: found.discoveryId, selectedModels: chosen.ids, defaultModel: chosen.defaultId, toolSearch: "auto", options: selectedOptions(client, chosen.ids, DEFAULT_OPTIONS) };
      const preview = await previewConfig(input);
      if (preview.blocked) { setError(preview.blocked); return; }
      setResult(await importConfig(input));
      try { await refresh(); } catch { /* the import itself is done */ }
    } catch (reason) {
      setError(typeof reason === "string" ? reason : "没接上，请稍后再试。"); setRetry(client);
    } finally { setBusy(null); }
  }
  const target = (client: Client, big: boolean) => {
    const info = clients.find((item) => item.id === client)!, state = status?.clients.find((item) => item.client === client);
    const blocked = state?.compatibility.blocked ?? null, missing = state ? !state.compatibility.installed : false;
    return (
      <button key={client} type="button" className={big ? "xc-target" : "xc-target xc-target-small"} data-client={client} disabled={disabled || !ready || !!blocked} onClick={() => void connect(client)}>
        {busy === client ? <LoaderCircle className="spinning" size={big ? 26 : 18} /> : <ClientIcon client={client} size={big ? 26 : 18} />}
        <span><strong>{busy === client ? "正在接入…" : big ? `接到 ${shortName(client)}` : `接到 ${info.name}`}</strong>
          <small>{blocked ?? (missing ? `这台电脑上还没找到 ${info.name}，先装好它` : big ? info.name : info.hint)}</small></span>
      </button>
    );
  };
  const done = result ? clients.find((item) => item.id === result.client)! : null;
  const shown = splitError(error);
  return (
    <main className="app-shell">
      <header className="app-header">
        <div className="brand"><BrandIcon /><span className="brand-wordmark">xxchun</span><span className="brand-divider" /><span className="brand-product">Connect</span></div>
        <div className="header-actions">
          <button className="restore-header-button" onClick={() => setShowRestore(true)} disabled={disabled}><RotateCcw size={15} /><span>恢复原配置</span></button>
          <button className="icon-button theme-button" onClick={() => setTheme(theme === "light" ? "dark" : "light")} aria-label={theme === "light" ? "切换深色外观" : "切换浅色外观"}>{theme === "light" ? <Moon size={17} /> : <Sun size={17} />}</button>
        </div>
      </header>
      <div className="workspace">
        {(!nativeAvailable || status?.sandbox) && <div className="preview-label">{!nativeAvailable ? "界面预览 · 接入要在应用里做" : "测试模式 · 使用独立配置目录"}</div>}
        <section className="wizard-card xc-card" aria-labelledby="step-title" aria-busy={disabled}>
          {result && done ? (
            <div className="success-content" role="status">
              <span className="success-mark"><Check size={28} strokeWidth={1.7} /></span>
              <h2 id="step-title" ref={heading} tabIndex={-1}>好了，完全退出 {shortName(result.client)} 再打开</h2>
              <p>{done.name} 已经接到 xxchun。只关窗口不算退出。</p>
              <div className="activation-guide">
                <strong>接下来</strong>
                <ol>{result.nextSteps.map((text) => <li key={text}>{text}</li>)}</ol>
                <p>然后回到网站的接入教程，对它说一句话；它回答了，网站会显示「接通了」。</p>
              </div>
              {result.warnings.length > 0 && <details className="compatibility-details"><summary>兼容性提示</summary>{result.warnings.map((text) => <p key={text}>{text}</p>)}</details>}
              <button className="primary-button xc-again" onClick={() => setResult(null)}>再接一个</button>
            </div>
          ) : (
            <>
              <div className="card-heading"><h2 id="step-title" ref={heading} tabIndex={-1}>{ready ? "② 选一个，点一下" : "① 粘贴你的密钥"}</h2>
                <p>{ready ? "它会自己读取能用的模型并写好配置，原来的配置先备份。" : `密钥在 ${SHOP_KEY_PAGE} 的「接入教程」里，点「复制我的密钥」。`}</p></div>
              <button type="button" className="primary-button xc-paste" onClick={() => void paste()} disabled={disabled}><ClipboardPaste size={20} />粘贴密钥</button>
              <label className="field-label" htmlFor="api-key">密钥</label>
              <div className="secret-field">
                <KeyRound size={18} />
                <input id="api-key" name="api-key" ref={field} type={visible ? "text" : "password"} value={apiKey} onChange={(event) => { setApiKey(event.target.value); setHint(""); }} placeholder="sk-xc-…" autoComplete="off" autoCapitalize="none" spellCheck={false} maxLength={4096} disabled={disabled} aria-describedby="key-destination" />
                <button type="button" className="icon-button" aria-label={visible ? "隐藏密钥" : "显示密钥"} aria-pressed={visible} onClick={() => setVisible(!visible)} disabled={disabled}>{visible ? <EyeOff size={17} /> : <Eye size={17} />}</button>
              </div>
              <p id="key-destination" className="key-destination"><LockKeyhole size={13} /><span>密钥只发给 <b>{stationAddress(DEFAULT_STATION)}</b>，并写进你电脑上的配置</span></p>
              {hint ? <p className={hint === PASTE_TIP ? "xc-hint" : "xc-hint xc-hint-bad"} role={hint === PASTE_TIP ? "status" : "alert"}>{hint}</p>
                : keyConfirmation(apiKey) && <p className="xc-hint xc-key-ok" role="status"><Check size={14} />{keyConfirmation(apiKey)}</p>}
              <div className="xc-targets">{MAIN.map((client) => target(client, true))}</div>
              <button type="button" className="text-button xc-more-open" aria-expanded={more} onClick={() => setMore(!more)}>{more ? "收起" : "更多：Claude Code、VS Code、终端里的 Codex"}</button>
              {more && <div className="xc-targets xc-targets-more">{MORE.map((client) => target(client, false))}</div>}
            </>
          )}
          {(error || pending || restored) && (
            <div className="feedback-area">
              {pending && <div className="notice notice-error" role="alert"><CircleAlert size={16} /><p>上次接入被中断了，请先点右上角「恢复原配置」。</p></div>}
              {error && (
                <div className="notice notice-error xc-error" role="alert"><CircleAlert size={16} />
                  <div className="xc-error-body"><p>{shown.message}</p>
                    {shown.detail && <p className="xc-error-detail">{shown.detail}</p>}
                    {retry && <button type="button" className="xc-retry" disabled={disabled || !ready} onClick={() => void connect(retry)}>{busy === retry ? "正在重试…" : "再试一次"}</button>}</div>
                  <button className="icon-button" aria-label="关闭提示" onClick={() => { setError(""); setRetry(null); }}><X size={15} /></button>
                </div>
              )}
              {restored && !error && <div className="notice" role="status"><Check size={16} /><p>已经恢复成接入之前的配置。完全退出那个程序再打开，它会回到官方登录。</p></div>}
            </div>
          )}
        </section>
      </div>
      <footer className="app-footer"><span>xxchun Connect</span><button onClick={() => setAbout(true)} className="version-button">v{version} <span>·</span> 关于</button></footer>
      {showRestore && <RestorePanel initialClient={result?.client ?? "claude-desktop"} onClose={() => setShowRestore(false)} onRestored={async () => { setResult(null); setRestored(true); await refresh(); }} />}
      {about && (
        <Modal title="xxchun Connect" close={() => setAbout(false)}>
          <p className="dialog-intro">v{version} · xxchun 的本地接入小工具</p>
          <p className="about-copy">基于开源项目 SuperGPT Connect（MIT，github.com/superpilot69/supergpt-connect），其配置引擎来自 CC Switch（MIT，原作者 Jason Young）。完整许可随应用附带。</p>
          <p className="about-copy">密钥只用于向 {stationAddress(DEFAULT_STATION)} 读取模型，以及写进你电脑上的客户端配置；不保存在这个应用里。</p>
          <div className="dialog-actions"><button className="primary-button" onClick={() => setAbout(false)}>知道了</button></div>
        </Modal>
      )}
    </main>
  );
}
