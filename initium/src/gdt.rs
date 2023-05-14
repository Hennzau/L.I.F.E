use x86_64::{
    instructions::segmentation::{self, Segment},
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable},
        paging::PhysFrame,
    },
    VirtAddr,
};

pub fn create_and_load(frame: PhysFrame) {
    let physical_address = frame.start_address();
    let virtual_address = VirtAddr::new(physical_address.as_u64());

    let ptr: *mut GlobalDescriptorTable = virtual_address.as_mut_ptr();

    let mut gdt = GlobalDescriptorTable::new();
    let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
    let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
    let gdt = unsafe {
        ptr.write(gdt);
        &*ptr
    };

    gdt.load();
    unsafe {
        segmentation::CS::set_reg(code_selector);
        segmentation::DS::set_reg(data_selector);
        segmentation::ES::set_reg(data_selector);
        segmentation::SS::set_reg(data_selector);
    }
}