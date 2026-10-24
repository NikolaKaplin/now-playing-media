// Talks to the sidecar through a caller-supplied transport, for hosts that run
// native code in a separate process. No Node built-ins: safe to bundle anywhere.
import { decodeSession, decodeSessions } from "./decode.js";
import {
  NowPlayingError,
  fromNative,
  type NowPlayingErrorCode,
} from "./errors.js";
import type { NowPlayingApi } from "./types.js";

/**
 * Sends one request to the sidecar and resolves with its `value`. It rejects
 * with an error carrying the sidecar's `code` when the response is not ok.
 */
export type SidecarCall = (
  method: "current" | "sessions" | "control",
  params: Record<string, unknown>,
) => Promise<unknown>;

export function createSidecarClient(call: SidecarCall): NowPlayingApi {
  const request = async (
    method: Parameters<SidecarCall>[0],
    params: Record<string, unknown> = {},
  ) => {
    try {
      return await call(method, params);
    } catch (error) {
      throw asNowPlayingError(error);
    }
  };
  return {
    async current() {
      const value = await request("current");
      return value === null ? null : decodeSession(value);
    },
    async sessions() {
      return decodeSessions(await request("sessions"));
    },
    async control(command, sessionId, positionMs) {
      await request("control", { command, sessionId, positionMs });
    },
  };
}

function asNowPlayingError(error: unknown): NowPlayingError {
  if (error instanceof NowPlayingError) return error;
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? String(error.code)
      : "";
  const message = error instanceof Error ? error.message : String(error);
  return fromNative(new Error(code ? `${code}: ${message}` : message));
}

export { NowPlayingError };
export type { NowPlayingErrorCode };
export type {
  NowPlayingApi,
  MediaSession,
  MediaTrack,
  PlaybackStatus,
  Control,
} from "./types.js";
