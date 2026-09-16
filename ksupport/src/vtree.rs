use common::{
    KMEM_START,
    addr::{Page, VirtPage, VirtPageRange},
};

extern crate alloc;

use alloc::boxed::Box;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VTreeData {
    area: VTreeArea,
    // Gap between the left of this and the next node (or the start of the KMEM area)
    gap: u64,
    // Max of all values of gap below this node.
    max_gap: u64,

    balance: u8,
}

// TODO: going to have to squeeze this down or increase the bootstrap slab alloc size
#[derive(Debug)]
pub struct VTreeNode {
    data: VTreeData,
    left: Option<Box<VTreeNode>>,
    right: Option<Box<VTreeNode>>,
}

pub struct VTree {
    head: Option<Box<VTreeNode>>,
    range: VirtPageRange,
}

impl VTree {
    fn insert_at_rec(area: &VTreeArea, node: &mut VTreeNode) {}

    pub fn insert(&mut self, area: &VTreeArea) {
        match &mut self.head {
            Some(head) => {
                Self::insert_at_rec(area, head);
            }
            None => {
                let gap = VirtPage::range_exclusive(self.range.first(), area.range.first()).len();
                let node = VTreeNode {
                    data: VTreeData {
                        area: *area,
                        gap,
                        max_gap: gap,
                        balance: 0,
                    },
                    left: None,
                    right: None,
                };
                let range = VirtPage::range_exclusive(self.range.first(), area.range.end());

                self.head = Some(Box::new(node));
                self.range = range;
            }
        };
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

    fn for_each_rec(&self, node: &VTreeNode, f: &mut impl FnMut(&VTreeNode)) {
        if let Some(left) = &node.left {
            self.for_each_rec(left, f);
        }

        f(node);

        if let Some(right) = &node.right {
            self.for_each_rec(right, f);
        }
    }

    pub fn for_each(&self, f: &mut impl FnMut(&VTreeNode)) {
        if let Some(head) = &self.head {
            self.for_each_rec(head, &mut *f);
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

    fn to_vec(tree: &VTree) -> Vec<VTreeData> {
        let mut vec = Vec::new();
        tree.for_each(&mut |n| vec.push(n.data));

        vec
    }

    #[test]
    fn test_insert_one() {
        let start = VirtPage::from_base_u64(KMEM_START + (3 * 4096));
        let range = VirtPage::range_length(start, 4);

        let one_area = VTreeArea {
            range,
            ty: VTreeAreaType::KernelCode,
        };

        let areas = [one_area];

        let tree = VTree::new(&areas);

        let nodes = to_vec(&tree);
        assert_eq!(
            nodes,
            vec![VTreeData {
                area: one_area,
                gap: 3,
                max_gap: 3,
                balance: 0,
            }]
        )
    }
}
