import assert from "node:assert/strict";
import test from "node:test";
import { looksLikeKey, pickModels } from "../src/quick.ts";

test("a pasted text is taken for a key only when it looks like one", () => {
  assert.equal(looksLikeKey("  sk-xc-" + "A1b2".repeat(8) + " "), true);
  for (const text of ["", "hello", "https://xxchun.com", "sk-short", "sk-xc-has space inside 0000000000"]) assert.equal(looksLikeKey(text), false, text);
});
test("the app picks the models itself: all the client can use, Sonnet first for Claude", () => {
  const offered = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "gpt-6.1-sol", "gpt-image-2", "claude-sonnet-5-5"];
  assert.deepEqual(pickModels("claude-desktop", offered), { ids: ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"], defaultId: "claude-sonnet-5-5" });
  assert.deepEqual(pickModels("codex-desktop", ["gpt-6.1-sol", "gpt-6.1-mini", "gpt-image-2"]), { ids: ["gpt-6.1-sol", "gpt-6.1-mini"], defaultId: "gpt-6.1-sol" });
  assert.deepEqual(pickModels("claude-desktop", ["gpt-6.1-sol"]), { ids: [], defaultId: "" });
});
