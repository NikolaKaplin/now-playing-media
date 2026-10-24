export function releaseInfo(tag: string, version: string) {
  const semver =
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][A-Za-z0-9-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][A-Za-z0-9-]*))*))?$/;
  if (!semver.test(version) || version.includes("\n") || tag !== `v${version}`)
    throw new Error(
      "Tag must match v<package version> with a valid SemVer version.",
    );
  return { version, distTag: version.includes("-") ? "next" : "latest" };
}
