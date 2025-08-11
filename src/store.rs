use crate::{
    segment_tree::{Node, SegmentTree},
    AppError,
};
use dashmap::DashMap;

/// The main store for all symbol data.
pub struct Store {
    pub symbols: DashMap<String, SymbolData>,
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

    /// This function returns our specific AppError type.
    pub fn get_stats(&self, symbol: &str, window_size: usize) -> Result<(Node, f64), AppError> {
        let data = self
            .symbols
            .get(symbol)
            .ok_or_else(|| AppError::SymbolNotFound(symbol.to_string()))?;

        let total_points = data.values.len();
        if total_points == 0 {
            return Err(AppError::NotEnoughData);
        }

        // If we don't have enough points for the requested window, return an error.
        if total_points < window_size {
            return Err(AppError::NotEnoughData);
        }

        let start_index = total_points.saturating_sub(window_size);
        let stats_node = data.tree.query(start_index, total_points - 1);

        if stats_node.count == 0 {
            return Err(AppError::NotEnoughData);
        }

        // This is now safe because we've confirmed total_points > 0.
        let last_value = data.values[total_points - 1];

        Ok((stats_node, last_value))
    }
}

/// Data specific to one financial symbol.
pub struct SymbolData {
    pub values: Vec<f64>,
    pub tree: SegmentTree,
}
