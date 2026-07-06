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

fn corners(r: GridRange) -> (i32, i32, i32, i32) {
    (r.x, r.y, r.x + r.w as i32, r.y + r.h as i32)
}

pub fn range_within(inner: GridRange, outer: GridRange) -> bool {
    let (ix, iy, ix2, iy2) = corners(inner);
    let (ox, oy, ox2, oy2) = corners(outer);
    ix >= ox && iy >= oy && ix2 <= ox2 && iy2 <= oy2
}

pub fn ranges_overlap(a: GridRange, b: GridRange) -> bool {
    let (ax, ay, ax2, ay2) = corners(a);
    let (bx, by, bx2, by2) = corners(b);
    ax < bx2 && ax2 > bx && ay < by2 && ay2 > by
}

pub fn ranges_edge_adjacent(a: GridRange, b: GridRange) -> bool {
    if !ranges_overlap(a, b) {
        let (ax, ay, ax2, ay2) = corners(a);
        let (bx, _by, bx2, by2) = corners(b);
        let horizontal_touch = (ax2 == bx || bx2 == ax) && ay < by2 && ay2 > b.y;
        let vertical_touch = (ay2 == b.y || by2 == ay) && ax < bx2 && ax2 > b.x;
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
