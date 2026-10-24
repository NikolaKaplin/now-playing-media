import { createRequire } from "node:module";

/** Runs the Deno from the `deno` dev dependency. */
export function denoCommand(...args: string[]): string[] {
  const launcher = createRequire(import.meta.url).resolve("deno/bin.cjs");
  return [process.execPath, launcher, ...args];
}
