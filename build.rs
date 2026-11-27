use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

// The SHA-256 checksums of the prebuilds attached to the release of this
// version, which the publish workflow checks before releasing them.
const CHECKSUMS: &[(&str, &str)] = &[
    (
        "android-arm",
        "5b3d21177d72d53e1099e6ce4fe89a736fd2899558aaf436c62c8581976b8f5f",
    ),
    (
        "android-arm64",
        "3af45055ecf78c9f2cd6326727c7666939718f99ca5a950c23990260bf57f014",
    ),
    (
        "android-ia32",
        "7bb4f2533843b2bc7a4ba009f7c91c35f8dfda00a9e912dd7bdb13d8363614c6",
    ),
    (
        "android-x64",
        "b57f7cce282b41f4c6e8dd837f30a7e06103d03257b1b6ef0fc59d7d94499358",
    ),
    (
        "darwin-arm64",
        "0e47c8b5004f3968295ae13c480d80f7723e68ee2cbcbfac1b171687c44d840c",
    ),
    (
        "darwin-x64",
        "a6ace600d2c8cfaef8c0f5f2eee22684c10688deee8d29a5be15dfe84c37d6e1",
    ),
    (
        "ios-arm64",
        "60d4133eda1495ebad348f6769c48485388faabbfeb64aa1d41de3058285045b",
    ),
    (
        "ios-arm64-simulator",
        "45f27a02419e4655423a1d8e667205cbcd49e9153cd4ddad8db1ba5f8c739c03",
    ),
    (
        "ios-x64-simulator",
        "1fc93b07ea2e4658b093fc6db445d59d377308302683e96dee622d53b3c585a6",
    ),
    (
        "linux-arm64",
        "a81f384e639c719c1168011cc2e34b4e8a4ceca63d11d7163080a2ee95b61250",
    ),
    (
        "linux-x64",
        "b13b7e36344ac8d9c7acb2e465bb9e44ff7ad585dee1833f15eb6ba18616ae5b",
    ),
    (
        "win32-arm64",
        "5e25a962c42c28fbe82e740a1bd638e6615e165cff1ce49d755a18f581960172",
    ),
    (
        "win32-x64",
        "3b356c3f2c514ca2200295cc420eaefe0e09ecd79758279d5f8841400391f058",
    ),
];

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

    if matches!(os.as_str(), "windows" | "linux" | "android") {
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
