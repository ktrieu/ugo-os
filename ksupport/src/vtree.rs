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
#[derive(Debug)]
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
        let (head, range) = match &self.head {
            Some(head) => self.insert_at_rec(area, head.clone()),
            None => {
                let gap = VirtPage::range_exclusive(self.range.first(), area.range.first()).len();
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

    fn for_each_rec(&self, node: Arc<VTreeNode>, f: &mut impl FnMut(Arc<VTreeNode>)) {
        if let Some(left) = &node.left {
            self.for_each_rec(left.clone(), f);
        }

        f(node.clone());

        if let Some(right) = &node.right {
            self.for_each_rec(right.clone(), f);
        }
    }

    pub fn for_each(&self, f: &mut impl FnMut(Arc<VTreeNode>)) {
        if let Some(head) = &self.head {
            self.for_each_rec(head.clone(), &mut *f);
        }
    }
}

#[cfg(test)]
mod test {
    use common::{
        KMEM_START,
        addr::{Page, VirtPage},
    };

    use super::*;
    use std::vec::Vec;

    fn to_vec(tree: &VTree) -> Vec<Arc<VTreeNode>> {
        let mut vec = Vec::new();
        tree.for_each(&mut |n| vec.push(n.clone()));

        vec
    }

    #[test]
    fn test_insert_one() {
        let start = VirtPage::from_base_u64(KMEM_START + (3 * 4096));
        let range = VirtPage::range_length(start, 4);

        let areas = [VTreeArea {
            range,
            ty: VTreeAreaType::KernelCode,
        }];

        let tree = VTree::new(&areas);

        let nodes = to_vec(&tree);
        let head = nodes.get(0).unwrap();

        assert_eq!(head.area.range, range);
        assert_eq!(head.gap, 3);
        assert_eq!(head.max_gap, head.gap);
    }
}
