import assert from "node:assert/strict";
import test from "node:test";
import { cleanKey, KEY_HELP, KEY_MASKED, keyConfirmation, keyProblem, looksLikeKey, pickModels, splitError, wrongKeyMessage } from "../src/quick.ts";

test("only a complete shop key passes; a masked one is named", () => {
  const key = "sk-xc-" + "A1b2".repeat(8);
  for (const text of [key, `  "${key}"\r\n`, ` “${key}” `]) { assert.equal(looksLikeKey(text), true, text); assert.equal(keyProblem(text), null); assert.equal(cleanKey(text), key); }
  for (const text of ["", "hello", "sk-xc-short", "sk-test-not-a-real-key", "https://xxchun.com/account?tab=api", key.slice(0, -1), key + "0", "sk-xc-" + "A1b_".repeat(8), "Bearer " + key]) assert.equal(keyProblem(text), KEY_HELP, text);
  for (const text of ["sk-xc-····xT4d", "sk-xc-••••xT4d", "sk-xc-…xT4d", "sk-xc-****xT4d"]) assert.equal(keyProblem(text), KEY_MASKED + KEY_HELP, text);
  assert.equal(keyConfirmation(` ${key} `), "密钥格式正确（sk-xc-…A1b2）");
  assert.equal(keyConfirmation("sk-xc-····xT4d"), "");
});
test("the app picks the models itself: all the client can use, Sonnet first for Claude", () => {
  const offered = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "gpt-6.1-sol", "gpt-image-2", "claude-sonnet-5-5"];
  assert.deepEqual(pickModels("claude-desktop", offered), { ids: ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"], defaultId: "claude-sonnet-5-5" });
  assert.deepEqual(pickModels("codex-desktop", ["gpt-6.1-sol", "gpt-6.1-mini", "gpt-image-2"]), { ids: ["gpt-6.1-sol", "gpt-6.1-mini"], defaultId: "gpt-6.1-sol" });
  assert.deepEqual(pickModels("claude-desktop", ["gpt-6.1-sol"]), { ids: [], defaultId: "" });
});
test("a key with nothing for this tool says to copy the key again", () => {
  assert.equal(wrongKeyMessage("codex-desktop"), "这把密钥用不了 Codex 的模型。回教程页重新点一次『复制我的密钥』。");
  assert.equal(wrongKeyMessage("claude-vscode"), "这把密钥用不了 Claude 的模型。回教程页重新点一次『复制我的密钥』。");
});
test("an error is split into what to do and the technical cause", () => {
  assert.deepEqual(splitError("连不上 xxchunai（api.xxchun.com）。请暂时关掉代理/VPN 和杀毒软件的\"HTTPS 扫描/网页防护\"再试，或换个网络。\nconnect: refused"), { message: "连不上 xxchunai（api.xxchun.com）。请暂时关掉代理/VPN 和杀毒软件的\"HTTPS 扫描/网页防护\"再试，或换个网络。", detail: "connect: refused" });
  assert.deepEqual(splitError("密钥未通过验证，请检查是否复制完整。"), { message: "密钥未通过验证，请检查是否复制完整。", detail: "" });
});
