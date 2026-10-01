use core::cmp::Ordering;

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

    height: u8,
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
    fn insert_at_rec(area: &VTreeArea, node: &mut VTreeNode) -> Option<VTreeData> {
        let branch = match area
            .range
            .partial_cmp(&node.data.area.range)
            .expect("no overlapping ranges should be present")
        {
            Ordering::Less => &mut node.left,
            Ordering::Equal => {
                // we should raise an error out of here for this case
                return None;
            }
            Ordering::Greater => &mut node.right,
        };

        let inserted = match branch.as_mut() {
            Some(child) => {
                let result = Self::insert_at_rec(area, child);
                result
            }
            None => {
                let data = VTreeData {
                    area: *area,
                    // TODO: actually calculate this without it being aggravating.
                    gap: 0,
                    max_gap: 0,
                    height: 1,
                };
                *branch = Some(Box::new(VTreeNode {
                    data,
                    left: None,
                    right: None,
                }));

                Some(data)
            }
        };

        // Be sure to update this node, if needed before return
        if let Some(data) = inserted {
            let left_height = node.left.as_ref().map(|n| n.data.height).unwrap_or(0);
            let right_height = node.right.as_ref().map(|n| n.data.height).unwrap_or(0);
            node.data.height = left_height.max(right_height) + 1;

            node.data.max_gap = node.data.max_gap.max(data.gap);
        }

        inserted
    }

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
                        height: 0,
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

    use crate::vtree::VTreeAreaType::KernelCode;

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
                height: 0,
            }]
        )
    }

    fn verify_height_rec(node: &VTreeNode) -> u8 {
        let left_height = node
            .left
            .as_ref()
            .map(|n| verify_height_rec(&n))
            .unwrap_or(0);
        let right_height = node
            .right
            .as_ref()
            .map(|n| verify_height_rec(&n))
            .unwrap_or(0);

        let verified = left_height.max(right_height) + 1;

        assert_eq!(verified, node.data.height);

        verified
    }

    fn verify_heights(tree: &VTree) {
        if let Some(node) = &tree.head {
            verify_height_rec(&node);
        }
    }

    #[test]
    fn test_multiple_insert() {
        // Generate some ranges
        let mut start = VirtPage::from_base_u64(KMEM_START);
        let mut ranges = Vec::new();
        for i in 0..5 {
            let range = VirtPage::range_length(start, 2);
            ranges.push(range);
            start = range.end();
        }

        let mut tree = VTree::new(&[]);

        for r in &ranges {
            tree.insert(&VTreeArea {
                range: *r,
                ty: KernelCode,
            });
        }

        let v: Vec<VirtPageRange> = to_vec(&tree).iter().map(|n| n.area.range).collect();

        assert_eq!(v, ranges);

        verify_heights(&tree);
    }
}
