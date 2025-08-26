use core::ptr::{slice_from_raw_parts_mut, write_bytes};

use common::{
    addr::{Address, Page, PageRange, PhysAddr, PhysFrame},
    MemRegions, RegionType,
};

enum FrameStatus {
    Allocated,
    Free,
}

struct Bitmap {
    map: &'static mut [u8],
}

impl Bitmap {
    fn new(map: &'static mut [u8]) -> Self {
        Self { map }
    }

    fn write(&mut self, frame: PhysFrame, status: FrameStatus) {
        let byte_idx = (frame.idx() / 8) as usize;
        let bit_idx = frame.idx() % 8;

        let byte = self.map[byte_idx];

        match status {
            FrameStatus::Allocated => {
                // Set the correct bit.
                let mask: u8 = 1 << (7 - bit_idx);
                self.map[byte_idx] = byte | mask;
            }
            FrameStatus::Free => {
                // Clear the 7 - bit_idxth bit. We subtract from 7 because
                // we want the 0th bit to be on the left, aka the high bit.
                let mask: u8 = !(1 << (7 - bit_idx));
                self.map[byte_idx] = byte & mask
            }
        }
    }

    fn read(&self, frame: PhysFrame) -> FrameStatus {
        let byte_idx = (frame.idx() / 8) as usize;
        let bit_idx = frame.idx() % 8;

        let byte = self.map[byte_idx];

        let byte = byte >> 7 - bit_idx;
        let masked = byte & 1;

        if masked == 0 {
            FrameStatus::Free
        } else {
            FrameStatus::Allocated
        }
    }
}

pub struct PhysFrameAllocator {
    bitmap: Bitmap,
    range: PageRange<PhysFrame>,

    allocated: u64,
}

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
            let ptr = start_phys.as_direct_mapped().as_u8_ptr_mut();
            let len = required_bytes as usize;

            write_bytes(ptr, 0, len);

            // We keep physical addresses for later bookkeeping, but the actual address we use needs to be virtual.
            // Just use direct mapping for now.
            let slice =
                slice_from_raw_parts_mut(start_phys.as_direct_mapped().as_u8_ptr_mut(), len);

            let phys_range = PhysFrame::range_inclusive(
                PhysFrame::from_containing_addr(start_phys),
                PhysFrame::from_containing_addr(last_phys),
            );

            (&mut *slice, phys_range)
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

        let mut bitmap = Bitmap::new(slice);
        let mut allocated = 0;

        // Mark existing used memory.
        for r in regions.iter() {
            if r.ty != RegionType::Usable {
                for f in r.as_range().iter() {
                    bitmap.write(f, FrameStatus::Allocated);
                    allocated += 1;
                }
            }
        }

        for f in used_frames.iter() {
            bitmap.write(f, FrameStatus::Allocated);
            allocated += 1;
        }

        Self {
            bitmap,
            range,
            allocated,
        }
    }

    pub fn alloc_frame(&mut self) -> Option<PhysFrame> {
        for f in self.range.iter() {
            if matches!(self.bitmap.read(f), FrameStatus::Free) {
                self.bitmap.write(f, FrameStatus::Allocated);
                self.allocated += 1;

                return Some(f);
            }
        }

        None
    }

    pub fn free_frame(&mut self, frame: PhysFrame) {
        self.bitmap.write(frame, FrameStatus::Free);
        self.allocated -= 1;
    }

    pub fn print_stats(&self) {
        kprintln!(
            "physical memory: {} / {} frames allocated.",
            self.allocated,
            self.range.len()
        );
    }
}
