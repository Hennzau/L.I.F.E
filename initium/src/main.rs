#![feature(step_trait)]
#![no_std]
#![no_main]

mod memory;
mod descriptor;
mod gdt;
mod entries;
mod kernel;

mod initium;

use crate::descriptor::UefiMemoryDescriptor;
use crate::memory::LegacyFrameAllocator;
use crate::initium::{load_and_switch_to_kernel, PageTables, RawFramebufferInfo, SystemInfo};

use crate::kernel::Kernel;

use synapse::framebuffer::FramebufferInfo;
use synapse::boot::BootConfig;

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    ptr, slice,
};
use uefi::{
    prelude::{entry, Boot, Handle, Status, SystemTable},
    proto::{
        console::gop::{GraphicsOutput, PixelFormat},
        device_path::DevicePath,
        loaded_image::LoadedImage,
        media::{
            file::{File, FileAttribute, FileInfo, FileMode},
            fs::SimpleFileSystem,
        },
        network::{
            pxe::{BaseCode, DhcpV4Packet},
            IpAddress,
        },
        ProtocolPointer,
    },
    table::boot::{
        AllocateType, MemoryType, OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol,
    },
    CStr16, CStr8,
};
use uefi::proto::console::gop::Mode;
use x86_64::{
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

struct RacyCell<T>(UnsafeCell<T>);

impl<T> RacyCell<T> {
    const fn new(v: T) -> Self {
        Self(UnsafeCell::new(v))
    }
}

unsafe impl<T> Sync for RacyCell<T> {}

impl<T> core::ops::Deref for RacyCell<T> {
    type Target = UnsafeCell<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

static SYSTEM_TABLE: RacyCell<Option<SystemTable<Boot>>> = RacyCell::new(None);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn open_device_path_protocol(
    image: Handle,
    system_table: &SystemTable<Boot>,
) -> Option<ScopedProtocol<DevicePath>> {
    let this = system_table.boot_services();
    let loaded_image = unsafe {
        this.open_protocol::<LoadedImage>(
            OpenProtocolParams {
                handle: image,
                agent: image,
                controller: None,
            },
            OpenProtocolAttributes::Exclusive,
        )
    };

    if loaded_image.is_err() {
        return None;
    }

    let loaded_image = loaded_image.unwrap();
    let loaded_image = loaded_image.deref();

    let device_handle = loaded_image.device();

    let device_path = unsafe {
        this.open_protocol::<DevicePath>(
            OpenProtocolParams {
                handle: device_handle,
                agent: image,
                controller: None,
            },
            OpenProtocolAttributes::Exclusive,
        )
    };
    if device_path.is_err() {
        return None;
    }
    Some(device_path.unwrap())
}

fn locate_and_open_protocol<P: ProtocolPointer>(
    image: Handle,
    system_table: &SystemTable<Boot>,
) -> Option<ScopedProtocol<P>> {
    let this = system_table.boot_services();
    let device_path = open_device_path_protocol(image, system_table)?;
    let mut device_path = device_path.deref();

    let fs_handle = this.locate_device_path::<P>(&mut device_path);
    if fs_handle.is_err() {
        return None;
    }

    let fs_handle = fs_handle.unwrap();

    let opened_handle = unsafe {
        this.open_protocol::<P>(
            OpenProtocolParams {
                handle: fs_handle,
                agent: image,
                controller: None,
            },
            OpenProtocolAttributes::Exclusive,
        )
    };

    if opened_handle.is_err() {
        return None;
    }
    Some(opened_handle.unwrap())
}

fn load_file_from_disk(
    name: &str,
    image: Handle,
    system_table: &SystemTable<Boot>,
) -> Option<&'static mut [u8]> {
    let mut file_system_raw = locate_and_open_protocol::<SimpleFileSystem>(image, system_table)?;
    let file_system = file_system_raw.deref_mut();

    let mut root = file_system.open_volume().unwrap();
    let mut buf = [0u16; 256];

    assert!(name.len() < 256);

    let filename = CStr16::from_str_with_buf(name.trim_end_matches('\0'), &mut buf)
        .expect("Failed to convert string to utf16");

    let file_handle_result = root.open(filename, FileMode::Read, FileAttribute::empty());

    let file_handle = match file_handle_result {
        Err(_) => return None,
        Ok(handle) => handle,
    };

    let mut file = match file_handle.into_type().unwrap() {
        uefi::proto::media::file::FileType::Regular(f) => f,
        uefi::proto::media::file::FileType::Dir(_) => panic!(),
    };

    let mut buf = [0; 500];
    let file_info: &mut FileInfo = file.get_info(&mut buf).unwrap();
    let file_size = usize::try_from(file_info.file_size()).unwrap();

    let file_ptr = system_table
        .boot_services()
        .allocate_pages(
            AllocateType::AnyPages,
            MemoryType::LOADER_DATA,
            ((file_size - 1) / 4096) + 1,
        )
        .unwrap() as *mut u8;
    unsafe { ptr::write_bytes(file_ptr, 0, file_size) };
    let file_slice = unsafe { slice::from_raw_parts_mut(file_ptr, file_size) };
    file.read(file_slice).unwrap();

    Some(file_slice)
}

fn load_kernel(
    image: Handle,
    system_table: &mut SystemTable<Boot>,
) -> Option<Kernel<'static>> {
    Some(Kernel::parse(load_file_from_disk("kernel-x86_64\0", image, system_table)?))
}

fn load_ramdisk(
    image: Handle,
    system_table: &mut SystemTable<Boot>,
) -> Option<&'static mut [u8]> {
    load_file_from_disk("ramdisk\0", image, system_table)
}

fn load_framebuffer(
    image_handle: Handle,
    system_table: &SystemTable<Boot>,
    config: &BootConfig,
) -> Option<RawFramebufferInfo> {
    let gop_handle = system_table
        .boot_services()
        .get_handle_for_protocol::<GraphicsOutput>()
        .ok()?;

    let mut gop = unsafe {
        system_table.boot_services()
            .open_protocol::<GraphicsOutput>(
                OpenProtocolParams {
                    handle: gop_handle,
                    agent: image_handle,
                    controller: None,
                },
                OpenProtocolAttributes::Exclusive,
            )
            .ok()?
    };

    let mut last_width = 0;
    let mut last_height = 0;

    for mode in gop.modes() {
        let (width, height) = mode.info().resolution();

        if width <= config.framebuffer_width && height <= config.framebuffer_height {
            if width >= last_width || height >= last_height {
                last_width = width;
                last_height = height;
            }
        }
    }

    let mode = {
        let modes = gop.modes();
        match (
            last_width,
            last_height
        ) {
            (width, height) => modes
                .filter(|m| {
                    let res = m.info().resolution();
                    res.0 == width && res.1 == height
                }).last()
        }
    };

    if let Some(mode) = mode {
        gop.set_mode(&mode)
            .expect("Failed to apply the desired display mode");
    }

    let mode_info = gop.current_mode_info();
    let mut framebuffer = gop.frame_buffer();
    let slice = unsafe { slice::from_raw_parts_mut(framebuffer.as_mut_ptr(), framebuffer.size()) };

    let info = FramebufferInfo {
        byte_len: framebuffer.size(),
        width: mode_info.resolution().0,
        height: mode_info.resolution().1,
        pixel_format: match mode_info.pixel_format() {
            PixelFormat::Rgb => synapse::framebuffer::PixelFormat::Rgb,
            PixelFormat::Bgr => synapse::framebuffer::PixelFormat::Bgr,
            PixelFormat::Bitmask | PixelFormat::BltOnly => {
                panic!("Bitmask and BltOnly framebuffers are not supported")
            }
        },
        bytes_per_pixel: 4,
        stride: mode_info.stride(),
    };

    Some(RawFramebufferInfo {
        addr: PhysAddr::new(framebuffer.as_mut_ptr() as u64),
        info,
    })
}

fn create_page_tables(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> PageTables {
    let phys_offset = VirtAddr::new(0);

    let bootloader_page_table = {
        let old_table = {
            let frame = x86_64::registers::control::Cr3::read().0;
            let ptr: *const PageTable = (phys_offset + frame.start_address().as_u64()).as_ptr();
            unsafe { &*ptr }
        };

        let new_frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate frame for new level 4 table");

        let new_table: &mut PageTable = {
            let ptr: *mut PageTable =
                (phys_offset + new_frame.start_address().as_u64()).as_mut_ptr();

            unsafe {
                ptr.write(PageTable::new());
                &mut *ptr
            }
        };

        new_table[0] = old_table[0].clone();

        unsafe {
            x86_64::registers::control::Cr3::write(
                new_frame,
                x86_64::registers::control::Cr3Flags::empty(),
            );
            OffsetPageTable::new(&mut *new_table, phys_offset)
        }
    };

    let (kernel_page_table, kernel_level_4_frame) = {
        let frame: PhysFrame = frame_allocator.allocate_frame().expect("no unused frames");
        let addr = phys_offset + frame.start_address().as_u64();

        let ptr = addr.as_mut_ptr();
        unsafe { *ptr = PageTable::new() };

        let level_4_table = unsafe { &mut *ptr };
        (
            unsafe { OffsetPageTable::new(level_4_table, phys_offset) },
            frame,
        )
    };

    initium::PageTables {
        bootloader: bootloader_page_table,
        kernel: kernel_page_table,
        kernel_level_4_frame,
    }
}

#[entry]
fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    main_inner(image, system_table)
}

fn main_inner(image: Handle, mut system_table: SystemTable<Boot>) -> Status {
    unsafe {
        *SYSTEM_TABLE.get() = Some(system_table.unsafe_clone());
    }

    let mut kernel = load_kernel(image, &mut system_table);
    let kernel = kernel.expect("Failed to load kernel");

    let config = BootConfig {
        framebuffer_width: 1280,
        framebuffer_height: 720,
    };

    let framebuffer = load_framebuffer(image, &system_table, &config);

    unsafe {
        *SYSTEM_TABLE.get() = None;
    }

    let ramdisk = load_ramdisk(image, &mut system_table);

    let (system_table, mut memory_map) = system_table.exit_boot_services();

    memory_map.sort();

    let mut frame_allocator =
        LegacyFrameAllocator::new(memory_map.entries().copied().map(UefiMemoryDescriptor));

    let page_tables = create_page_tables(&mut frame_allocator);
    let mut ramdisk_len = 0u64;
    let ramdisk_addr = if let Some(rd) = ramdisk {
        ramdisk_len = rd.len() as u64;
        Some(rd.as_ptr() as usize as u64)
    } else {
        None
    };
    let system_info = SystemInfo {
        framebuffer,
        rsdp_addr: {
            use uefi::table::cfg;

            let mut config_entries = system_table.config_table().iter();
            let acpi2_rsdp = config_entries.find(|entry| matches!(entry.guid, cfg::ACPI2_GUID));
            let rsdp = acpi2_rsdp
                .or_else(|| config_entries.find(|entry| matches!(entry.guid, cfg::ACPI_GUID)));

            rsdp.map(|entry| PhysAddr::new(entry.address as u64))
        },
        ramdisk_addr,
        ramdisk_len,
    };

    load_and_switch_to_kernel(kernel, config, frame_allocator, page_tables, system_info)
}