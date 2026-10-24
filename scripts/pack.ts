// Packs the npm tarball into artifacts/. `--all` requires every prebuilt binary,
// as for a release; without it the tarball only carries the binaries present.
import { existsSync } from "node:fs";
import { mkdir, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { npmCommand } from "./npm";
import { platforms, prebuildPath } from "./platform";

const root = resolve(import.meta.dir, "..");
const present = platforms.filter((p) =>
  existsSync(join(root, prebuildPath(p.key))),
);
if (present.length === 0)
  throw new Error("No prebuilt binaries; run build:native.");
if (process.argv.includes("--all") && present.length !== platforms.length)
  throw new Error(
    `Missing binaries: ${platforms
      .filter((p) => !present.includes(p))
      .map((p) => p.key)
      .join(", ")}`,
  );
await rm(join(root, "artifacts"), { recursive: true, force: true });
await mkdir(join(root, "artifacts"));
const child = Bun.spawn(npmCommand("pack", "--pack-destination", "artifacts"), {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
});
if ((await child.exited) !== 0) throw new Error("npm pack failed.");
console.log(`Packed binaries: ${present.map((p) => p.key).join(", ")}`);
