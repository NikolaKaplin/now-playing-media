# now-playing-media

Read and control the media the operating system knows is playing (Spotify,
browsers, Apple Music, VLC, …) from Node.js and Deno.

| OS      | Backend                                                                                                               | Architectures |
| ------- | --------------------------------------------------------------------------------------------------------------------- | ------------- |
| Windows | Global System Media Transport Controls (Windows 10 1809+)                                                             | x64, arm64    |
| macOS   | MediaRemote through [mediaremote-adapter](https://github.com/ungive/mediaremote-adapter) (macOS 11+, including 15.4+) | x64, arm64    |
| Linux   | MPRIS over the session D-Bus (glibc 2.35+)                                                                            | x64, arm64    |

Prebuilt Node-API binaries for all six targets ship inside the package; no
compiler, node-gyp or install script runs on install.

```sh
npm install now-playing-media
```

```ts
import {
  current,
  sessions,
  pause,
  seek,
  NowPlayingError,
} from "now-playing-media";

const session = await current(); // MediaSession | null
console.log(session?.track?.title, session?.playback.status);

await pause(); // the current session
const [first] = await sessions();
if (first) await seek(45_000, first.id); // milliseconds
```

## API

All functions return promises.

- `current()`: the current session, preferring one that is playing, or `null`.
- `sessions()`: every session the OS exposes. macOS exposes only one.
- `play`, `pause`, `toggle`, `next`, `previous` `(sessionId?)`
- `seek(positionMs, sessionId?)`

An omitted or empty `sessionId` targets the current session. The same calls
are available as `nowPlaying.current()`, `nowPlaying.sessions()` and
`nowPlaying.control(command, sessionId, positionMs?)`.

```ts
interface MediaSession {
  id: string; // pass to the control functions
  source: { id: string; name: string | null }; // the player application
  track: {
    title: string;
    artist: string | null;
    album: string | null;
    albumArtist: string | null;
    durationMs: number | null;
    trackNumber: number | null;
    genres: string[];
  } | null;
  playback: {
    status: "Playing" | "Paused" | "Stopped" | "Unknown";
    positionMs: number;
    playbackRate: number;
  };
}
```

Failures reject with `NowPlayingError`, whose `code` is one of
`INVALID_ARGUMENT`, `NO_SESSION`, `SESSION_NOT_FOUND`,
`UNSUPPORTED_OPERATION` (the player does not allow it), `UNSUPPORTED_PLATFORM`,
`BACKEND_UNAVAILABLE` (e.g. no D-Bus session), `PLATFORM_ERROR` or
`INVALID_DATA`. Error messages never contain track metadata.

## Deno

Deno loads the addon through Node-API, which needs FFI permission and a local
`node_modules`:

```sh
deno run --node-modules-dir --allow-read=node_modules --allow-ffi=node_modules main.ts
```

The addon runs native code with the process's privileges; `--allow-ffi` does
not sandbox it. On macOS it starts `/usr/bin/perl` to reach MediaRemote, which
Deno's `--allow-run` does not govern either.

## Bundling

The addon cannot be inlined into a JavaScript bundle. When this package's code
is bundled, copy the binary to `prebuilds/<target>/now-playing-media.node` next
to the bundled file. `nativeBindingPath()` returns the installed binary for the
running platform and `nativeTarget()` its name (for example `darwin-arm64`);
`nativeTargets` lists all six for cross-platform builds, whose binaries are in
`node_modules/now-playing-media/prebuilds/`.

## macOS

Since macOS 15.4 only Apple-entitled processes may read MediaRemote. The addon
embeds [mediaremote-adapter](https://github.com/ungive/mediaremote-adapter),
writes it to a private temporary directory and runs it with the system
`/usr/bin/perl`, which has the entitlement. Each call starts a short-lived perl
process (tens of milliseconds), so poll at a sensible interval. Playback rate
and position come from the player's last report and may lag slightly.

## Development

Requires Bun, Rust (pinned in `rust-toolchain.toml`) and, on Linux,
`dbus-run-session` for the end-to-end test.

```sh
bun install
bun run check   # typecheck, lint, Rust and JS tests, build, pack, install smoke test, MPRIS e2e
```

`native/media` is a fork of the [nowplaying](https://crates.io/crates/nowplaying)
crate with a new macOS backend. See [docs/PUBLISHING.md](docs/PUBLISHING.md)
for releases and [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) for
licenses.
