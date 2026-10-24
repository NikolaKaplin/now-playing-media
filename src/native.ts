import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { arch, platform } from "node:process";
import { fileURLToPath } from "node:url";
import { decodeSession, decodeSessions } from "./decode.js";
import { NowPlayingError, fromNative } from "./errors.js";
import type { NativeBinding, NowPlayingApi } from "./types.js";

export const nativeTargets = [
  "linux-x64-gnu",
  "linux-arm64-gnu",
  "win32-x64-msvc",
  "win32-arm64-msvc",
  "darwin-x64",
  "darwin-arm64",
] as const;
export type NativeTarget = (typeof nativeTargets)[number];

function isMusl(): boolean {
  let report: { header?: { glibcVersionRuntime?: string } } | undefined;
  try {
    report = (
      process as {
        report?: {
          getReport?(): { header?: { glibcVersionRuntime?: string } };
        };
      }
    ).report?.getReport?.();
  } catch {
    // Deno without --allow-sys. A glibc binary then fails to load on musl instead.
    return false;
  }
  return report?.header !== undefined && !report.header.glibcVersionRuntime;
}

/** The prebuilt binary matching this process, e.g. `linux-x64-gnu`. */
export function nativeTarget(): NativeTarget {
  const target =
    platform === "linux"
      ? isMusl()
        ? `linux-${arch}-musl`
        : `linux-${arch}-gnu`
      : platform === "win32"
        ? `win32-${arch}-msvc`
        : `${platform}-${arch}`;
  if (!(nativeTargets as readonly string[]).includes(target))
    throw new NowPlayingError(
      "UNSUPPORTED_PLATFORM",
      `now-playing-media has no prebuilt binary for ${target}.`,
    );
  return target as NativeTarget;
}

/**
 * Absolute path of the addon. It is looked up in the package (`dist/../prebuilds`)
 * and, for bundled code, in `prebuilds/<target>/` next to the bundle.
 */
export function nativeBindingPath(): string {
  const file = `prebuilds/${nativeTarget()}/now-playing-media.node`;
  const candidates = [`../${file}`, `./${file}`].map((path) =>
    fileURLToPath(new URL(path, import.meta.url)),
  );
  return candidates.find((path) => existsSync(path)) ?? candidates[0]!;
}

let binding: NativeBinding | undefined;
function load(): NativeBinding {
  if (binding) return binding;
  const path = nativeBindingPath();
  let loaded: unknown;
  try {
    loaded = createRequire(import.meta.url)(path);
  } catch (error) {
    throw new NowPlayingError(
      "BACKEND_UNAVAILABLE",
      `Could not load ${path}. Deno needs --allow-ffi for it. ${error instanceof Error ? error.message : ""}`.trim(),
    );
  }
  const exports = loaded as Partial<Record<keyof NativeBinding, unknown>>;
  if (
    typeof exports.current !== "function" ||
    typeof exports.sessions !== "function" ||
    typeof exports.control !== "function"
  )
    throw new NowPlayingError(
      "BACKEND_UNAVAILABLE",
      `${path} is not a compatible now-playing-media addon.`,
    );
  binding = loaded as NativeBinding;
  return binding;
}

async function call<T>(operation: () => Promise<T>): Promise<T> {
  try {
    return await operation();
  } catch (error) {
    throw error instanceof NowPlayingError ? error : fromNative(error);
  }
}

export const nowPlaying: NowPlayingApi = {
  current: () =>
    call(async () => {
      const json = await load().current();
      return json === null || json === undefined
        ? null
        : decodeSession(JSON.parse(json));
    }),
  sessions: () =>
    call(async () => decodeSessions(JSON.parse(await load().sessions()))),
  control: (command, sessionId, positionMs) =>
    call(() => load().control(command, sessionId, positionMs)),
};
