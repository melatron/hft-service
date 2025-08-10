use std::ops::Add;
use tracing::trace;

/// Represents a single node in the segment tree.
/// It stores aggregate data for a specific range of values.
#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub min: f64,
    pub max: f64,
    pub sum: f64,
    pub sum_of_squares: f64,
    pub count: u64,
}

/// Defines how to merge two child nodes into a parent node.
/// This operation is the core of the tree's aggregation logic.
impl Add for Node {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        // If one node is empty, return the other one.
        if self.count == 0 {
            return rhs;
        }
        if rhs.count == 0 {
            return self;
        }

        // Combine the metrics from both nodes.
        Self {
            min: self.min.min(rhs.min),
            max: self.max.max(rhs.max),
            sum: self.sum + rhs.sum,
            sum_of_squares: self.sum_of_squares + rhs.sum_of_squares,
            count: self.count + rhs.count,
        }
    }
}

/// The identity node, representing an empty range.
impl Default for Node {
    fn default() -> Self {
        Node {
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            sum: 0.0,
            sum_of_squares: 0.0,
            count: 0,
        }
    }
}

pub struct SegmentTree {
    tree: Vec<Node>,
    /// The number of leaf nodes, which is the capacity of the original data array.
    n: usize,
}

impl SegmentTree {
    pub fn new(capacity: usize) -> Self {
        SegmentTree {
            tree: vec![Node::default(); 2 * capacity],
            n: capacity,
        }
    }

    pub fn update(&mut self, mut i: usize, value: f64) {
        // Go to the leaf position.
        i += self.n;
        trace!(target_index = i, value, "Updating leaf node");

        // Update the leaf node.
        self.tree[i] = Node {
            min: value,
            max: value,
            sum: value,
            sum_of_squares: value * value,
            count: 1,
        };

        // Move up the tree, updating parents.
        while i > 1 {
            i /= 2;
            let left_child = 2 * i;
            let right_child = 2 * i + 1;
            trace!(
                parent_index = i,
                left_child = left_child,
                right_child = right_child,
                "Merging children to update parent"
            );
            self.tree[i] = self.tree[left_child] + self.tree[right_child];
        }
    }

    pub fn query(&self, mut l: usize, mut r: usize) -> Node {
        if l > r {
            return Node::default();
        }
        trace!(
            query_range_start = l,
            query_range_end = r,
            "Executing iterative query"
        );

        let mut res_left = Node::default();
        let mut res_right = Node::default();

        // Move to the leaf positions.
        l += self.n;
        r += self.n;

        while l <= r {
            // If l is a right child, include its value and move to the right.
            if l % 2 == 1 {
                trace!(index = l, "Including right child in left result");
                res_left = res_left + self.tree[l];
                l += 1;
            }
            // If r is a left child, include its value and move to the left.
            if r % 2 == 0 {
                trace!(index = r, "Including left child in right result");
                res_right = self.tree[r] + res_right;
                r -= 1;
            }
            // Move up to the parents.
            l /= 2;
            r /= 2;
        }

        trace!("Merging left and right results for final query response");
        res_left + res_right
    }
}
