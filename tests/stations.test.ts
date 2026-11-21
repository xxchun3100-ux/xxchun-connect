import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { DEFAULT_STATION, ENTRIES, SHOP_SITE, entryHost, keyDestinations, stationAddress } from "../src/stations.ts";

test("the only relay is xxchun's, asked for by its names in order", () => {
  assert.deepEqual(ENTRIES, ["https://api.xxchunai.com", "https://api.xxchun.com"]);
  assert.deepEqual(DEFAULT_STATION, { id: "xxchun", name: "xxchun", baseUrl: "https://api.xxchunai.com" });
  assert.equal(stationAddress(DEFAULT_STATION), "api.xxchunai.com");
  assert.equal(SHOP_SITE, "xxchun.com");
});
test("the lock line names the address that answered, or every address that may be asked", () => {
  assert.equal(keyDestinations(""), "api.xxchunai.com 或 api.xxchun.com");
  assert.equal(keyDestinations("https://api.xxchun.com"), "api.xxchun.com");
  assert.equal(entryHost("http://127.0.0.1:8080"), "127.0.0.1:8080");
});
test("the native side has the same entries in the same order", () => {
  const rust = readFileSync(new URL("../src-tauri/src/models.rs", import.meta.url), "utf8");
  const list = /pub const BUILT_IN_ENTRIES: \[&str; (\d+)\] = \[([^\]]+)\];/.exec(rust);
  assert.ok(list, "BUILT_IN_ENTRIES not found");
  assert.deepEqual(JSON.parse("[" + list[2] + "]"), ENTRIES);
  assert.equal(Number(list[1]), ENTRIES.length);
});
