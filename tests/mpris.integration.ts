import { rejects, strictEqual } from "node:assert/strict";
const media = await import(new URL("../dist/index.js", import.meta.url).href);
Deno.test("reads and controls an isolated MPRIS player", async () => {
  const sessions = await media.sessions();
  strictEqual(sessions.length, 1);
  strictEqual(sessions[0].source.name, "Now Playing Fixture");
  strictEqual((await media.current()).track.title, "Track 1");
  await media.pause();
  strictEqual((await media.current()).playback.status, "Paused");
  await media.play();
  strictEqual((await media.current()).playback.status, "Playing");
  await media.seek(45000);
  strictEqual((await media.current()).playback.positionMs, 45000);
  await media.next();
  strictEqual((await media.current()).track.title, "Track 2");
  await media.previous();
  strictEqual((await media.current()).track.title, "Track 1");
  await rejects(() => media.seek(-1), { code: "INVALID_ARGUMENT" });
  await rejects(() => media.play("org.mpris.MediaPlayer2.missing"), {
    code: "SESSION_NOT_FOUND",
  });
});
