use core::cmp::Ordering;

use common::{
    KMEM_START, PAGE_SIZE,
    addr::{Page, VirtPage, VirtPageRange},
};

extern crate alloc;

use alloc::boxed::Box;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VTreeAreaType {
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
    // Gap between the left of this and the next node (or the start of the tree's area)
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

impl VTreeNode {
    fn gap_range(&self) -> VirtPageRange {
        let start = VirtPage::from_base_u64(
            self.data.area.range.first().base_u64() - (self.data.gap * PAGE_SIZE),
        );

        VirtPage::range_length(start, self.data.gap)
    }
}

pub struct VTree {
    head: Option<Box<VTreeNode>>,
    range: VirtPageRange,
}

impl VTree {
    fn insert_at_rec(area: &VTreeArea, node: &mut VTreeNode, gap: u64) -> Option<VTreeData> {
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
                let result = Self::insert_at_rec(area, child, gap);
                result
            }
            None => {
                let data = VTreeData {
                    area: *area,
                    gap,
                    max_gap: gap,
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
        if inserted.is_some() {
            let left_height = node.left.as_ref().map(|n| n.data.height).unwrap_or(0);
            let right_height = node.right.as_ref().map(|n| n.data.height).unwrap_or(0);
            node.data.height = left_height.max(right_height) + 1;

            let left_max_gap = node.left.as_ref().map(|n| n.data.max_gap).unwrap_or(0);
            let right_max_gap = node.right.as_ref().map(|n| n.data.max_gap).unwrap_or(0);
            node.data.max_gap = left_max_gap.max(right_max_gap).max(node.data.gap);
        }

        inserted
    }

    pub fn insert(&mut self, area: &VTreeArea, gap: u64) {
        match &mut self.head {
            Some(head) => {
                Self::insert_at_rec(area, head, gap);
            }
            None => {
                let node = VTreeNode {
                    data: VTreeData {
                        area: *area,
                        gap,
                        max_gap: gap,
                        height: 1,
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

    fn search_mut_rec<F: Fn(&VTreeData) -> bool + Clone>(
        node: &mut VTreeNode,
        stop: F,
    ) -> Option<&mut VTreeNode> {
        if stop(&node.data) {
            Some(node)
        } else {
            Self::search_mut_rec(node.left.as_mut()?, stop.clone())
                .or(Self::search_mut_rec(node.right.as_mut()?, stop.clone()))
        }
    }

    fn search_mut<F: Fn(&VTreeData) -> bool + Clone>(&mut self, stop: F) -> Option<&mut VTreeNode> {
        match &mut self.head {
            Some(h) => Self::search_mut_rec(h, stop),
            None => None,
        }
    }

    pub fn alloc_range(&mut self, requested: u64, ty: VTreeAreaType) {
        let target = match self.search_mut(|d: &VTreeData| d.max_gap > requested) {
            Some(t) => t,
            None => return,
        };

        let new_range = VirtPage::range_length(target.gap_range().first(), requested);

        let new_area = VTreeArea {
            range: new_range,
            ty,
        };

        // And we've eaten `requested` pages from this gap.
        target.data.gap -= requested;

        // Since we've carved this new range from the left edge of the existing gap the new gap is 0.
        self.insert(&new_area, 0);
    }

    pub fn new(areas: &[VTreeArea]) -> Self {
        // This tree captures the entirety of virtual kernel space.
        let kmem_start = VirtPage::from_base_u64(KMEM_START);
        let end = kmem_start;

        let mut tree = Self {
            head: None,
            range: VirtPage::range_exclusive(kmem_start, end),
        };

        let mut prev_end = kmem_start;
        for a in areas {
            let gap = (a.range.first().base_u64() - prev_end.base_u64()) / PAGE_SIZE;

            tree.insert(a, gap);

            prev_end = a.range.end();
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

    fn verify_gaps(tree: &VTree) {
        let datas = to_vec(tree);

        let mut prev = tree.range.first();

        for d in datas {
            let gap = (d.area.range.first().base_u64() - prev.base_u64()) / PAGE_SIZE;
            assert_eq!(gap, d.gap);
            prev = d.area.range.end();
        }
    }

    fn verify_max_gap_rec(node: &VTreeNode) {
        let left_max = node.left.as_ref().map(|n| n.data.max_gap).unwrap_or(0);
        let right_max = node.right.as_ref().map(|n| n.data.max_gap).unwrap_or(0);

        let computed_max = left_max.max(right_max).max(node.data.gap);

        assert_eq!(computed_max, node.data.max_gap);
        assert!(node.data.gap <= computed_max);
    }

    fn verify_max_gaps(tree: &VTree) {
        if let Some(node) = &tree.head {
            verify_max_gap_rec(&node)
        }
    }

    fn verify(tree: &VTree) {
        verify_gaps(tree);
        verify_heights(tree);
        verify_max_gaps(tree);
    }

    #[test]
    fn test_init_one() {
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
                height: 1,
            }]
        );

        verify(&tree)
    }

    #[test]
    fn test_multiple_init() {
        // Generate some ranges
        let mut start = VirtPage::from_base_u64(KMEM_START + (PAGE_SIZE * 2));
        let mut init_areas = Vec::new();
        for _ in 0..5 {
            let range = VirtPage::range_length(start, 2);
            let area = VTreeArea {
                range,
                ty: KernelCode,
            };
            init_areas.push(area);
            start = range.end();
        }

        let tree = VTree::new(&init_areas);

        let v: Vec<VTreeData> = to_vec(&tree);

        let areas: Vec<VTreeArea> = v.iter().map(|d| d.area).collect();
        assert_eq!(init_areas, areas);

        verify(&tree)
    }

    #[test]
    fn alloc_from_gap() {
        let start = VirtPage::from_base_u64(KMEM_START + (PAGE_SIZE * 4));
        let range = VirtPage::range_length(start, 2);

        let area = VTreeArea {
            range,
            ty: KernelCode,
        };

        let mut tree = VTree::new(&[area]);
        tree.alloc_range(1, VTreeAreaType::KernelCode);

        let areas = to_vec(&tree);

        verify(&tree);

        assert_eq!(areas[0].area.range.len(), 1);
    }
}
