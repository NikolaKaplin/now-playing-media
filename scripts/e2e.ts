// Drives the built package against a fake MPRIS player on a private D-Bus, so
// no real player or user session is touched. Linux only.
import { join, resolve } from "node:path";
import { denoCommand } from "./deno";

const root = resolve(import.meta.dir, "..");
if (process.platform !== "linux") {
  console.log("The MPRIS end-to-end test runs on Linux only.");
  process.exit(0);
}
if (!process.argv.includes("--session")) {
  const build = Bun.spawn(
    [
      "cargo",
      "build",
      "--locked",
      "--manifest-path",
      "native/Cargo.toml",
      "--example",
      "mpris_fixture",
    ],
    { cwd: root, stdout: "inherit", stderr: "inherit" },
  );
  if ((await build.exited) !== 0)
    throw new Error("MPRIS fixture build failed.");
  const session = Bun.spawn(
    ["dbus-run-session", "--", process.execPath, import.meta.path, "--session"],
    { cwd: root, stdout: "inherit", stderr: "inherit" },
  );
  process.exit(await session.exited);
}
const target = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : join(root, "native/target");
const fixture = Bun.spawn([join(target, "debug/examples/mpris_fixture")], {
  cwd: root,
  stdout: "pipe",
  stderr: "inherit",
});
try {
  const reader = fixture.stdout.getReader();
  const timer = setTimeout(() => fixture.kill(), 10_000);
  const first = await reader.read();
  clearTimeout(timer);
  if (first.done || !new TextDecoder().decode(first.value).includes("READY"))
    throw new Error("MPRIS fixture did not start.");
  reader.releaseLock();
  const test = Bun.spawn(
    denoCommand(
      "test",
      "--no-config",
      "--no-lock",
      `--allow-read=${root}`,
      `--allow-ffi=${join(root, "prebuilds")}`,
      join(root, "tests/mpris.integration.ts"),
    ),
    { cwd: root, stdout: "inherit", stderr: "inherit" },
  );
  if ((await test.exited) !== 0)
    throw new Error("MPRIS end-to-end test failed.");
} finally {
  fixture.kill();
  await fixture.exited;
}
