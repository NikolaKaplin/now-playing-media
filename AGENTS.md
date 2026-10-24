# Working on now-playing-media

An npm library for reading and controlling system media sessions from Node.js
and Deno. Native code lives in `native/`: `media` is the forked nowplaying crate
with its platform backends, `service` the operations shared by the Node-API
addon (`native/src`) and the JSON-lines `sidecar`. The JS API and types are
in `src/`; `src/client.ts` must stay free of Node imports. All six targets'
addon and sidecar ship in the one npm package under `prebuilds/<target>/`;
consumers never compile Rust.

- Run `bun run check` before finishing; CI covers Linux, Windows and macOS on
  x64 and arm64.
- Integration tests use the private D-Bus MPRIS fixture, never a real player.
- Never log or put track or session metadata into error messages.
- Native code is not sandboxed by Deno permissions; never claim otherwise.
- Keep `THIRD-PARTY-NOTICES.md` in sync with vendored code in `native/media`.
