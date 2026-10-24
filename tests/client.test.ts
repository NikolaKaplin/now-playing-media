import { expect, test } from "bun:test";
import { createSidecarClient, NowPlayingError } from "../src/client";
import { rawSession } from "./fixtures";

test("decodes sidecar values and forwards control parameters", async () => {
  const calls: unknown[] = [];
  const client = createSidecarClient(async (method, params) => {
    calls.push([method, params]);
    return method === "sessions"
      ? [rawSession]
      : method === "current"
        ? null
        : null;
  });
  expect(await client.current()).toBeNull();
  expect((await client.sessions())[0]?.playback.positionMs).toBe(12_000);
  await client.control("seek", "player", 5);
  expect(calls).toEqual([
    ["current", {}],
    ["sessions", {}],
    ["control", { command: "seek", sessionId: "player", positionMs: 5 }],
  ]);
});

test("sidecar error codes become NowPlayingError", async () => {
  const failing = createSidecarClient(async () => {
    throw Object.assign(new Error("Gone."), { code: "SESSION_NOT_FOUND" });
  });
  const error = await failing.current().catch((e: unknown) => e);
  expect(error).toBeInstanceOf(NowPlayingError);
  expect(error).toMatchObject({ code: "SESSION_NOT_FOUND", message: "Gone." });
  const broken = createSidecarClient(async () => ({ not: "a session" }));
  expect(broken.current()).rejects.toMatchObject({ code: "INVALID_DATA" });
});
