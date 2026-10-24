import { nativeTargets, type NativeTarget } from "../src/native";

const rust: Record<NativeTarget, { triple: string; library: string }> = {
  "linux-x64-gnu": {
    triple: "x86_64-unknown-linux-gnu",
    library: "libnow_playing_media.so",
  },
  "linux-arm64-gnu": {
    triple: "aarch64-unknown-linux-gnu",
    library: "libnow_playing_media.so",
  },
  "win32-x64-msvc": {
    triple: "x86_64-pc-windows-msvc",
    library: "now_playing_media.dll",
  },
  "win32-arm64-msvc": {
    triple: "aarch64-pc-windows-msvc",
    library: "now_playing_media.dll",
  },
  "darwin-x64": {
    triple: "x86_64-apple-darwin",
    library: "libnow_playing_media.dylib",
  },
  "darwin-arm64": {
    triple: "aarch64-apple-darwin",
    library: "libnow_playing_media.dylib",
  },
};

export const platforms = nativeTargets.map((key) => ({ key, ...rust[key] }));

export function prebuildPath(key: NativeTarget) {
  return `prebuilds/${key}/now-playing-media.node`;
}
