import assert from "node:assert/strict";
import test from "node:test";
import { looksLikeKey, pickModels, splitError, wrongKeyMessage } from "../src/quick.ts";

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
test("a key for the other tool is named, with the key to copy instead", () => {
  assert.equal(wrongKeyMessage("codex-desktop"), "这把密钥只能用 Claude 的模型，接 Codex 请回教程页复制 Codex 的密钥。");
  assert.equal(wrongKeyMessage("claude-vscode"), "这把密钥只能用 Codex 的模型，接 Claude 请回教程页复制 Claude 的密钥。");
});
test("an error is split into what to do and the technical cause", () => {
  assert.deepEqual(splitError("连不上 xxchun（api.xxchun.com）。请关掉代理/VPN 再试一次，或换个网络。\nconnect: refused"), { message: "连不上 xxchun（api.xxchun.com）。请关掉代理/VPN 再试一次，或换个网络。", detail: "connect: refused" });
  assert.deepEqual(splitError("密钥未通过验证，请检查是否复制完整。"), { message: "密钥未通过验证，请检查是否复制完整。", detail: "" });
});
