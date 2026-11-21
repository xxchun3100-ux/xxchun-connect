// The choices the app makes for a beginner, kept apart from the screen so they can be tested with node.
import { clientFamily, modelSelectable } from "./clients.ts";
import { MAX_MODELS } from "./selection.ts";
import type { Client } from "./native";

export const looksLikeKey = (text: string) => /^sk-[A-Za-z0-9_-]{16,}$/.test(text.trim());
// Every model the key offers that the client can use; the default is, for Claude, the Sonnet (the everyday one),
// otherwise the first one the shop lists.
export function pickModels(client: Client, ids: string[]): { ids: string[]; defaultId: string } {
  const usable = [...new Set(ids)].filter((id) => modelSelectable(client, id)).slice(0, MAX_MODELS);
  const preferred = clientFamily(client) === "claude" ? usable.find((id) => /sonnet/i.test(id)) : undefined;
  return { ids: usable, defaultId: preferred ?? usable[0] ?? "" };
}
