use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

// The SHA-256 checksums of the prebuilds attached to the release of this
// version, which the publish workflow checks before releasing them.
const CHECKSUMS: &[(&str, &str)] = &[];

fn main() {
    println!("cargo:rerun-if-env-changed=BARE_KIT_DIR");
    println!("cargo:rerun-if-env-changed=BARE_KIT_MIRROR");

    if env::var_os("DOCS_RS").is_some() {
        return;
    }

    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target = target(&os);

    // The prebuilt engine links the static C runtime on Windows, which the
    // rest of the executable must then link too.
    if os == "windows"
        && !env::var("CARGO_CFG_TARGET_FEATURE")
            .unwrap_or_default()
            .split(',')
            .any(|feature| feature == "crt-static")
    {
        panic!(
            "Bare Kit requires the static C runtime on Windows, build with `-C target-feature=+crt-static`"
        );
    }

    let dir = match env::var_os("BARE_KIT_DIR") {
        Some(dir) => local(PathBuf::from(dir), &os, &target),
        None => {
            let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
                .join("prebuilds")
                .join(&target);

            if dir.is_dir() {
                local(dir, &os, &target)
            } else {
                download(&target)
            }
        }
    };

    println!("cargo:rustc-link-search=native={}", dir.display());

    if os == "windows" {
        println!("cargo:rustc-link-lib=static:+whole-archive=bare_kit");
        println!("cargo:rustc-link-lib=static=bare_kit_deps");
    } else {
        println!("cargo:rustc-link-lib=static=bare_kit");
    }

    if os == "windows" || os == "linux" {
        for library in fs::read_to_string(dir.join("bare_kit.libs"))
            .unwrap()
            .lines()
        {
            println!("cargo:rustc-link-lib={library}");
        }
    }

    match os.as_str() {
        "macos" => {
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
        }
        "ios" => {
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            println!("cargo:rustc-link-lib=framework=UIKit");
        }
        "linux" => {
            // Link arguments only reach the targets of this package, so the
            // dynamic list is also passed on for applications to link with.
            let dynamic_list = dir.join("bare_kit.dynamic");

            println!(
                "cargo:rustc-link-arg=-Wl,--dynamic-list={}",
                dynamic_list.display()
            );
            println!("cargo:dynamic_list={}", dynamic_list.display());
        }
        "android" => {
            println!("cargo:rustc-link-lib=android");
            println!("cargo:rustc-link-lib=log");
        }
        _ => {}
    }
}

fn local(dir: PathBuf, os: &str, target: &str) -> PathBuf {
    let archive = dir.join(archive_name(os));

    if !archive.is_file() {
        panic!("No Bare Kit prebuild for {target} in {}", dir.display());
    }

    println!("cargo:rerun-if-changed={}", archive.display());

    dir
}

// Download the prebuild from the release of this version, and extract it to
// the output directory unless an earlier build already did.
fn download(target: &str) -> PathBuf {
    let checksum = CHECKSUMS
        .iter()
        .find(|(name, _)| *name == target)
        .map(|(_, checksum)| *checksum)
        .unwrap_or_else(|| panic!("No Bare Kit prebuild for {target}"));

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let archive = out.join(format!("{target}.tar.gz"));
    let dir = out.join("prebuilds").join(target);

    if dir.is_dir() && verify(&archive, checksum) {
        return dir;
    }

    let mirror = env::var("BARE_KIT_MIRROR").unwrap_or_else(|_| {
        "https://github.com/holepunchto/bare-kit-rust/releases/download".into()
    });

    let url = format!(
        "{}/v{}/{target}.tar.gz",
        mirror.trim_end_matches('/'),
        env::var("CARGO_PKG_VERSION").unwrap()
    );

    run(Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--output",
        ])
        .arg(&archive)
        .arg(&url));

    if !verify(&archive, checksum) {
        let _ = fs::remove_file(&archive);

        panic!("Checksum mismatch for {url}");
    }

    run(Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&out));

    dir
}

fn verify(archive: &Path, checksum: &str) -> bool {
    let Ok(data) = fs::read(archive) else {
        return false;
    };

    let digest: String = Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    digest == checksum
}

fn run(command: &mut Command) {
    let status = command
        .status()
        .unwrap_or_else(|err| panic!("Failed to run {command:?}: {err}"));

    if !status.success() {
        panic!("{command:?} failed with {status}");
    }
}

// The target named as by Bare, such as `darwin-arm64` or `ios-arm64-simulator`.
fn target(os: &str) -> String {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let abi = env::var("CARGO_CFG_TARGET_ABI").unwrap_or_default();

    let platform = match os {
        "macos" => "darwin",
        "windows" => "win32",
        "ios" | "linux" | "android" => os,
        _ => panic!("Unsupported target OS {os}"),
    };

    let arch = match arch.as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        "x86" => "ia32",
        "arm" => "arm",
        _ => panic!("Unsupported target architecture {arch}"),
    };

    if os == "ios" && (abi == "sim" || arch == "x64") {
        format!("{platform}-{arch}-simulator")
    } else {
        format!("{platform}-{arch}")
    }
}

fn archive_name(os: &str) -> &'static str {
    if os == "windows" {
        "bare_kit.lib"
    } else {
        "libbare_kit.a"
    }
}
