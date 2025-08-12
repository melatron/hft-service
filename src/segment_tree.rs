use std::ops::Add;
use tracing::{info, trace};

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

    /// Public update method that checks capacity and triggers a resize if needed.
    pub fn update(&mut self, index: usize, value: f64, all_values: &[f64]) {
        if index >= self.capacity {
            self.resize(index + 1, all_values);
        }
        self.update_internal(index, value);
    }

    /// Internal method for the actual update logic.
    fn update_internal(&mut self, mut index: usize, value: f64) {
        index += self.capacity;
        self.tree[index] = Node {
            min: value,
            max: value,
            sum: value,
            sum_of_squares: value * value,
            count: 1,
        };

        while index > 1 {
            index /= 2;
            let left_child = 2 * index;
            let right_child = 2 * index + 1;
            self.tree[index] = self.tree[left_child] + self.tree[right_child];
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
        assert!(node.min.is_infinite() && node.min.is_sign_positive());
    }

    #[test]
    fn test_single_element() {
        let mut tree = SegmentTree::new(10);
        let mut values = Vec::new();

        values.push(150.5);
        tree.update(0, 150.5, &values);

        let node = tree.query(0, 0);
        assert_eq!(node.count, 1);
        assert_float_eq(node.min, 150.5);
        assert_float_eq(node.max, 150.5);
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
        assert_float_eq(node.sum, 50.0);
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

        let node = tree.query(1, 3);
        assert_eq!(node.count, 3);
        assert_float_eq(node.min, 5.0);
        assert_float_eq(node.max, 20.0);
        assert_float_eq(node.sum, 40.0);
    }

    #[test]
    fn test_resizing() {
        // Start with a small capacity of 2
        let mut tree = SegmentTree::new(2);
        let mut values = Vec::new();
        // This data will trigger a resize
        let test_data = [10.0, 20.0, 5.0, 15.0];

        for (i, &v) in test_data.iter().enumerate() {
            values.push(v);
            tree.update(i, v, &values);
        }

        // After adding 4 elements, the capacity should have grown (e.g., to 4).
        assert!(tree.capacity >= 4);

        // Check if data is still correct after resizing and rebuilding.
        let node = tree.query(0, 3);
        assert_eq!(node.count, 4);
        assert_float_eq(node.sum, 50.0);
        assert_float_eq(node.min, 5.0);
        assert_float_eq(node.max, 20.0);
    }
}
