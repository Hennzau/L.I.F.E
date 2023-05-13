use std::path::Path;

use crate::disk_image::DiskImageBuilder;

pub struct UefiBoot {
    image_builder: DiskImageBuilder,
}

impl UefiBoot {
    pub fn new(kernel_path: &Path) -> Self {
        Self {
            image_builder: DiskImageBuilder::new(kernel_path.to_owned()),
        }
    }

    pub fn set_ramdisk(&mut self, ramdisk_path: &Path) -> &mut Self {
        self.image_builder.set_ramdisk(ramdisk_path.to_owned());
        self
    }

    pub fn create_disk_image(&self, bootloader_path: &Path, out_path: &Path) -> anyhow::Result<()> {
        self.image_builder.create_uefi_image(bootloader_path, out_path)
    }
}