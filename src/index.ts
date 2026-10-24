export {
  nowPlaying,
  nativeBindingPath,
  nativeTarget,
  nativeTargets,
} from "./native.js";
export type { NativeTarget } from "./native.js";
export { NowPlayingError } from "./errors.js";
export type { NowPlayingErrorCode } from "./errors.js";
export type {
  NowPlayingApi,
  MediaSession,
  MediaTrack,
  PlaybackStatus,
  Control,
} from "./types.js";

import { nowPlaying } from "./native.js";

/** The session the OS considers current, preferring one that is playing. */
export const current = nowPlaying.current;
/** Every media session the OS exposes. */
export const sessions = nowPlaying.sessions;
// An empty session id targets the current session.
export function play(sessionId = "") {
  return nowPlaying.control("play", sessionId);
}
export function pause(sessionId = "") {
  return nowPlaying.control("pause", sessionId);
}
export function toggle(sessionId = "") {
  return nowPlaying.control("toggle", sessionId);
}
export function next(sessionId = "") {
  return nowPlaying.control("next", sessionId);
}
export function previous(sessionId = "") {
  return nowPlaying.control("previous", sessionId);
}
export function seek(positionMs: number, sessionId = "") {
  return nowPlaying.control("seek", sessionId, positionMs);
}
