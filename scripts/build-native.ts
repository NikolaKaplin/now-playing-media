import { copyFile, mkdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { nativeTarget, sidecarFileName } from "../src/native";
import { platforms, prebuildPath, sidecarPrebuildPath } from "./platform";

const root = resolve(import.meta.dir, ".."),
  platform = platforms.find((p) => p.key === nativeTarget())!;
const child = Bun.spawn(
  [
    "cargo",
    "build",
    "--release",
    "--locked",
    "--manifest-path",
    "native/Cargo.toml",
    "--package",
    "now-playing-media",
    "--package",
    "now-playing-media-sidecar",
    "--target",
    platform.triple,
  ],
  { cwd: root, stdout: "inherit", stderr: "inherit" },
);
if ((await child.exited) !== 0) throw new Error("Native build failed.");
const target = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : join(root, "native/target");
const release = join(target, platform.triple, "release");
await mkdir(dirname(join(root, prebuildPath(platform.key))), {
  recursive: true,
});
await copyFile(
  join(release, platform.library),
  join(root, prebuildPath(platform.key)),
);
// copyFile keeps the executable bit on Unix.
await copyFile(
  join(release, sidecarFileName(platform.key)),
  join(root, sidecarPrebuildPath(platform.key)),
);
console.log(`Built prebuilds/${platform.key}/`);
