import { expect, test } from "bun:test";
import { releaseInfo } from "../scripts/release-info";
test("tags match package versions and prereleases do not replace latest", () => {
  expect(releaseInfo("v2.0.0", "2.0.0")).toEqual({
    version: "2.0.0",
    distTag: "latest",
  });
  expect(releaseInfo("v2.1.0-beta.1", "2.1.0-beta.1").distTag).toBe("next");
  for (const tag of ["main", "2.0.0", "v2.0.1", "v2.0.0\n"])
    expect(() => releaseInfo(tag, "2.0.0")).toThrow();
});
test("malformed versions cannot reach npm", () => {
  for (const version of [
    "02.0.0",
    "2.0",
    "2.0.0-",
    "2.0.0-01",
    "2.0.0+local",
    "2.0.0;echo",
    "2.0.0\n",
  ])
    expect(() => releaseInfo(`v${version}`, version)).toThrow();
});
