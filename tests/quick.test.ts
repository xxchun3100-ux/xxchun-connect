import assert from "node:assert/strict";
import test from "node:test";
import { cleanKey, DEFAULT_MODEL_NOTE, DEFAULT_MODEL_QUESTION, KEY_HELP, KEY_MASKED, keyConfirmation, keyProblem, looksLikeKey, modelChoices, pickModels, splitError, wrongKeyMessage } from "../src/quick.ts";

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
// 0.1.10: the customer chooses the default. What is preselected (and marked 「推荐」) is the everyday model, never
// simply the first one listed: the shop lists its dearest Codex model first.
test("the recommended default: gpt-6.1-sol for Codex when it is offered, else the second listed, else the first; the Sonnet for Claude", () => {
  const shop = ["gpt-6.2-astra", "gpt-6.2-sol", "gpt-6.1-sol", "gpt-6.1-terra", "gpt-image-2"];
  assert.deepEqual(pickModels("codex-desktop", shop), { ids: ["gpt-6.2-astra", "gpt-6.2-sol", "gpt-6.1-sol", "gpt-6.1-terra"], defaultId: "gpt-6.1-sol" });
  assert.equal(pickModels("codex-cli", ["gpt-6.2-astra", "gpt-6.2-sol", "gpt-6.1-terra"]).defaultId, "gpt-6.2-sol");
  assert.equal(pickModels("codex-desktop", ["gpt-6.2-astra"]).defaultId, "gpt-6.2-astra");
  assert.equal(pickModels("claude-code", ["claude-fable-5-2", "claude-opus-5-5", "claude-sonnet-5-5"]).defaultId, "claude-sonnet-5-5");
  assert.equal(pickModels("claude-code", ["claude-fable-5-2", "claude-opus-5-5"]).defaultId, "claude-fable-5-2");
});
test("the choice the customer is shown: every usable model under the shop's own name (the id where it has none), one of them recommended; every one of them is still written", () => {
  const found = [{ id: "gpt-6.2-astra", name: "GPT-6.2 Astra" }, { id: "gpt-6.1-sol", name: "GPT-6.1 Sol" }, { id: "gpt-6.1-terra", name: "" }, { id: "gpt-image-2", name: "GPT Image 2" }, { id: "gpt-6.1-sol", name: "GPT-6.1 Sol" }, { id: "gpt-6.2-sol", name: "   " }];
  const shown = modelChoices("codex-desktop", found);
  assert.deepEqual(shown.choices, [{ id: "gpt-6.2-astra", name: "GPT-6.2 Astra", recommended: false }, { id: "gpt-6.1-sol", name: "GPT-6.1 Sol", recommended: true }, { id: "gpt-6.1-terra", name: "gpt-6.1-terra", recommended: false }, { id: "gpt-6.2-sol", name: "gpt-6.2-sol", recommended: false }]);
  assert.equal(shown.recommendedId, "gpt-6.1-sol"); assert.deepEqual(shown.ids, ["gpt-6.2-astra", "gpt-6.1-sol", "gpt-6.1-terra", "gpt-6.2-sol"]);
  // A model the client cannot use (the capability check of the caller) is not offered and not written.
  assert.deepEqual(modelChoices("claude-desktop", [{ id: "claude-opus-5-5", name: "Claude Opus 5.5" }, { id: "claude-sonnet-5-5", name: "Claude Sonnet 5.5" }], (id) => id !== "claude-sonnet-5-5"), { choices: [{ id: "claude-opus-5-5", name: "Claude Opus 5.5", recommended: true }], ids: ["claude-opus-5-5"], recommendedId: "claude-opus-5-5" });
  assert.deepEqual(modelChoices("claude-desktop", [{ id: "gpt-6.1-sol", name: "GPT-6.1 Sol" }]), { choices: [], ids: [], recommendedId: "" });
  // The words of the step: a question, and one line that the choice is not final. No price anywhere.
  assert.equal(DEFAULT_MODEL_QUESTION, "默认用哪个模型？"); assert.equal(DEFAULT_MODEL_NOTE, "接好以后在 Codex / Claude 里也能随时换。");
  assert.doesNotMatch(JSON.stringify(shown) + DEFAULT_MODEL_NOTE, /[¥$￥]|元|价/);
});
test("a key with nothing for this tool says to copy the key again", () => {
  assert.equal(wrongKeyMessage("codex-desktop"), "这把密钥用不了 Codex 的模型。回教程页重新点一次『复制我的密钥』。");
  assert.equal(wrongKeyMessage("claude-vscode"), "这把密钥用不了 Claude 的模型。回教程页重新点一次『复制我的密钥』。");
});
test("an error is split into what to do and the technical cause", () => {
  assert.deepEqual(splitError("连不上 xxchunai（api.xxchun.com）。请暂时关掉代理/VPN 和杀毒软件的\"HTTPS 扫描/网页防护\"再试，或换个网络。\nconnect: refused"), { message: "连不上 xxchunai（api.xxchun.com）。请暂时关掉代理/VPN 和杀毒软件的\"HTTPS 扫描/网页防护\"再试，或换个网络。", detail: "connect: refused" });
  assert.deepEqual(splitError("密钥未通过验证，请检查是否复制完整。"), { message: "密钥未通过验证，请检查是否复制完整。", detail: "" });
});
