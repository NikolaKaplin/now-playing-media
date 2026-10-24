import { NowPlayingError } from "./errors.js";
import type { MediaSession, PlaybackStatus } from "./types.js";
function invalid(message: string): never {
  throw new NowPlayingError("INVALID_DATA", message);
}
function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    invalid("Media session is not an object.");
  return value as Record<string, unknown>;
}
function text(value: unknown): string {
  if (typeof value !== "string") invalid("Media session text is not a string.");
  return value;
}
function optionalText(value: unknown): string | null {
  return value === null ? null : text(value);
}
function finite(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value))
    invalid("Media session number is not finite.");
  return value;
}
function integer(value: unknown): number {
  const n = finite(value);
  if (!Number.isSafeInteger(n) || n < 0)
    invalid("Media position or duration is not a non-negative integer.");
  return n;
}
function status(value: unknown): PlaybackStatus {
  if (
    value !== "Playing" &&
    value !== "Paused" &&
    value !== "Stopped" &&
    value !== "Unknown"
  )
    invalid("Unknown playback status.");
  return value;
}
function genres(value: unknown): string[] {
  if (!Array.isArray(value)) invalid("Media genres are not a list.");
  return value.map(text);
}
export function decodeSession(value: unknown): MediaSession {
  const raw = record(value),
    source = record(raw.source),
    playback = record(raw.playback),
    track = raw.track === null ? null : record(raw.track);
  return {
    id: text(raw.id),
    source: { id: text(source.id), name: optionalText(source.name) },
    playback: {
      status: status(playback.status),
      positionMs: integer(playback.position_ms),
      playbackRate: finite(playback.playback_rate),
    },
    track: track
      ? {
          title: text(track.title),
          artist: optionalText(track.artist),
          album: optionalText(track.album),
          albumArtist: optionalText(track.album_artist),
          durationMs:
            track.duration_ms === null ? null : integer(track.duration_ms),
          trackNumber:
            track.track_number === null ? null : integer(track.track_number),
          genres: genres(track.genres),
        }
      : null,
  };
}
export function decodeSessions(value: unknown): MediaSession[] {
  if (!Array.isArray(value)) invalid("Media sessions are not a list.");
  return value.map(decodeSession);
}
