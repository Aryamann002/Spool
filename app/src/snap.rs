//! Alignment snapping for moving geometry.
//!
//! # Why this exists, and what it deliberately is not
//!
//! The research corpus is clear that snapping is the largest interaction gap in
//! the prototype, and equally clear about how it behaves: Figma snaps on by
//! default and `⌘`/`Ctrl` temporarily suspends it, tldraw keeps the threshold
//! in **screen pixels divided by zoom**, Affinity suspends with `⌥`, and every
//! product snaps a box's edges and centre lines against other boxes' edges and
//! centre lines.
//!
//! So this is the smallest subset that is recognisably the same gesture:
//!
//! - edges and centre lines only — no gap centring, no equal-spacing
//!   duplication, no vertex snapping, no persistent guides;
//! - a threshold in **screen pixels converted to world units by dividing by
//!   zoom**, so the feel is identical at 25% and at 400%;
//! - one snap per axis, the nearest candidate wins;
//! - a guide is returned for every snap, because a snap the user cannot see is
//!   indistinguishable from the object jumping for no reason.
//!
//! Everything here is a pure function over rectangles: no document, no camera,
//! no GPUI. That is what makes it testable without a window, and it is why the
//! caller decides *which* rectangles are candidates.
//!
//! # What a caller must supply
//!
//! - the bounds of everything being moved, as one rectangle — snapping a
//!   multi-selection as a unit is the convention, not snapping each object
//!   independently;
//! - the proposed world-space delta;
//! - the candidate rectangles, already excluding whatever is moving (an object
//!   is never a snap target for itself);
//! - the zoom, so the threshold is expressed in the screen pixels the user
//!   actually judges distance in.

/// One rectangle in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn left(self) -> f32 {
        self.x
    }

    pub fn center_x(self) -> f32 {
        self.x + self.width / 2.0
    }

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn top(self) -> f32 {
        self.y
    }

    pub fn center_y(self) -> f32 {
        self.y + self.height / 2.0
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    fn translated(self, (dx, dy): (f32, f32)) -> Self {
        Self::new(self.x + dx, self.y + dy, self.width, self.height)
    }
}

/// The smallest rectangle containing every input, if there is at least one.
pub fn bounds_of(rects: &[Rect]) -> Option<Rect> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for rect in rects {
        min_x = min_x.min(rect.left());
        min_y = min_y.min(rect.top());
        max_x = max_x.max(rect.right());
        max_y = max_y.max(rect.bottom());
    }
    if min_x > max_x {
        return None;
    }
    Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
}

/// Which line a guide is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// A vertical line at a world `x`: two objects share a left edge.
    Vertical,
    /// A horizontal line at a world `y`: two objects share a top edge.
    Horizontal,
}

/// A line to draw while a snap is holding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub axis: Axis,
    /// World coordinate of the line.
    pub at: f32,
    /// Extent along the other axis, spanning both boxes so the user can see
    /// *which* two things are being aligned rather than just where the line is.
    pub start: f32,
    pub end: f32,
}

/// Screen pixels a box may be from a candidate line before it snaps.
///
/// 8px is tldraw's documented default and roughly the distance a user can
/// resolve on a trackpad; Affinity's presets are in the same range.
pub const DEFAULT_THRESHOLD_PX: f32 = 8.0;

/// The threshold in world units for a given zoom.
///
/// The whole reason this conversion exists: a fixed world threshold would be
/// unreachable at 400% and unavoidable at 10%.
///
/// A camera that has never been sized has no meaningful scale, so it gets a
/// threshold of zero rather than an infinite one: nothing snaps, which is the
/// only safe answer when the user cannot yet see how far away anything is.
pub fn world_threshold(zoom: f32, threshold_px: f32) -> f32 {
    if zoom <= 0.0 {
        return 0.0;
    }
    threshold_px / zoom
}

/// The corrected delta and the guides that explain it.
///
/// An empty guide list means nothing snapped, and the returned delta is the one
/// that was passed in — so a caller can use the result unconditionally.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snap {
    /// The caller's delta plus whatever correction the snap added.
    pub delta: (f32, f32),
    pub guides: Vec<Guide>,
}

/// Snap one moving box against a set of candidates.
///
/// `bounds` is what is moving (already the union of a multi-selection), `delta`
/// is the proposed world-space translation, and `targets` must not include it.
///
/// Candidate lines per axis are every target's near edge, centre and far edge,
/// plus the document origin — Figma snaps to the page edge without needing a
/// page object to exist for it, and the document's 0,0 is Spool's closest
/// equivalent.
pub fn snap_translation(bounds: Rect, delta: (f32, f32), targets: &[Rect], zoom: f32) -> Snap {
    // Snapping corrects a *proposal*. A pointer that has not moved has proposed
    // nothing, so there is nothing to correct — and without this an object that
    // happens to sit four pixels off a neighbour's centre line would jump into
    // alignment the instant the user pressed the mouse, which is a move nobody
    // asked for and one that can create history for a click.
    if delta == (0.0, 0.0) {
        return Snap {
            delta,
            guides: Vec::new(),
        };
    }
    let threshold = world_threshold(zoom, DEFAULT_THRESHOLD_PX);
    let moved = bounds.translated(delta);
    // The origin is always the last candidate, and `candidates` indexes this
    // combined list so a guide can always find the rectangle it aligned to.
    let mut candidates = targets.to_vec();
    candidates.push(Rect::new(0.0, 0.0, 0.0, 0.0));
    // One line list per axis. Sharing a single list would compare vertical
    // positions against horizontal ones, which quietly produces corrections in
    // both axes for a purely horizontal drag.
    let vertical_lines: Vec<(f32, usize)> = candidates
        .iter()
        .enumerate()
        .flat_map(|(index, rect)| {
            [
                (rect.left(), index),
                (rect.center_x(), index),
                (rect.right(), index),
            ]
        })
        .collect();
    let horizontal_lines: Vec<(f32, usize)> = candidates
        .iter()
        .enumerate()
        .flat_map(|(index, rect)| {
            [
                (rect.top(), index),
                (rect.center_y(), index),
                (rect.bottom(), index),
            ]
        })
        .collect();
    let (dx, x_hit) = best_correction(
        [moved.left(), moved.center_x(), moved.right()],
        vertical_lines.into_iter(),
        threshold,
    );
    let (dy, y_hit) = best_correction(
        [moved.top(), moved.center_y(), moved.bottom()],
        horizontal_lines.into_iter(),
        threshold,
    );
    let mut guides = Vec::new();
    if let Some(hit) = x_hit {
        let other = candidates[hit.target];
        guides.push(Guide {
            axis: Axis::Vertical,
            at: hit.at,
            start: moved.top().min(other.top()),
            end: moved.bottom().max(other.bottom()),
        });
    }
    if let Some(hit) = y_hit {
        let other = candidates[hit.target];
        guides.push(Guide {
            axis: Axis::Horizontal,
            at: hit.at,
            start: moved.left().min(other.left()),
            end: moved.right().max(other.right()),
        });
    }
    Snap {
        delta: (delta.0 + dx, delta.1 + dy),
        guides,
    }
}

/// The line a snap matched, and the candidate it matched against.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SnapHit {
    at: f32,
    target: usize,
}

/// The smallest correction that puts one of `moving` onto one of `candidates`.
///
/// A correction of exactly zero is *not* a snap. It happens whenever the box is
/// already in line, and treating it as one would make a keyboard nudge of an
/// aligned object a no-op: the box would be pulled straight back to where it
/// was. Alignment that the user did not move towards is not a snap, and the
/// guide for it is noise.
fn best_correction(
    moving: [f32; 3],
    candidates: impl Iterator<Item = (f32, usize)>,
    threshold: f32,
) -> (f32, Option<SnapHit>) {
    let mut best: Option<(f32, f32, SnapHit)> = None;
    for (candidate, target) in candidates {
        for moving_line in moving {
            let correction = candidate - moving_line;
            let distance = correction.abs();
            if distance == 0.0 || distance > threshold {
                continue;
            }
            if best.is_none_or(|(best_distance, ..)| distance < best_distance) {
                best = Some((
                    distance,
                    correction,
                    SnapHit {
                        at: candidate,
                        target,
                    },
                ));
            }
        }
    }
    match best {
        Some((_, correction, hit)) => (correction, Some(hit)),
        None => (0.0, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_near_another_boxs_left_edge_snaps_to_it() {
        // The Figma gesture: drag until the left edge is almost level, and it
        // clicks into line. The box starts already in the right place and the
        // drag is the delta, which is how a caller actually calls this.
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(0.0, 200.0, 60.0, 20.0);
        let snap = snap_translation(start, (96.0, 0.0), &[target], 1.0);
        assert!((snap.delta.0 - 100.0).abs() < 0.001, "{:?}", snap.delta);
        assert_eq!(snap.delta.1, 0.0, "only the axis that snapped moves");
        assert_eq!(snap.guides.len(), 1, "the snap says why: {:?}", snap.guides);
        let guide = snap.guides[0];
        assert_eq!(guide.axis, Axis::Vertical);
        assert!((guide.at - 100.0).abs() < 0.001);
    }

    #[test]
    fn a_guide_spans_both_boxes_so_the_user_sees_what_aligned() {
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(0.0, 200.0, 60.0, 20.0);
        let snap = snap_translation(start, (96.0, 0.0), &[target], 1.0);
        let guide = snap.guides[0];
        assert!((guide.start - 40.0).abs() < 0.001, "{:?}", guide);
        assert!((guide.end - 220.0).abs() < 0.001, "{:?}", guide);
    }

    #[test]
    fn a_box_beyond_the_threshold_does_not_snap() {
        // Nothing within 8px of any of the target's three lines, and nothing
        // near the origin either.
        let target = Rect::new(300.0, 40.0, 80.0, 30.0);
        let start = Rect::new(0.0, 200.0, 60.0, 20.0);
        let snap = snap_translation(start, (96.0, 0.0), &[target], 1.0);
        assert_eq!(snap.delta, (96.0, 0.0), "the drag is untouched");
        assert!(snap.guides.is_empty(), "no guide, no silent movement");
    }

    #[test]
    fn the_threshold_is_screen_pixels_so_the_feel_is_the_same_at_every_zoom() {
        // 20 world units apart. At 100% that is 20 screen px — past an 8px
        // threshold. At 20% the very same gap is 4 screen px, which is inside
        // it. A fixed *world* threshold cannot produce that difference, which is
        // the entire reason the zoom is an input.
        let target = Rect::new(0.0, 0.0, 40.0, 40.0);
        let start = Rect::new(0.0, 200.0, 40.0, 40.0);
        assert!(
            snap_translation(start, (60.0, 0.0), &[target], 1.0)
                .guides
                .is_empty(),
            "20 screen px away at 100%"
        );
        assert!(
            snap_translation(start, (60.0, 0.0), &[target], 0.2)
                .guides
                .len()
                == 1,
            "4 screen px away at 20%"
        );
    }

    #[test]
    fn centres_align_as_well_as_edges() {
        let target = Rect::new(120.0, 0.0, 200.0, 20.0);
        let start = Rect::new(0.0, 100.0, 40.0, 20.0);
        // Centre 200 on the left, 220 on the right: 20 out, so nothing snaps yet.
        assert!(
            snap_translation(start, (180.0, 0.0), &[target], 1.0)
                .guides
                .is_empty(),
            "centres 20 apart"
        );
        let snap = snap_translation(start, (204.0, 0.0), &[target], 1.0);
        assert!(
            (snap.delta.0 - 200.0).abs() < 0.001,
            "centre line to centre line: {:?}",
            snap.delta
        );
    }

    #[test]
    fn both_axes_can_snap_in_one_drag() {
        let target = Rect::new(0.0, 0.0, 200.0, 100.0);
        let start = Rect::new(0.0, 0.0, 40.0, 20.0);
        let snap = snap_translation(start, (3.0, 7.0), &[target], 1.0);
        assert_eq!(
            (snap.delta.0, snap.delta.1),
            (0.0, 0.0),
            "both edges pulled into line, so the pointer's own delta survives: {:?}",
            snap.delta
        );
        assert_eq!(snap.guides.len(), 2, "{:?}", snap.guides);
        assert!(snap
            .guides
            .iter()
            .any(|guide| guide.axis == Axis::Horizontal));
        assert!(snap.guides.iter().any(|guide| guide.axis == Axis::Vertical));
    }

    #[test]
    fn a_multi_selection_snaps_as_one_unit() {
        // The convention that matters: two objects move by one correction, taken
        // from the union of their bounds. The caller applies the single returned
        // delta to both, so a group can never be pulled apart by snapping.
        let target = Rect::new(0.0, 0.0, 100.0, 20.0);
        let a = Rect::new(100.0, 100.0, 40.0, 20.0);
        let b = Rect::new(160.0, 140.0, 40.0, 20.0);
        let union = bounds_of(&[a, b]).expect("two rectangles have bounds");
        assert_eq!(union.left(), 100.0);
        assert_eq!(union.right(), 200.0);
        assert_eq!(union.center_x(), 150.0);
        // Drag so the union's centre sits 6 world px from the target's centre.
        let snap = snap_translation(union, (-56.0, 0.0), &[target], 1.0);
        assert!(
            (snap.delta.0 - -50.0).abs() < 0.001,
            "one correction for the whole group: {:?}",
            snap.delta
        );
    }

    #[test]
    fn the_page_origin_is_always_a_candidate() {
        // Figma snaps to the page edge without a page object existing for it.
        let start = Rect::new(0.0, 300.0, 40.0, 20.0);
        let snap = snap_translation(start, (5.0, 0.0), &[], 1.0);
        assert!((snap.delta.0 - 0.0).abs() < 0.001, "{:?}", snap.delta);
        assert_eq!(snap.guides.len(), 1, "the origin still explains itself");
    }

    #[test]
    fn an_alignment_the_user_never_moved_towards_is_not_a_snap() {
        // The box already shares an edge before the pointer has moved. There is
        // no correction to apply, so claiming a snap would mean a guide drawn
        // for an alignment the user did not create.
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(100.0, 200.0, 60.0, 20.0);
        let snap = snap_translation(start, (0.0, 0.0), &[target], 1.0);
        assert_eq!(snap.delta, (0.0, 0.0));
        assert!(snap.guides.is_empty());
    }

    #[test]
    fn arriving_exactly_on_a_line_is_not_a_correction() {
        // The one case the zero-distance guard actually decides. The pointer has
        // moved — one pixel left, which is why this reaches `best_correction` at
        // all — and the box lands exactly on the target's left edge. There is no
        // correction to apply and no line the user is being pulled towards, so
        // claiming a snap here would draw a guide for a drag that already did
        // what was asked.
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(101.0, 200.0, 60.0, 20.0);
        let snap = snap_translation(start, (-1.0, 0.0), &[target], 1.0);
        assert_eq!(
            snap.delta,
            (-1.0, 0.0),
            "the drag is untouched: it landed where the pointer asked"
        );
        assert!(snap.guides.is_empty(), "no correction, no guide");
    }

    #[test]
    fn a_drag_that_tries_to_leave_an_alignment_is_held_by_it() {
        // Why a keyboard nudge must not go through here at all. Dragging one
        // pixel off a shared edge is within the threshold, so the box is held
        // in line — which is the documented Figma behaviour, and the reason
        // `⌘` exists to suspend snapping. Reached from an arrow key it would
        // mean the key does nothing.
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(100.0, 200.0, 60.0, 20.0);
        let snap = snap_translation(start, (1.0, 0.0), &[target], 1.0);
        assert_eq!(snap.delta, (0.0, 0.0), "held in line");
        assert_eq!(snap.guides.len(), 1, "and it says so");
    }

    #[test]
    fn a_pointer_that_has_not_moved_proposes_nothing_so_nothing_snaps() {
        // The object sits four world pixels off the target's centre line, which
        // is well inside the threshold. Pressing the pointer must still not
        // move it: alignment the user did not ask for is not a snap, and a
        // click that silently repositions an object would also create history
        // for a gesture that never happened.
        let target = Rect::new(0.0, 0.0, 100.0, 100.0);
        let start = Rect::new(54.0, 300.0, 40.0, 20.0);
        let snap = snap_translation(start, (0.0, 0.0), &[target], 1.0);
        assert_eq!(snap.delta, (0.0, 0.0));
        assert!(snap.guides.is_empty());
    }

    #[test]
    fn one_pixel_of_drag_is_enough_to_be_held_by_a_nearby_alignment() {
        // The counterpart to the test above, and the exact reason a keyboard
        // nudge must not come through here. The box starts aligned with the
        // target's centre line; one pixel of drag is inside the threshold, so
        // the magnet puts it straight back. Reached from an arrow key the key
        // press would do nothing at all.
        let target = Rect::new(0.0, 0.0, 100.0, 100.0);
        let start = Rect::new(50.0, 300.0, 40.0, 20.0);
        let snap = snap_translation(start, (1.0, 0.0), &[target], 1.0);
        assert_eq!(snap.delta, (0.0, 0.0), "held in line");
        assert_eq!(snap.guides.len(), 1, "and it says why");
        assert_eq!(snap.guides[0].at, 50.0);
    }

    #[test]
    fn bounds_of_nothing_is_none_rather_than_a_box_around_the_origin() {
        assert_eq!(bounds_of(&[]), None);
    }

    #[test]
    fn bounds_of_a_multi_selection_is_one_rectangle() {
        let union = bounds_of(&[
            Rect::new(10.0, 10.0, 20.0, 20.0),
            Rect::new(-5.0, 40.0, 5.0, 5.0),
        ])
        .expect("two rectangles have bounds");
        assert_eq!(union, Rect::new(-5.0, 10.0, 35.0, 35.0));
    }

    #[test]
    fn an_absurd_zoom_leaves_snapping_off_rather_than_snapping_to_everything() {
        let target = Rect::new(0.0, 0.0, 40.0, 40.0);
        let start = Rect::new(0.0, 200.0, 40.0, 40.0);
        assert!(snap_translation(start, (60.0, 0.0), &[target], 0.0)
            .guides
            .is_empty());
    }
}
