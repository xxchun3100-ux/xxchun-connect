import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync, readdirSync, existsSync } from "node:fs";
const read = name => readFileSync(new URL("../" + name, import.meta.url), "utf8");
const versions = [JSON.parse(read("package.json")).version, JSON.parse(read("src-tauri/tauri.conf.json")).version, /^version = "([^"]+)"/m.exec(read("src-tauri/Cargo.toml"))[1], /name = "xxchun-connect"\r?\nversion = "([^"]+)"/.exec(read("src-tauri/Cargo.lock"))[1]];

test("one version in the four places the build reads it", () => assert.deepEqual(versions, Array(4).fill(versions[0])));
test("what people see says xxchunai Connect, and names upstream only where credit is given", () => {
  const conf = JSON.parse(read("src-tauri/tauri.conf.json"));
  assert.deepEqual([conf.productName, conf.identifier, conf.app.windows[0].title], ["xxchunai Connect", "com.xxchun.connect", "xxchunai Connect"]);
  for (const name of readdirSync(new URL("../src/", import.meta.url))) {
    const text = read("src/" + name).replace(/SuperGPT Connect（MIT，github\.com\/superpilot69\/supergpt-connect）/g, "").replace(/^\/\/.*$/gm, "");
    assert.doesNotMatch(text, /supergpt/i, name);
    assert.doesNotMatch(text, /添加站点|自定义站点|中转站/, name + " offers another relay");
  }
  assert.equal(existsSync(new URL("../src/StationEditor.tsx", import.meta.url)), false);
  assert.match(read("src-tauri/src/storage.rs"), /\.xxchun-connect/);
  for (const kept of ["LICENSE", "UPSTREAM.md", "licenses/CC-Switch-MIT.txt", "NOTICE.md"]) assert.ok(existsSync(new URL("../" + kept, import.meta.url)), kept);
});
test("the release workflow builds both zips under the names the shop links to", () => {
  const flow = read(".github/workflows/release.yml");
  assert.match(flow, /tags:\s*\n\s*- ['"]v\*['"]/); assert.match(flow, /SHA256SUMS/);
  assert.match(read("scripts/build-windows-portable.mjs"), /xxchunai-Connect-\$\{version\}-Windows-x64-portable\.zip/);
  assert.match(read("scripts/build-macos.mjs"), /xxchunai-Connect-\$\{version\}-macOS-arm64\.zip/);
});
test("the old display name is gone from what ships, and what the app recognises its own work by is unchanged", () => {
  const shipped = [".github/workflows/release.yml", "ci/build.yml", "index.html", "scripts/build-windows-portable.mjs", "scripts/build-macos.mjs", "src-tauri/tauri.conf.json",
    ...readdirSync(new URL("../src/", import.meta.url)).map(name => "src/" + name),
    ...readdirSync(new URL("../src-tauri/src/", import.meta.url)).filter(name => name.endsWith(".rs")).map(name => "src-tauri/src/" + name)];
  for (const name of shipped) assert.doesNotMatch(read(name).replace(/^\s*\/\/.*$/gm, ""), /xxchun[ -]Connect/, name);
  const adapter = read("src-tauri/src/adapter.rs");
  assert.ok(adapter.includes('provider.insert("name", value("xxchunai Connect"))'));
  assert.ok(adapter.includes('("model_provider", "xxchun-connect")'));
  assert.ok(adapter.includes('KeyPath::new(&["model_providers", "xxchun-connect"])'));
  assert.ok(read("src-tauri/src/desktop_config.rs").includes('"name": "xxchunai Connect"'));
  assert.ok(read("src-tauri/src/storage.rs").includes('join("xxchun-connect-models.json")'));
  assert.ok(read("src-tauri/src/storage.rs").includes('home.join(".xxchun-connect")'));
  assert.ok(read("src/App.tsx").includes('"xxchun-connect.theme"'));
  assert.ok(read("src/quick.ts").includes("/^sk-xc-[0-9A-Za-z]{32}$/"));
});
