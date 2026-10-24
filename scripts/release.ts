// Validates that a release tag matches package.json and comes from main.
import { appendFile, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { releaseInfo } from "./release-info";

const root = resolve(import.meta.dir, "..");
const pkg = JSON.parse(await readFile(join(root, "package.json"), "utf8")) as {
  name: string;
  version: string;
  repository: { url: string };
};
if (pkg.name !== "now-playing-media")
  throw new Error("Unexpected package name.");
const release = releaseInfo(
  process.env.GITHUB_REF_NAME ?? process.argv[2] ?? "",
  pkg.version,
);
if (process.env.GITHUB_ACTIONS === "true") {
  if (
    process.env.GITHUB_REF_TYPE !== "tag" ||
    pkg.repository.url !==
      `git+https://github.com/${process.env.GITHUB_REPOSITORY}.git`
  )
    throw new Error("Release repository does not match package metadata.");
  if (
    Bun.spawnSync(
      ["git", "merge-base", "--is-ancestor", "HEAD", "origin/main"],
      {
        cwd: root,
      },
    ).exitCode !== 0
  )
    throw new Error("Release commit must belong to main.");
}
if (process.env.GITHUB_OUTPUT)
  await appendFile(
    process.env.GITHUB_OUTPUT,
    `version=${release.version}\ndist_tag=${release.distTag}\n`,
  );
console.log(`Validated ${pkg.name}@${release.version} (${release.distTag}).`);
