use crate::{
    segment_tree::{Node, SegmentTree},
    AppError,
};
use std::collections::HashMap;

/// The main store for all symbol data.
pub struct Store {
    pub symbols: HashMap<String, SymbolData>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }

    /// This function returns our specific AppError type.
    pub fn get_stats(&self, symbol: &str, n: usize) -> Result<(Node, f64), AppError> {
        let data = self
            .symbols
            .get(symbol)
            .ok_or_else(|| AppError::SymbolNotFound(symbol.to_string()))?;

        let total_points = data.values.len();
        if total_points == 0 {
            return Err(AppError::NotEnoughData);
        }

        let start_index = total_points.saturating_sub(n);
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
