use common::MemRegions;

pub struct PhysFrameAllocator {}

impl PhysFrameAllocator {
    pub fn new(regions: MemRegions) -> Self {
        for region in &*regions {
            kprintln!("{}", region)
        }

        PhysFrameAllocator {}
    }
}
