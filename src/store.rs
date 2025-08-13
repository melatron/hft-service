use crate::{segment_tree::SegmentTree, AppError};
use dashmap::DashMap;

/// The initial capacity for the segment tree.
const STARTING_CAPACITY: usize = 1_000_000;
// The maximum number of unique symbols we can track.
const MAX_SYMBOLS: usize = 10;

/// The main store for all symbol data.
pub struct Store {
    pub symbols: DashMap<String, SymbolData>,
}

/// A complete statistics object, decoupled from the web response.
#[derive(Debug, Clone, Copy)]
pub struct SymbolStats {
    pub min: f64,
    pub max: f64,
    pub last: f64,
    pub avg: f64,
    pub var: f64,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            symbols: DashMap::new(),
        }
    }

    /// Adds a batch of values for a given symbol.
    /// This now contains the core update logic.
    pub fn add_batch(&self, symbol: &str, batch_values: &[f64]) -> Result<(), AppError> {
        // If the symbol doesn't exist yet and we are at capacity, reject the request.
        if !self.symbols.contains_key(symbol) && self.symbols.len() >= MAX_SYMBOLS {
            return Err(AppError::BadRequest(format!(
                "Maximum number of unique symbols ({}) reached.",
                MAX_SYMBOLS
            )));
        }

        let mut symbol_data_guard =
            self.symbols
                .entry(symbol.to_string())
                .or_insert_with(|| SymbolData {
                    values: Vec::new(),
                    tree: SegmentTree::new(STARTING_CAPACITY),
                });

        let SymbolData { values, tree } = &mut *symbol_data_guard;

        for value in batch_values {
            values.push(*value);
            let new_index = values.len() - 1;
            tree.update(new_index, *value, values);
        }

        Ok(())
    }

    /// Retrieves and calculates full statistics for a given symbol and window.
    /// This now contains the final avg/var calculations.
    pub fn get_stats(&self, symbol: &str, window_size: usize) -> Result<SymbolStats, AppError> {
        let data = self
            .symbols
            .get(symbol)
            .ok_or_else(|| AppError::SymbolNotFound(symbol.to_string()))?;

        let total_points = data.values.len();
        if total_points == 0 {
            return Err(AppError::NotEnoughData);
        }

        // Use the full dataset if the window is larger than available points.
        let actual_window_size = window_size.min(total_points);
        if actual_window_size == 0 {
            return Err(AppError::NotEnoughData);
        }

        let start_index = total_points.saturating_sub(actual_window_size);
        let stats_node = data.tree.query(start_index, total_points - 1);

        if stats_node.count == 0 {
            return Err(AppError::NotEnoughData);
        }

        let last_value = data.values[total_points - 1];
        let avg = stats_node.mean;
        let variance = if stats_node.count > 0 {
            stats_node.m2 / stats_node.count as f64
        } else {
            0.0
        };

        Ok(SymbolStats {
            min: stats_node.min,
            max: stats_node.max,
            last: last_value,
            avg,
            var: variance,
        })
    }
}

/// Data specific to one financial symbol.
pub struct SymbolData {
    pub values: Vec<f64>,
    pub tree: SegmentTree,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A helper for comparing floating-point numbers in tests.
    fn fuzzy_assert_eq(a: f64, b: f64) {
        let epsilon = 1e-9;
        assert!((a - b).abs() < epsilon, "Expected {}, got {}", b, a);
    }

    #[test]
    fn test_get_stats_with_window_larger_than_data() {
        // Arrange
        let store = Store::new();
        let symbol = "TEST";
        let values = vec![10.0, 20.0, 5.0, 15.0, 25.0];
        store.add_batch(symbol, &values).unwrap();

        // Act: Request a window of 100, but only 5 points are available.
        let result = store.get_stats(symbol, 100);

        // Assert
        assert!(result.is_ok());
        let stats = result.unwrap();

        // Verify the stats are calculated correctly on the 5 available points.
        assert_eq!(stats.min, 5.0);
        assert_eq!(stats.max, 25.0);
        assert_eq!(stats.last, 25.0);
        fuzzy_assert_eq(stats.avg, 15.0);
        fuzzy_assert_eq(stats.var, 50.0);
    }

    #[test]
    fn test_get_stats_for_nonexistent_symbol() {
        // Arrange
        let store = Store::new();

        // Act
        let result = store.get_stats("NO-SUCH-SYMBOL", 100);

        // Assert
        assert!(result.is_err());
        match result.err().unwrap() {
            AppError::SymbolNotFound(_) => {} // Correct error type
            _ => panic!("Expected SymbolNotFound error"),
        }
    }
}
