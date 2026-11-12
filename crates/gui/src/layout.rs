//! Physical arrangement of keys and knobs for drawing the keypad.
//!
//! The keypad only reports *how many* keys and knobs it has, not how they are
//! arranged. Known models use the arrangement drawn by the vendor tool (its
//! embedded layout pictures); other sizes fall back to a sensible grid.

/// Where the knobs sit relative to the key grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnobSide {
    /// A row of knobs above the keys.
    Top,
    /// A cluster of knobs to the right of the keys (up to 2 per row).
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadLayout {
    /// Keys per row.
    pub cols: usize,
    pub knob_side: KnobSide,
    /// True when the arrangement comes from a known model (vendor drawing or hardware).
    pub known: bool,
}

/// Arrangement for a pad with `keys` keys and `knobs` knobs.
pub fn pad_layout(keys: usize, knobs: usize) -> PadLayout {
    use KnobSide::{Right, Top};
    let known = |cols, knob_side| PadLayout { cols, knob_side, known: true };
    match (keys, knobs) {
        // From the vendor tool's layout pictures.
        (3, 1) => known(3, Right),  // 3 keys in a row, knob on the right
        (5, 0) => known(5, Top),    // 5 keys in a row
        (6, 0) => known(3, Top),    // 3 × 2
        (6, 2) => known(3, Top),    // 2 knobs above a 3 × 2 grid
        (9, 3) => known(3, Right),  // 3 × 3, knobs on the right
        (12, 2) => known(4, Right), // 4 × 3, knobs on the right
        (15, 3) => known(5, Right), // 5 × 3, knobs on the right
        // Tested hardware: 514C:8850.
        (16, 3) => known(4, Top), // 4 × 4, knobs above
        _ => PadLayout { cols: fallback_cols(keys), knob_side: Top, known: false },
    }
}

/// Columns for an unknown pad: the most even rows that look like real macropads.
fn fallback_cols(keys: usize) -> usize {
    match keys {
        0 => 1,
        1..=5 => keys,  // one row
        6 | 9 => 3,     // 3 × 2, 3 × 3
        7 | 8 => 4,     // 4 + 3, 4 × 2
        10 => 5,        // 5 × 2
        11 | 12 => 4,   // 4 × 3
        13..=15 => 5,   // 5 × 3
        16 => 4,        // 4 × 4
        n => n.isqrt() + usize::from(n.isqrt() * n.isqrt() < n), // ~square
    }
}

/// Knobs per row in a right-hand cluster: one column, stacked top to bottom
/// (as in the vendor drawing of the 9-key + 3-knob pad); keeps wide pads on screen.
pub fn right_cluster_cols(_knobs: usize) -> usize {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_models() {
        assert_eq!(pad_layout(3, 1), PadLayout { cols: 3, knob_side: KnobSide::Right, known: true });
        assert_eq!(pad_layout(5, 0).cols, 5);
        assert_eq!(pad_layout(6, 2).knob_side, KnobSide::Top);
        assert_eq!(pad_layout(9, 3), PadLayout { cols: 3, knob_side: KnobSide::Right, known: true });
        assert_eq!(pad_layout(12, 2), PadLayout { cols: 4, knob_side: KnobSide::Right, known: true });
        assert_eq!(pad_layout(15, 3), PadLayout { cols: 5, knob_side: KnobSide::Right, known: true });
        assert_eq!(pad_layout(16, 3), PadLayout { cols: 4, knob_side: KnobSide::Top, known: true });
    }

    #[test]
    fn fallback_rows_are_even() {
        for (keys, cols) in [(2, 2), (4, 4), (5, 5), (8, 4), (10, 5), (11, 4), (14, 5), (20, 5), (25, 5), (30, 6)] {
            let l = pad_layout(keys, 1);
            assert_eq!(l.cols, cols, "{keys} keys");
            assert!(!l.known);
        }
    }

    #[test]
    fn cluster() {
        assert_eq!(right_cluster_cols(1), 1);
        assert_eq!(right_cluster_cols(2), 1);
        assert_eq!(right_cluster_cols(3), 1);
    }
}
