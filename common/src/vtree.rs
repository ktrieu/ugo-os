use crate::{
    addr::{Page, VirtPage, VirtPageRange},
    KMEM_START,
};

extern crate alloc;

use alloc::boxed::Box;

enum VTreeAreaType {
    KernelCode,
    Framebuffer,
}

pub struct VTreeArea {
    range: VirtPageRange,
    ty: VTreeAreaType,
}

// TODO: going to have to squeeze this down or increase the bootstrap slab alloc size
pub struct VTreeNode {
    area: VTreeArea,

    gap: u64,
    max_gap: u64,

    balance: u8,
    left: Option<Box<VTreeNode>>,
    right: Option<Box<VTreeNode>>,
}

pub struct VTree {
    head: Option<Box<VTreeNode>>,
    range: VirtPageRange,
}

impl VTree {
    pub fn new(areas: &[(u64, VTreeArea)]) -> Self {
        // This tree captures the entirety of virtual kernel space.
        let start = KMEM_START;

        let mut prev_addr = start;
        for (_gap, a) in areas {
            let node = V
        }

        Self {
            head: None,
            range: VirtPageRange::new(
                VirtPage::from_base_u64(start),
                VirtPage::from_base_u64(prev_addr),
            ),
        }
    }


}
