import assert from "node:assert/strict";
import test from "node:test";
import { DEFAULT_STATION, stationAddress } from "../src/stations.ts";

test("the only relay is xxchun's, by its Anthropic-format base address", () => {
  assert.deepEqual(DEFAULT_STATION, { id: "xxchun", name: "xxchun", baseUrl: "https://api.xxchun.com" });
  assert.equal(stationAddress(DEFAULT_STATION), "api.xxchun.com");
});
