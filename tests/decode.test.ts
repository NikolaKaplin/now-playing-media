import { expect, test } from "bun:test";
import { decodeSession, decodeSessions } from "../src/decode";
import { NowPlayingError } from "../src/errors";
import { rawSession } from "./fixtures";

test("normalizes native metadata without losing nulls and milliseconds", () => {
  const session = decodeSession(rawSession);
  expect(session.track).toEqual({
    title: "Track",
    artist: "Artist",
    album: "Album",
    albumArtist: null,
    durationMs: 180_000,
    trackNumber: 1,
    genres: ["Test"],
  });
  expect(session.playback).toEqual({
    status: "Playing",
    positionMs: 12_000,
    playbackRate: 1,
  });
  expect(decodeSessions([])).toEqual([]);
  expect(decodeSession({ ...rawSession, track: null }).track).toBeNull();
});

test("rejects malformed native data with INVALID_DATA", () => {
  for (const value of [
    null,
    [],
    {},
    { ...rawSession, track: { ...rawSession.track, genres: [3] } },
    { ...rawSession, playback: { ...rawSession.playback, status: "bad" } },
    { ...rawSession, playback: { ...rawSession.playback, position_ms: NaN } },
    { ...rawSession, playback: { ...rawSession.playback, position_ms: -1 } },
    { ...rawSession, track: { ...rawSession.track, duration_ms: 2 ** 54 } },
  ])
    expect(() => decodeSession(value)).toThrow(NowPlayingError);
  expect(() => decodeSessions({})).toThrow(NowPlayingError);
});
