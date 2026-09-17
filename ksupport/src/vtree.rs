use common::{
    KMEM_START,
    addr::{Page, VirtPage, VirtPageRange},
};

extern crate alloc;

use alloc::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VTreeAreaType {
    KernelCode,
    Framebuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VTreeArea {
    range: VirtPageRange,
    ty: VTreeAreaType,
}

// TODO: going to have to squeeze this down or increase the bootstrap slab alloc size
pub struct VTreeNode {
    area: VTreeArea,

    // Gap between the left of this and the next node (or the start of the KMEM area)
    gap: u64,
    // Max of all values of gap below this node.
    max_gap: u64,

    balance: u8,
    left: Option<Arc<VTreeNode>>,
    right: Option<Arc<VTreeNode>>,
}

pub struct VTree {
    head: Option<Arc<VTreeNode>>,
    range: VirtPageRange,
}

impl VTree {
    fn insert_at_rec(
        &mut self,
        area: &VTreeArea,
        node: Arc<VTreeNode>,
    ) -> (Arc<VTreeNode>, VirtPageRange) {
        // nothing ever happens
        (node, area.range)
    }

    pub fn insert(&mut self, area: &VTreeArea) {
        let range_start = self.range.first().base_u64();

        let (head, range) = match &self.head {
            Some(head) => self.insert_at_rec(area, head.clone()),
            None => {
                let gap = area.range.first().base_u64() - range_start;
                let node = VTreeNode {
                    area: *area,
                    gap,
                    max_gap: gap,
                    balance: 0,
                    left: None,
                    right: None,
                };
                let range = VirtPage::range_exclusive(self.range.first(), area.range.end());
                (Arc::new(node), range)
            }
        };

        self.head = Some(head);
        self.range = range;
    }

    pub fn new(areas: &[VTreeArea]) -> Self {
        // This tree captures the entirety of virtual kernel space.
        let start = VirtPage::from_base_u64(KMEM_START);
        let end = start;

        let mut tree = Self {
            head: None,
            range: VirtPage::range_exclusive(start, end),
        };

        for a in areas {
            tree.insert(a);
        }

        tree
    }
}
