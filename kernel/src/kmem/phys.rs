use core::ptr::slice_from_raw_parts_mut;

use common::{
    addr::{Address, Page, PageRange, PhysAddr, PhysFrame},
    MemRegions, RegionType,
};
pub struct PhysFrameAllocator {}

impl PhysFrameAllocator {
    fn alloc_storage(
        num_frames: u64,
        regions: &MemRegions,
    ) -> (&'static mut [u8], PageRange<PhysFrame>) {
        // One bit per frame, for now...
        let required_bytes = num_frames.div_ceil(8);

        let usable_region = regions
            .iter()
            .find(|r| r.as_range().len_bytes() > required_bytes && r.ty == RegionType::Usable)
            // For now just panic. Not much we can do to handle this anyway.
            .expect("No memory region available for allocating phys frame buffer!");

        let start_phys = PhysAddr::new(usable_region.start);
        // This address is an inclusive end, not exclusive, so we can use from_containing_addr below.
        let last_phys = PhysAddr::new(usable_region.start + required_bytes - 1);

        // Safety: This memory is usable for allocation because it comes from a region marked usable in the map.
        unsafe {
            // We keep physical addresses for later bookkeeping, but the actual address we use needs to be virtual.
            // Just use direct mapping for now.
            let slice = slice_from_raw_parts_mut(
                start_phys.as_direct_mapped().as_u8_ptr_mut(),
                required_bytes as usize,
            );

            let phys_range = PhysFrame::range_inclusive(
                PhysFrame::from_containing_addr(start_phys),
                PhysFrame::from_containing_addr(last_phys),
            );

            (&mut *slice, phys_range)
        }
    }

    fn write_free(map: &mut [u8], frame: PhysFrame, free: bool) {
        let byte_idx = (frame.idx() / 8) as usize;
        let bit_idx = frame.idx() % 8;

        let byte = map[byte_idx];

        if free {
            // Clear the 7 - bit_idxth bit. We subtract from 8 because
            // we want the 0th bit to be on the left, aka the high bit.
            let mask: u8 = !(1 << (7 - bit_idx));
            map[byte_idx] = byte & mask;
        } else {
            // Set the correct bit.
            let mask: u8 = 1 << (7 - bit_idx);
            map[byte_idx] = byte | mask;
        }
    }

    fn initialize_frame_map(
        regions: &MemRegions,
        storage_range: PageRange<PhysFrame>,
        map: &mut [u8],
    ) {
        // Zero all the storage (i.e., set everything to free)
        for b in map.iter_mut() {
            *b = 0;
        }

        for r in regions.iter() {
            if r.ty != RegionType::Usable {
                for f in r.as_range().iter() {
                    Self::write_free(map, f, false);
                }
            }
        }

        for frame in storage_range.iter() {
            Self::write_free(map, frame, false);
        }
    }

    pub fn new(regions: MemRegions) -> Self {
        let mut end = PhysFrame::from_base_u64(0);

        for region in &*regions {
            kprintln!("{}", region);
            end = end.max(region.as_range().end())
        }

        let range = PhysFrame::range_exclusive(PhysFrame::from_base_u64(0), end);

        let (slice, used_frames) = Self::alloc_storage(range.len(), &regions);

        kprintln!(
            "Allocating space for PhysFrameAllocator: {:?} {}",
            slice.as_ptr_range(),
            used_frames
        );

        Self::initialize_frame_map(&regions, used_frames, slice);

        Self {}
    }
}
