mod uefi;
mod file_data;
mod fat_fs;
mod gpt_part;

mod disk_image;

use std::path::{Path, PathBuf};

use uefi::UefiBoot;

fn main() {
    let out_dir = PathBuf::from(env!("OUT_DIR"));
    let nukleus = PathBuf::from(env!("CARGO_BIN_FILE_NUKLEUS_nukleus"));
    let initium = PathBuf::from(env!("CARGO_BIN_FILE_INITIUM_initium"));

    let uefi_path = out_dir.join("uefi.img");
    let uefi_boot = UefiBoot::new(&nukleus);

    uefi_boot.create_disk_image(initium.as_path(), &uefi_path).unwrap();

    let uefi_path = uefi_path.display();

    let mut cmd = std::process::Command::new("qemu-system-x86_64");

    cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
    cmd.arg("-drive").arg(format!("format=raw,file={uefi_path}"));

    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}