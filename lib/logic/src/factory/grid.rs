//! Grid validation helpers shared by factory placement handlers.
//!
//! Used in `place`, `move_node`, and `dismantle` so they don't have
//! duplicate rules.  Lives in `perlica-logic` so unit tests don't need a
//! running server.
//!
//! All geometry is in grid coordinates. Negative values are legal; the
//! bounds check is against the region's main mesh, which may also be at
//! negative coords (`FactoryMapTable.pos_x/pos_y`).

use super::{GridPos, GridRange};

/// Inclusive top-left, exclusive bottom-right, same convention as Rust
/// slice indexing.
pub fn is_in_bounds(pos: GridPos, range: GridRange) -> bool {
    let x_ok = pos.x >= range.x && pos.x < range.x + range.w as i32;
    let y_ok = pos.y >= range.y && pos.y < range.y + range.h as i32;
    x_ok && y_ok
}

/// Rejects buildings that would straddle the region boundary.
pub fn range_within(inner: GridRange, outer: GridRange) -> bool {
    let inner_x2 = inner.x + inner.w as i32;
    let inner_y2 = inner.y + inner.h as i32;
    let outer_x2 = outer.x + outer.w as i32;
    let outer_y2 = outer.y + outer.h as i32;
    inner.x >= outer.x && inner.y >= outer.y && inner_x2 <= outer_x2 && inner_y2 <= outer_y2
}

/// Rejects overlapping buildings in `place`.
pub fn ranges_overlap(a: GridRange, b: GridRange) -> bool {
    let a_x2 = a.x + a.w as i32;
    let a_y2 = a.y + a.h as i32;
    let b_x2 = b.x + b.w as i32;
    let b_y2 = b.y + b.h as i32;
    a.x < b_x2 && a_x2 > b.x && a.y < b_y2 && a_y2 > b.y
}

/// Conveyor / connection rule: share an edge, not just a corner.
// TODO(Clause 2): confirm live-server rules (corner-touch allowed? edge
// length ≥1?) and tighten the check once we know.
pub fn ranges_edge_adjacent(a: GridRange, b: GridRange) -> bool {
    if !ranges_overlap(a, b) {
        let a_x2 = a.x + a.w as i32;
        let a_y2 = a.y + a.h as i32;
        let b_x2 = b.x + b.w as i32;
        let b_y2 = b.y + b.h as i32;

        let horizontal_touch = (a_x2 == b.x || b_x2 == a.x) && a.y < b_y2 && a_y2 > b.y;
        let vertical_touch = (a_y2 == b.y || b_y2 == a.y) && a.x < b_x2 && a_x2 > b.x;

        return horizontal_touch || vertical_touch;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{GridPos, GridRange};

    #[test]
    fn in_bounds_basic() {
        let r = GridRange {
            x: 0,
            y: 0,
            w: 4,
            h: 4,
        };
        assert!(is_in_bounds(GridPos { x: 0, y: 0 }, r));
        assert!(is_in_bounds(GridPos { x: 3, y: 3 }, r));
        assert!(!is_in_bounds(GridPos { x: 4, y: 0 }, r));
        assert!(!is_in_bounds(GridPos { x: 0, y: -1 }, r));
    }

    #[test]
    fn in_bounds_negative_origin() {
        // map01_lv001 has pos_x=17, pos_y=-36, so this matters.
        let r = GridRange {
            x: -10,
            y: -10,
            w: 5,
            h: 5,
        };
        assert!(is_in_bounds(GridPos { x: -10, y: -10 }, r));
        assert!(is_in_bounds(GridPos { x: -6, y: -6 }, r));
        assert!(!is_in_bounds(GridPos { x: -11, y: 0 }, r));
    }

    #[test]
    fn range_within_strict() {
        let outer = GridRange {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        assert!(range_within(
            GridRange {
                x: 0,
                y: 0,
                w: 5,
                h: 5
            },
            outer
        ));
        assert!(range_within(
            GridRange {
                x: 5,
                y: 5,
                w: 5,
                h: 5
            },
            outer
        ));
        assert!(!range_within(
            GridRange {
                x: -1,
                y: 0,
                w: 5,
                h: 5
            },
            outer
        ));
        assert!(!range_within(
            GridRange {
                x: 6,
                y: 6,
                w: 5,
                h: 5
            },
            outer
        ));
    }

    #[test]
    fn overlap_symmetric() {
        let a = GridRange {
            x: 0,
            y: 0,
            w: 3,
            h: 3,
        };
        let b = GridRange {
            x: 2,
            y: 2,
            w: 3,
            h: 3,
        };
        assert!(ranges_overlap(a, b));
        assert!(ranges_overlap(b, a));

        let c = GridRange {
            x: 5,
            y: 5,
            w: 2,
            h: 2,
        };
        assert!(!ranges_overlap(a, c));
        assert!(!ranges_overlap(c, a));
    }

    #[test]
    fn edge_adjacent_rejects_corner_touch() {
        let a = GridRange {
            x: 0,
            y: 0,
            w: 2,
            h: 2,
        };
        // corner touch only, should be rejected
        let corner = GridRange {
            x: 2,
            y: 2,
            w: 2,
            h: 2,
        };
        assert!(!ranges_edge_adjacent(a, corner));

        // proper edge touch
        let edge = GridRange {
            x: 2,
            y: 0,
            w: 2,
            h: 2,
        };
        assert!(ranges_edge_adjacent(a, edge));

        // overlapping is NOT edge-adjacent
        let overlap = GridRange {
            x: 1,
            y: 0,
            w: 2,
            h: 2,
        };
        assert!(!ranges_edge_adjacent(a, overlap));
    }
}
