// Installs the packed tarball into a clean consumer and loads the addon from
// Node and Deno, as a user of the published package would.
import {
  copyFile,
  mkdir,
  mkdtemp,
  readdir,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { build } from "esbuild";
import { join, resolve } from "node:path";
import { nativeTarget } from "../src/native";
import { denoCommand } from "./deno";
import { npmCommand } from "./npm";

const root = resolve(import.meta.dir, ".."),
  // Deno matches permissions against long Windows paths, not 8.3 names like RUNNER~1.
  dir = await realpath(
    await mkdtemp(join(tmpdir(), "now-playing-media-consumer-")),
  );
async function run(cmd: string[]) {
  const child = Bun.spawn(cmd, {
    cwd: dir,
    stdout: "inherit",
    stderr: "inherit",
  });
  if ((await child.exited) !== 0)
    throw new Error(`Consumer failed: ${cmd.join(" ")}`);
}
const consumer = `
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import * as media from "now-playing-media";
assert.equal(media.nativeTarget(), ${JSON.stringify(nativeTarget())});
assert.ok(existsSync(media.nativeBindingPath()));
// The addon validates input itself, so this proves it loaded.
await assert.rejects(() => media.nowPlaying.control("bad", ""), (error) =>
  error instanceof media.NowPlayingError && error.code === "INVALID_ARGUMENT");
await assert.rejects(() => media.seek(-1), { code: "INVALID_ARGUMENT" });
try {
  const sessions = await media.sessions();
  assert.ok(Array.isArray(sessions));
  console.log("sessions:", sessions.length);
} catch (error) {
  // CI machines may lack a session bus or media service; macOS always has one
  // and must reach MediaRemote through the bundled adapter.
  assert.ok(error instanceof media.NowPlayingError, String(error));
  assert.notEqual(process.platform, "darwin", error.message);
  console.log("media service unavailable:", error.code);
}
console.log("consumer OK");
`;
// The sidecar from the installed package, driven through the bundle-safe client.
const sidecar = `
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { sidecarPath } from "now-playing-media";
import { createSidecarClient, NowPlayingError } from "now-playing-media/client";
const child = spawn(sidecarPath(), [], { stdio: ["pipe", "pipe", "inherit"] });
const lines = createInterface({ input: child.stdout });
const pending = new Map();
let nextId = 0;
lines.on("line", (line) => {
  const response = JSON.parse(line);
  const settle = pending.get(response.id);
  pending.delete(response.id);
  if (response.ok) settle.resolve(response.value);
  else settle.reject(Object.assign(new Error(response.error.message), { code: response.error.code }));
});
const call = (method, params) => new Promise((resolve, reject) => {
  const id = ++nextId;
  pending.set(id, { resolve, reject });
  child.stdin.write(JSON.stringify({ id, method, params }) + "\\n");
});
const client = createSidecarClient(call);
await assert.rejects(() => client.control("seek", "", -1), (error) =>
  error instanceof NowPlayingError && error.code === "INVALID_ARGUMENT");
try {
  console.log("sidecar sessions:", (await client.sessions()).length);
} catch (error) {
  assert.ok(error instanceof NowPlayingError, String(error));
  assert.notEqual(process.platform, "darwin", error.message);
  console.log("sidecar: media service unavailable:", error.code);
}
child.stdin.end();
assert.equal(await new Promise((resolve) => child.on("exit", resolve)), 0);
console.log("sidecar OK");
`;
// A consumer that bundles this package and ships the binary next to the bundle.
const bundled = `
import assert from "node:assert/strict";
import { join } from "node:path";
import * as media from "./bundle/app.js";
assert.equal(media.nativeBindingPath(), join(import.meta.dirname, "bundle", "prebuilds", media.nativeTarget(), "now-playing-media.node"));
await assert.rejects(() => media.seek(-1), { code: "INVALID_ARGUMENT" });
console.log("bundle OK");
`;
try {
  const tarballs = (await readdir(join(root, "artifacts"))).filter((f) =>
    f.endsWith(".tgz"),
  );
  if (tarballs.length !== 1)
    throw new Error("Expected one tarball; run pack first.");
  await copyFile(
    join(root, "artifacts", tarballs[0]!),
    join(dir, "package.tgz"),
  );
  await writeFile(
    join(dir, "package.json"),
    JSON.stringify({
      private: true,
      type: "module",
      dependencies: { "now-playing-media": "file:./package.tgz" },
    }),
  );
  await writeFile(join(dir, "consumer.mjs"), consumer);
  await run(
    npmCommand("install", "--ignore-scripts", "--no-audit", "--no-fund"),
  );
  await run(["node", "consumer.mjs"]);
  const installed = join(dir, "node_modules/now-playing-media"),
    binary = join("prebuilds", nativeTarget(), "now-playing-media.node");
  await mkdir(join(dir, "bundle", "prebuilds", nativeTarget()), {
    recursive: true,
  });
  await build({
    stdin: { contents: 'export * from "now-playing-media";', resolveDir: dir },
    bundle: true,
    format: "esm",
    platform: "node",
    outfile: join(dir, "bundle/app.js"),
    logLevel: "warning",
  });
  await copyFile(join(installed, binary), join(dir, "bundle", binary));
  await writeFile(join(dir, "bundled.mjs"), bundled);
  await run(["node", "bundled.mjs"]);
  await writeFile(join(dir, "sidecar.mjs"), sidecar);
  await run(["node", "sidecar.mjs"]);
  const modules = join(dir, "node_modules");
  await run(
    denoCommand(
      "run",
      "--no-config",
      "--no-lock",
      "--node-modules-dir=manual",
      `--allow-read=${modules}`,
      `--allow-ffi=${modules}`,
      "consumer.mjs",
    ),
  );
  console.log(
    `Verified ${tarballs[0]} with Node and Deno on ${nativeTarget()}.`,
  );
} finally {
  await rm(dir, { recursive: true, force: true });
}
