// The choices the app makes for a beginner, kept apart from the screen so they can be tested with node.
import { clientFamily, modelSelectable } from "./clients.ts";
import { MAX_MODELS } from "./selection.ts";
import type { Client } from "./native";

export const KEY_HELP = "这不是完整的密钥。回到 xxchunai 的接入教程，点『复制我的密钥』，再回来点『粘贴密钥』。";
export const KEY_MASKED = "你复制的是打了码的密钥（中间是点），不能用。";
// Spaces and quotes around a key come from copying it; they are not part of it.
export const cleanKey = (text: string) => text.replace(/^[\s"'`“”‘’]+|[\s"'`“”‘’]+$/g, "");
// A key of this shop is exactly sk-xc- and 32 letters or digits. The native side checks the same thing.
export const looksLikeKey = (text: string) => /^sk-xc-[0-9A-Za-z]{32}$/.test(cleanKey(text));
export const keyProblem = (text: string): string | null => (looksLikeKey(text) ? null : /[·•…*]/.test(text) ? KEY_MASKED + KEY_HELP : KEY_HELP);
// Lets the person see which key is in the field without showing it.
export const keyConfirmation = (text: string) => (looksLikeKey(text) ? `密钥格式正确（sk-xc-…${cleanKey(text).slice(-4)}）` : "");
// The key offers nothing this tool can use (the shop issues one key for both tools, so copy it again).
export function wrongKeyMessage(client: Client): string {
  return `这把密钥用不了 ${clientFamily(client) === "codex" ? "Codex" : "Claude"} 的模型。回教程页重新点一次『复制我的密钥』。`;
}
// The native side puts what to do on the first line and the technical cause on the lines after it.
export function splitError(text: string): { message: string; detail: string } {
  const [message, ...rest] = text.split("\n");
  return { message: message.trim(), detail: rest.join(" ").trim() };
}
// Every model the key offers that the client can use; the default is, for Claude, the Sonnet (the everyday one),
// otherwise the first one the shop lists.
export function pickModels(client: Client, ids: string[]): { ids: string[]; defaultId: string } {
  const usable = [...new Set(ids)].filter((id) => modelSelectable(client, id)).slice(0, MAX_MODELS);
  const preferred = clientFamily(client) === "claude" ? usable.find((id) => /sonnet/i.test(id)) : undefined;
  return { ids: usable, defaultId: preferred ?? usable[0] ?? "" };
}
