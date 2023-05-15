use anyhow::Context;
use std::{
    fs::{self, File},
    io::{self, Seek},
    path::Path,
};

use gpt::{mbr, disk, GptConfig, partition_types};

pub fn create_gpt_disk(fat_image: &Path, out_gpt_path: &Path) -> anyhow::Result<()> {
    let mut disk = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(out_gpt_path)
        .with_context(|| format!("failed to create GPT file at `{}`", out_gpt_path.display()))?;

    let partition_size: u64 = fs::metadata(fat_image)
        .context("failed to read metadata of fat image")?
        .len();

    let disk_size = partition_size + 1024 * 64; // for GPT headers
    disk.set_len(disk_size)
        .context("failed to set GPT image file length")?;

    let mbr = mbr::ProtectiveMBR::with_lb_size(
        u32::try_from((disk_size / 512) - 1).unwrap_or(0xFF_FF_FF_FF),
    );

    mbr.overwrite_lba0(&mut disk)
        .context("failed to write protective MBR")?;

    let block_size = disk::LogicalBlockSize::Lb512;

    let mut gpt = GptConfig::new()
        .writable(true)
        .initialized(false)
        .logical_block_size(block_size)
        .create_from_device(Box::new(&mut disk), None)
        .context("failed to create GPT structure in file")?;
    gpt.update_partitions(Default::default())
        .context("failed to update GPT partitions")?;

    let partition_id = gpt
        .add_partition("boot", partition_size, partition_types::EFI, 0, None)
        .context("failed to add boot EFI partition")?;

    let partition = gpt
        .partitions()
        .get(&partition_id)
        .context("failed to open boot partition after creation")?;

    let start_offset = partition
        .bytes_start(block_size)
        .context("failed to get start offset of boot partition")?;

    gpt.write().context("failed to write out GPT changes")?;

    disk.seek(io::SeekFrom::Start(start_offset))
        .context("failed to seek to start offset")?;

    io::copy(
        &mut File::open(fat_image).context("failed to open FAT image")?,
        &mut disk,
    )
        .context("failed to copy FAT image to GPT disk")?;

    Ok(())
}