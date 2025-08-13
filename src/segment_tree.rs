use std::ops::Add;
use tracing::{info, trace};

/// Represents a single node in the segment tree.
/// It stores aggregate data for a specific range of values.

#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub min: f64,
    pub max: f64,
    pub count: u64,

    // Fields for Welford's algorithm
    pub mean: f64,
    pub m2: f64,
}

impl Default for Node {
    fn default() -> Self {
        Node {
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }
}

/// Defines how to merge two child nodes into a parent node.
/// This operation is the core of the tree's aggregation logic.
impl Add for Node {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        if self.count == 0 {
            return rhs;
        }
        if rhs.count == 0 {
            return self;
        }

        let combined_count = self.count + rhs.count;
        let delta = rhs.mean - self.mean;

        let new_mean = self.mean + delta * (rhs.count as f64 / combined_count as f64);

        // This is the core formula for combining the sum of squared differences (m2)
        let new_m2 = self.m2
            + rhs.m2
            + delta.powi(2) * (self.count as f64 * rhs.count as f64 / combined_count as f64);

        Self {
            min: self.min.min(rhs.min),
            max: self.max.max(rhs.max),
            count: combined_count,
            mean: new_mean,
            m2: new_m2,
        }
    }
}

pub struct SegmentTree {
    tree: Vec<Node>,
    capacity: usize,
}

impl SegmentTree {
    pub fn new(capacity: usize) -> Self {
        SegmentTree {
            tree: vec![Node::default(); 2 * capacity],
            capacity,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn update(&mut self, index: usize, value: f64, all_values: &[f64]) {
        if index >= self.capacity {
            self.resize(index + 1, all_values);
        }
        self.update_internal(index, value);
    }

    fn update_internal(&mut self, mut index: usize, value: f64) {
        index += self.capacity;
        // A single point has a mean equal to its value and a variance (m2) of 0.
        self.tree[index] = Node {
            min: value,
            max: value,
            count: 1,
            mean: value,
            m2: 0.0,
        };

        while index > 1 {
            index /= 2;
            self.tree[index] = self.tree[2 * index] + self.tree[2 * index + 1];
        }
    }

    /// Resizes the tree by creating a new, larger tree and rebuilding it.
    /// This is an O(N * log N) operation.
    fn resize(&mut self, required_capacity: usize, all_values: &[f64]) {
        let new_capacity = (self.capacity * 2).max(required_capacity);
        info!(
            old_capacity = self.capacity,
            new_capacity, "Resizing SegmentTree"
        );

        self.capacity = new_capacity;
        self.tree = vec![Node::default(); 2 * new_capacity];

        // Rebuild the tree with all existing values.
        for (i, &v) in all_values.iter().enumerate() {
            self.update_internal(i, v);
        }
    }

    /// Queries the tree for an aggregate Node over the given range [left, right].
    pub fn query(&self, mut left: usize, mut right: usize) -> Node {
        if left > right {
            return Node::default();
        }
        trace!(
            query_range_start = left,
            query_range_end = right,
            "Executing iterative query"
        );

        let mut res_left = Node::default();
        let mut res_right = Node::default();

        // Move to the leaf positions.
        left += self.capacity;
        right += self.capacity;

        while left <= right {
            // If left is a right child, include its value and move to the right.
            if left % 2 == 1 {
                trace!(index = left, "Including right child in left result");
                res_left = res_left + self.tree[left];
                left += 1;
            }
            // If right is a left child, include its value and move to the left.
            if right % 2 == 0 {
                trace!(index = right, "Including left child in right result");
                res_right = self.tree[right] + res_right;
                right -= 1;
            }
            // Move up to the parents.
            left /= 2;
            right /= 2;
        }

        trace!("Merging left and right results for final query response");
        res_left + res_right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-9;

    fn assert_float_eq(a: f64, b: f64) {
        assert!(
            (a - b).abs() < EPSILON,
            "Assertion failed: Expected {}, got {}",
            b,
            a
        );
    }

    #[test]
    fn test_empty_query() {
        let tree = SegmentTree::new(10);
        let node = tree.query(0, 5);
        assert_eq!(node.count, 0);
    }

    #[test]
    fn test_single_element() {
        let mut tree = SegmentTree::new(10);
        let mut values = Vec::new();

        values.push(150.5);
        tree.update(0, 150.5, &values);

        let node = tree.query(0, 0);
        assert_eq!(node.count, 1);
        assert_float_eq(node.mean, 150.5);
        // The m2 (sum of squared differences from the mean) of a single point is 0.
        assert_float_eq(node.m2, 0.0);
    }

    #[test]
    fn test_multiple_elements_full_range() {
        let mut tree = SegmentTree::new(10);
        let mut values = Vec::new();
        let test_data = [10.0, 20.0, 5.0, 15.0];

        for (i, &v) in test_data.iter().enumerate() {
            values.push(v);
            tree.update(i, v, &values);
        }

        let node = tree.query(0, 3);
        assert_eq!(node.count, 4);
        assert_float_eq(node.min, 5.0);
        assert_float_eq(node.max, 20.0);

        // Expected mean: (10 + 20 + 5 + 15) / 4 = 12.5
        assert_float_eq(node.mean, 12.5);

        // Expected m2 = (10-12.5)^2 + (20-12.5)^2 + (5-12.5)^2 + (15-12.5)^2
        // = 6.25 + 56.25 + 56.25 + 6.25 = 125.0
        assert_float_eq(node.m2, 125.0);

        // We can also verify the final variance
        let variance = node.m2 / node.count as f64;
        assert_float_eq(variance, 31.25);
    }

    #[test]
    fn test_multiple_elements_sub_range() {
        let mut tree = SegmentTree::new(10);
        let mut values = Vec::new();
        let test_data = [10.0, 20.0, 5.0, 15.0, 25.0];

        for (i, &v) in test_data.iter().enumerate() {
            values.push(v);
            tree.update(i, v, &values);
        }

        // Query for the sub-range [20.0, 5.0, 15.0]
        let node = tree.query(1, 3);
        assert_eq!(node.count, 3);
        assert_float_eq(node.min, 5.0);
        assert_float_eq(node.max, 20.0);

        // Expected mean: (20 + 5 + 15) / 3 = 13.333...
        let expected_mean = 40.0 / 3.0;
        assert_float_eq(node.mean, expected_mean);

        // Expected m2 = (20-mean)^2 + (5-mean)^2 + (15-mean)^2 = 72.22... + 72.22... + 2.77...
        let expected_m2 = (20.0 - expected_mean).powi(2)
            + (5.0 - expected_mean).powi(2)
            + (15.0 - expected_mean).powi(2);
        assert_float_eq(node.m2, expected_m2);
    }

    #[test]
    fn test_resizing_with_welford() {
        let mut tree = SegmentTree::new(2);
        let mut values = Vec::new();
        let test_data = [10.0, 20.0, 5.0, 15.0];

        for (i, &v) in test_data.iter().enumerate() {
            values.push(v);
            tree.update(i, v, &values);
        }

        assert!(tree.capacity >= 4);

        let node = tree.query(0, 3);
        assert_eq!(node.count, 4);
        assert_float_eq(node.mean, 12.5);
        assert_float_eq(node.m2, 125.0);
    }

    #[test]
    fn test_numerical_stability_with_large_offset() {
        let mut tree = SegmentTree::new(4);
        let mut values = Vec::new();

        // Use a very large offset to simulate real-world data far from zero.
        const OFFSET: f64 = 1_000_000_000.0;

        // The actual variance comes from these small deviations (+1.0 and -1.0).
        let test_data = [OFFSET + 1.0, OFFSET - 1.0, OFFSET + 1.0, OFFSET - 1.0];

        for (i, &v) in test_data.iter().enumerate() {
            values.push(v);
            tree.update(i, v, &values);
        }

        // Query for the full range of data.
        let node = tree.query(0, 3);

        // The mean should be exactly the offset.
        assert_float_eq(node.mean, OFFSET);

        // The actual variance is simple to calculate by hand:
        // It's the variance of [+1, -1, +1, -1], which is 1.0.
        // var = ((1-0)^2 + (-1-0)^2 + (1-0)^2 + (-1-0)^2) / 4 = (1+1+1+1)/4 = 1.0
        let variance = node.m2 / node.count as f64;

        // A naive sum-of-squares approach would likely fail this assertion due to
        // catastrophic cancellation, returning 0.0 or another incorrect value.
        // Welford's algorithm passes.
        assert_float_eq(variance, 1.0);
    }
}
