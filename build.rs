use std::path::{Path, PathBuf};
use std::process::Command;

use futures::executor::block_on;
use futures_concurrency::future::{FutureExt, Join};

async fn build_initium_as_efi(out_dir: &Path) -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.arg("install").arg("initium");

    cmd.arg("--path").arg("initium");
    cmd.arg("--locked");
    cmd.arg("--target").arg("x86_64-unknown-uefi");
    cmd.arg("-Zbuild-std=core")
        .arg("-Zbuild-std-features=compiler-builtins-mem");

    cmd.arg("--root").arg(out_dir);
    cmd.env_remove("RUSTFLAGS");
    cmd.env_remove("CARGO_ENCODED_RUSTFLAGS");

    let status = cmd
        .status()
        .expect("failed to run cargo install for uefi bootloader");

    if status.success() {
        let path = out_dir.join("bin").join("initium.efi");
        assert!(
            path.exists(),
            "uefi bootloader executable does not exist after building"
        );
        path
    } else {
        panic!("failed to build uefi bootloader");
    }
}

async fn build() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let nukleus_file = PathBuf::from(std::env::var_os("CARGO_BIN_FILE_NUKLEUS_nukleus").unwrap());
    let initium_file = build_initium_as_efi(&out_dir).await;

    println!("cargo:rustc-env=OUT_DIR={}", out_dir.display());
    println!("cargo:rustc-env=CARGO_BIN_FILE_NUKLEUS_nukleus={}", nukleus_file.display());
    println!("cargo:rustc-env=CARGO_BIN_FILE_INITIUM_initium={}", initium_file.display());
}

fn main() {
    block_on(build ());
}
