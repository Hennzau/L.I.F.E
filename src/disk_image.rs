use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::Context;
use tempfile::NamedTempFile;

use crate::file_data::FileDataSource;
use crate::fat_fs::create_fat_filesystem;
use crate::gpt_fs::create_gpt_disk;

pub const KERNEL_FILE_NAME: &str = "kernel-x86_64";
pub const BOOTLOADER_FILE_NAME: &str = "efi/boot/bootx64.efi";
pub const RAMDISK_FILE_NAME: &str = "ramdisk";

pub struct DiskImageBuilder {
    files: BTreeMap<Cow<'static, str>, FileDataSource>,
}

impl DiskImageBuilder {
    pub fn new(kernel: PathBuf) -> Self {
        let mut obj = Self::empty();
        obj.set_kernel(kernel);
        obj
    }

    pub fn empty() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    pub fn set_kernel(&mut self, path: PathBuf) -> &mut Self {
        self.set_file_source(KERNEL_FILE_NAME.into(), FileDataSource::File(path))
    }

    pub fn set_ramdisk(&mut self, path: PathBuf) -> &mut Self {
        self.set_file_source(RAMDISK_FILE_NAME.into(), FileDataSource::File(path))
    }

    pub fn set_file_contents(&mut self, destination: String, data: Vec<u8>) -> &mut Self {
        self.set_file_source(destination.into(), FileDataSource::Data(data))
    }

    pub fn set_file(&mut self, destination: String, file_path: PathBuf) -> &mut Self {
        self.set_file_source(destination.into(), FileDataSource::File(file_path))
    }


    fn set_file_source(
        &mut self,
        destination: Cow<'static, str>,
        source: FileDataSource,
    ) -> &mut Self {
        self.files.insert(destination, source);
        self
    }

    fn create_fat_filesystem_image(
        &self,
        internal_files: BTreeMap<&str, FileDataSource>,
    ) -> anyhow::Result<NamedTempFile> {
        let mut local_map: BTreeMap<&str, _> = BTreeMap::new();

        for (name, source) in &self.files {
            local_map.insert(name, source);
        }

        for k in &internal_files {
            if local_map.insert(k.0, k.1).is_some() {
                return Err(anyhow::Error::msg(format!(
                    "Attempted to overwrite internal file: {}",
                    k.0
                )));
            }
        }

        let out_file = NamedTempFile::new().context("failed to create temp file")?;
        create_fat_filesystem(local_map, out_file.path())
            .context("failed to create BIOS FAT filesystem")?;

        Ok(out_file)
    }

    pub fn create_uefi_image(&self, bootloader_path: &Path, image_path: &Path) -> anyhow::Result<()> {
        let mut internal_files = BTreeMap::new();
        internal_files.insert(
            BOOTLOADER_FILE_NAME,
            FileDataSource::File(bootloader_path.to_path_buf()),
        );

        let fat_partition = self
            .create_fat_filesystem_image(internal_files)
            .context("failed to create FAT partition")?;

        create_gpt_disk(fat_partition.path(), image_path)
            .context("failed to create UEFI GPT disk image")?;

        fat_partition
            .close()
            .context("failed to delete FAT partition after disk image creation")?;

        Ok(())
    }
}