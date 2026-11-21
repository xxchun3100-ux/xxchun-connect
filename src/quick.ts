// The choices the app makes for a beginner, kept apart from the screen so they can be tested with node.
import { clientFamily, modelSelectable } from "./clients.ts";
import { MAX_MODELS } from "./selection.ts";
import type { Client } from "./native";

export const looksLikeKey = (text: string) => /^sk-[A-Za-z0-9_-]{16,}$/.test(text.trim());
// A key of this shop is for one tool. Used on the other one it offers nothing that tool can use.
export function wrongKeyMessage(client: Client): string {
  const [wants, has] = clientFamily(client) === "codex" ? ["Codex", "Claude"] : ["Claude", "Codex"];
  return `这把密钥只能用 ${has} 的模型，接 ${wants} 请回教程页复制 ${wants} 的密钥。`;
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
