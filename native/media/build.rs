//! Builds the mediaremote-adapter helper library for the macOS backend. It is
//! embedded into the crate and loaded by /usr/bin/perl, never by this process.

use std::{env, fs, path::PathBuf, process::Command};

const SOURCES: &[&str] = &[
    "src/adapter/env.m",
    "src/adapter/get.m",
    "src/adapter/globals.m",
    "src/adapter/keys.m",
    "src/adapter/now_playing.m",
    "src/adapter/repeat.m",
    "src/adapter/seek.m",
    "src/adapter/send.m",
    "src/adapter/shuffle.m",
    "src/adapter/speed.m",
    "src/adapter/stream.m",
    "src/adapter/test.m",
    "src/private/MediaRemote.m",
    "src/utility/Debounce.m",
    "src/utility/helpers.m",
];

fn main() {
    println!("cargo:rerun-if-changed=mediaremote-adapter");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("MediaRemoteAdapter");
    if !cfg!(target_os = "macos") {
        // `cargo check --target *-apple-darwin` from another OS has no Apple SDK.
        // The backend reports itself unavailable when the helper is empty.
        println!("cargo:warning=mediaremote-adapter is only built on macOS hosts");
        fs::write(&output, []).unwrap();
        return;
    }

    let arch = match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        other => panic!("unsupported macOS architecture: {other}"),
    };
    let root =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("mediaremote-adapter");
    let deployment_target = env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "11.0".into());

    let status = Command::new("xcrun")
        .args([
            "--sdk",
            "macosx",
            "clang",
            "-dynamiclib",
            "-O2",
            "-fobjc-arc",
        ])
        // The perl script resolves the adapter functions by name.
        .arg("-fvisibility=default")
        .args(["-arch", arch])
        .arg(format!("-mmacosx-version-min={deployment_target}"))
        .arg("-I")
        .arg(root.join("include"))
        .arg("-I")
        .arg(root.join("src"))
        .args(SOURCES.iter().map(|source| root.join(source)))
        .args(["-framework", "Foundation", "-framework", "AppKit"])
        .args(["-framework", "UniformTypeIdentifiers"])
        .arg("-o")
        .arg(&output)
        .status()
        .expect("failed to run xcrun clang");
    assert!(status.success(), "building mediaremote-adapter failed");

    // Apple silicon refuses unsigned code; an ad-hoc signature is enough for perl.
    let status = Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&output)
        .status()
        .expect("failed to run codesign");
    assert!(status.success(), "signing mediaremote-adapter failed");
}
