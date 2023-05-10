use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let nukleus_file = PathBuf::from(std::env::var_os("CARGO_BIN_FILE_NUKLEUS_nukleus").unwrap());
    let initium_file = PathBuf::from(std::env::var_os("CARGO_BIN_FILE_INITIUM_initium").unwrap());

    println!("cargo:rustc-env=OUT_DIR={}", out_dir.display());
    println!("cargo:rustc-env=CARGO_BIN_FILE_NUKLEUS_nukleus={}", nukleus_file.display());
    println!("cargo:rustc-env=CARGO_BIN_FILE_INITIUM_initium={}", initium_file.display());
}