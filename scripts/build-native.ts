import { copyFile, mkdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { nativeTarget } from "../src/native";
import { platforms, prebuildPath } from "./platform";

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
    "--target",
    platform.triple,
  ],
  { cwd: root, stdout: "inherit", stderr: "inherit" },
);
if ((await child.exited) !== 0) throw new Error("Native build failed.");
const target = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : join(root, "native/target");
const output = join(root, prebuildPath(platform.key));
await mkdir(dirname(output), { recursive: true });
await copyFile(
  join(target, platform.triple, "release", platform.library),
  output,
);
console.log(`Built ${prebuildPath(platform.key)}`);
