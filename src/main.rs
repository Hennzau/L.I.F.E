mod uefi;
mod file_data;
mod fat_fs;
mod gpt_fs;

mod disk_image;

use std::path::PathBuf;

use uefi::UefiBoot;

fn main() {
    let out_dir = PathBuf::from(env!("OUT_DIR"));
    let nukleus = PathBuf::from(env!("CARGO_BIN_FILE_NUKLEUS_nukleus"));
    let initium = PathBuf::from(env!("CARGO_BIN_FILE_INITIUM_initium"));

    let uefi_path = out_dir.join("uefi.img");
    let mut uefi_boot = UefiBoot::new(&nukleus);

    uefi_boot.create_disk_image(initium.as_path(), &uefi_path).unwrap();
}