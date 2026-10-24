# Publishing

CI builds the addon natively on Linux, Windows and macOS (x64 and arm64), runs
the tests on each, assembles one npm tarball holding all six binaries, and
installs that tarball on all six platforms with Node and Deno.

## Releases

1. Bump `version` in `package.json`, then run `bun install` and `bun run check`.
2. Merge into `main`.
3. Push the matching tag:

   ```sh
   git tag v0.1.1
   git push origin v0.1.1
   ```

4. When the Publish workflow succeeds, approve the staged version:

   ```sh
   npm stage list now-playing-media
   npm stage download <stage-id>   # optional: compare with the CI artifact
   npm stage approve <stage-id>    # asks for 2FA
   ```

The workflow checks that the tag matches `package.json` and that the commit is
on `main`, reruns the whole CI, and stages exactly the tarball CI verified.
Prereleases such as `v0.2.0-beta.1` go to the `next` dist-tag, others to
`latest`. Reject a bad stage with `npm stage reject <stage-id>`; npm versions
are immutable once approved.

## npm setup

On npmjs.com, in the package settings:

- **Trusted Publisher**: GitHub Actions, owner `NikolaKaplin`, repository
  `now-playing-media`, workflow `publish.yml`, no environment. Leave
  "Allow npm publish" and "Allow npm dist-tag" unchecked, so CI can only stage.
- **Publishing access**: require two-factor authentication and disallow bypass
  2FA tokens.

No npm token is stored in GitHub.

A trusted publisher can only be added to an existing package, so the first
version is published by hand from the CI artifact: run CI on `main`, download
the `package` artifact and run `npm publish now-playing-media-<version>.tgz`.
