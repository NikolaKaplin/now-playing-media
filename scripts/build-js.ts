import { rm, mkdir } from "node:fs/promises";
import { resolve, join } from "node:path";
import { build } from "esbuild";
const root = resolve(import.meta.dir, ".."),
  out = join(root, "dist");
await rm(out, { recursive: true, force: true });
await mkdir(out, { recursive: true });
await build({
  // A shared chunk keeps one NowPlayingError class for both entry points.
  entryPoints: [join(root, "src/index.ts"), join(root, "src/client.ts")],
  outdir: out,
  bundle: true,
  splitting: true,
  chunkNames: "[name]-[hash]",
  format: "esm",
  platform: "node",
  target: "es2022",
  legalComments: "none",
});
const child = Bun.spawn(
  [process.execPath, "run", "tsc", "--project", "tsconfig.build.json"],
  { cwd: root, stdout: "inherit", stderr: "inherit" },
);
if ((await child.exited) !== 0) throw new Error("Declarations failed.");
