/// 2-D spatial grid for fast radius queries.
///
/// Objects are bucketed by their XZ world position into fixed-size cells.
/// Querying a radius walks only the cells that overlap a square centred on
/// the query point, typically a tiny fraction of the full list.
///
/// The Y axis is intentionally ignored here; callers do the precise 3-D
/// distance check on the small candidate set the grid returns.
///
/// # Building
/// ```
/// use perlica_logic::spatial::SpatialGrid;
///
/// let positions = vec![(10.0_f32, 5.0_f32), (100.0, 200.0)];
/// let grid = SpatialGrid::build(positions.into_iter(), 20.0);
/// assert_eq!(grid.len(), 2);
/// ```
///
/// # Querying
/// ```
/// use perlica_logic::spatial::SpatialGrid;
///
/// let positions = vec![(0.0_f32, 0.0_f32), (500.0, 500.0)];
/// let grid = SpatialGrid::build(positions.into_iter(), 20.0);
///
/// let hits = grid.query_radius_indices(0.0, 0.0, 80.0);
/// assert!(hits.contains(&0));  // nearby point included
/// assert!(!hits.contains(&1)); // far point excluded
/// ```
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<usize>>,
    count: usize,
}

impl SpatialGrid {
    pub fn build(positions: impl Iterator<Item = (f32, f32)>, cell_size: f32) -> Self {
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let mut count = 0usize;
        for (idx, (x, z)) in positions.enumerate() {
            cells
                .entry(cell_key(x, z, cell_size))
                .or_default()
                .push(idx);
            count += 1;
        }
        Self {
            cell_size,
            cells,
            count,
        }
    }

    /// Returns indices (into the original slice) of every object whose cell
    /// overlaps the square `[cx±radius, cz±radius]`.
    ///
    /// This is a *conservative* approximation, it never misses a candidate
    /// within `radius`, but may return a few extras from corner cells.
    /// The caller is responsible for the precise distance check.
    pub fn query_radius_indices(&self, cx: f32, cz: f32, radius: f32) -> Vec<usize> {
        let steps = (radius / self.cell_size).ceil() as i32;
        let base_x = (cx / self.cell_size).floor() as i32;
        let base_z = (cz / self.cell_size).floor() as i32;

        // Pre-size: in the worst case (dense map, tiny cells) every cell is
        // occupied.  With a reasonable cell_size the result is small.
        let mut result = Vec::new();
        for dx in -steps..=steps {
            for dz in -steps..=steps {
                if let Some(indices) = self.cells.get(&(base_x + dx, base_z + dz)) {
                    result.extend_from_slice(indices);
                }
            }
        }
        result
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[inline]
fn cell_key(x: f32, z: f32, cell_size: f32) -> (i32, i32) {
    (
        (x / cell_size).floor() as i32,
        (z / cell_size).floor() as i32,
    )
}
