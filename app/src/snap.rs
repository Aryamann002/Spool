//! Alignment snapping for moving and resizing geometry.
//!
//! # Why this exists, and what it deliberately is not
//!
//! The research corpus is clear that snapping is the largest interaction gap in
//! the prototype, and equally clear about how it behaves: Figma snaps on by
//! default and `⌘`/`Ctrl` temporarily suspends it, tldraw keeps the threshold
//! in **screen pixels divided by zoom**, Affinity suspends with `⌥`, and every
//! product snaps a box's edges and centre lines against other boxes' edges and
//! centre lines. All four also snap the edges a resize handle moves.
//!
//! So this is the smallest subset that is recognisably the same gesture:
//!
//! - edges and centre lines only — no gap centring, no equal-spacing
//!   duplication, no vertex snapping, no persistent guides;
//! - a threshold in **screen pixels converted to world units by dividing by
//!   zoom**, so the feel is identical at 25% and at 400%;
//! - at most one snap per axis;
//! - a guide for every snap, because a snap the user cannot see is
//!   indistinguishable from the object jumping for no reason.
//!
//! Everything here is a pure function over rectangles: no document, no camera,
//! no GPUI. That is what makes it testable without a window, and it is why the
//! caller decides *which* rectangles are candidates.
//!
//! # What a caller must supply
//!
//! For a move, the bounds of everything being moved as one rectangle — snapping
//! a multi-selection as a unit is the convention, not snapping each object
//! independently — the proposed world-space delta, the candidate rectangles
//! (already excluding whatever is moving, since an object is never a snap
//! target for itself), the zoom, and any axis the gesture has been constrained
//! away from. For a resize, the box as it was, the proposed delta, which edges
//! the handle moves, and the smallest size a box may have.
//!
//! # Determinism
//!
//! The rule that decides *which* candidate wins is written out in one place —
//! [`Hit`]'s ordering — rather than emerging from whichever order the engine
//! happened to visit candidates in. Two objects at the same distance must not
//! resolve differently because one of them happens to come first in the
//! document, and a caller that builds its candidate list from a hash map would
//! otherwise get a different answer on every run. The only tie-break left that
//! looks at the caller's list is the last one, and it is reached only when two
//! candidates offer the *identical* line, which is the one case where the two
//! are indistinguishable on screen anyway.
//!
//! The order is: nearest line wins, because a magnet that pulls further than
//! the nearer thing it is nearer to feels broken; then an edge against an edge
//! over a centre against a centre, because sharing an edge is what a designer
//! means far more often than sharing a centre; then the lower coordinate, which
//! settles the two-lines-on-opposite-sides case by geometry rather than by
//! document order; then the moving box's own outer line before its centre; then
//! the caller's order.

use std::cmp::Ordering;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis {
    /// A vertical line at a world `x`: two objects share a left edge.
    Vertical,
    /// A horizontal line at a world `y`: two objects share a top edge.
    Horizontal,
}

impl Axis {
    /// This axis's three lines for a box: the near edge, the centre, the far edge.
    fn lines(self, rect: Rect) -> [f32; 3] {
        match self {
            Self::Vertical => [rect.left(), rect.center_x(), rect.right()],
            Self::Horizontal => [rect.top(), rect.center_y(), rect.bottom()],
        }
    }

    /// Does the box have no extent along this axis?
    fn is_flat(self, rect: Rect) -> bool {
        match self {
            Self::Vertical => rect.width <= 0.0,
            Self::Horizontal => rect.height <= 0.0,
        }
    }

    /// The box's extent on the axis a guide for *this* axis is drawn across.
    fn across(self, rect: Rect) -> (f32, f32) {
        match self {
            Self::Vertical => (rect.top(), rect.bottom()),
            Self::Horizontal => (rect.left(), rect.right()),
        }
    }

    fn component(self, delta: (f32, f32)) -> f32 {
        match self {
            Self::Vertical => delta.0,
            Self::Horizontal => delta.1,
        }
    }

    /// The same delta with one axis replaced.
    fn with(self, delta: (f32, f32), value: f32) -> (f32, f32) {
        match self {
            Self::Vertical => (value, delta.1),
            Self::Horizontal => (delta.0, value),
        }
    }

    /// Is this the line that lines up `x`?
    ///
    /// A *vertical* line is the one that lines `x` up — the opposite of what its
    /// name suggests. That inversion is asked here rather than spelled out at
    /// every call site because pairing a movement axis with a line axis is
    /// invisible until a constrained drag moves the wrong way.
    fn lines_x(self) -> bool {
        matches!(self, Self::Vertical)
    }
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

/// An axis a gesture has been constrained away from.
///
/// `⇧` constrains a drag to one axis, and snapping has to be told, because a
/// snap is a correction on an axis the pointer is not otherwise moving: without
/// this, constraining a drag to X would still let the magnet pull the box
/// vertically, which is the one thing the constraint promised would not happen.
///
/// A zero delta component is *not* a lock. Dragging purely sideways across a
/// neighbour's centre line should still click into line vertically — the guide
/// is what makes that legible — so the lock has to be stated, not inferred.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AxisLock {
    #[default]
    Free,
    /// The gesture may move horizontally only.
    X,
    /// The gesture may move vertically only.
    Y,
}

impl AxisLock {
    /// May a snap correct this axis?
    ///
    /// `X` means the gesture may move on `x`, so the axis that lines `x` up —
    /// [`Axis::Vertical`] — is the one still allowed to snap.
    fn allows(self, axis: Axis) -> bool {
        match self {
            Self::Free => true,
            Self::X => axis.lines_x(),
            Self::Y => !axis.lines_x(),
        }
    }
}

/// Which of a box's three lines on one axis something is.
///
/// Declared outermost to innermost so that a `[f32; 3]` of a box's lines indexes
/// straight into [`Side::ALL`], and so that "the outer edge beats the centre" is
/// a consequence of the declaration order rather than a comparison somebody has
/// to remember.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Side {
    Start,
    Middle,
    End,
}

impl Side {
    const ALL: [Self; 3] = [Self::Start, Self::Middle, Self::End];
}

/// What sort of alignment two lines form.
///
/// The order is the tie-break, and it is deliberately `Edge` before `Center`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AlignKind {
    /// An edge of one box on an edge of another.
    Edge,
    /// An edge on a centre, or a centre on an edge.
    Mixed,
    /// A centre line on a centre line.
    Center,
}

impl AlignKind {
    fn of(moving: Side, candidate: Side) -> Self {
        match (moving, candidate) {
            (Side::Middle, Side::Middle) => Self::Center,
            (Side::Middle, _) | (_, Side::Middle) => Self::Mixed,
            _ => Self::Edge,
        }
    }
}

/// A line a snap can land on, and where it came from.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    /// The world coordinate of the line.
    at: f32,
    /// Which line of its rectangle this is.
    side: Side,
    /// Index into the rectangle list, so a guide can report the box it aligned
    /// to without the caller having to describe its own geometry twice.
    source: usize,
}

/// The candidate lines for one axis: every target's near edge, centre and far
/// edge, plus the page origin.
///
/// A rectangle with no extent along this axis has one line where its edges and
/// its centre all coincide — the page origin, or a zero-sized object — so it
/// offers that line once rather than three times. Offering it three times would
/// make the winner depend on which of three identical candidates was visited
/// first, which is exactly the accident the ordering below exists to remove.
fn candidate_lines(rects: &[Rect], axis: Axis) -> Vec<Candidate> {
    let mut lines = Vec::with_capacity(rects.len() * 3);
    for (source, rect) in rects.iter().enumerate() {
        let coordinates = axis.lines(*rect);
        let flat = axis.is_flat(*rect);
        for (index, side) in Side::ALL.into_iter().enumerate() {
            if flat && side != Side::Start {
                continue;
            }
            lines.push(Candidate {
                at: coordinates[index],
                side,
                source,
            });
        }
    }
    lines
}

/// The candidate rectangles, with the page origin appended.
///
/// Figma snaps to the page edge without needing a page object to exist for it,
/// and the document's 0,0 is Spool's closest equivalent. It goes last so that a
/// real object's edge, being the same distance and the same kind of alignment,
/// wins the tie-break above it.
fn candidates_with_origin(targets: &[Rect]) -> Vec<Rect> {
    let mut rects = targets.to_vec();
    rects.push(Rect::new(0.0, 0.0, 0.0, 0.0));
    rects
}

/// The line a snap matched, and the candidate it matched against.
#[derive(Clone, Copy, Debug)]
struct Hit {
    /// The candidate line's world coordinate.
    at: f32,
    /// What to add to the proposed delta.
    correction: f32,
    /// How far the correction is, which is what the threshold is compared
    /// against and what the winner is chosen by.
    distance: f32,
    /// Which line of the moving box is being pulled.
    moving: Side,
    kind: AlignKind,
    /// Index of the rectangle the candidate came from.
    source: usize,
}

/// The complete, documented tie-break.
///
/// See the module docs for the reasoning. Every field is compared, so this is a
/// total order and the winner does not depend on the order candidates were
/// generated in — with one deliberate exception: two candidates offering the
/// identical line, which is resolved by the caller's order and which the user
/// cannot tell apart anyway.
impl Ord for Hit {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .total_cmp(&other.distance)
            // Sharing an edge is what a designer means far more often than
            // sharing a centre, so at equal distance an edge wins.
            .then_with(|| self.kind.cmp(&other.kind))
            // Two lines on opposite sides of the moving box are equally close
            // and cannot both be satisfied. The lower coordinate wins, so the
            // answer comes from the geometry rather than from document order.
            .then_with(|| self.at.total_cmp(&other.at))
            .then_with(|| self.moving.cmp(&other.moving))
            .then_with(|| self.source.cmp(&other.source))
    }
}

impl PartialOrd for Hit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Equality *is* the order, so the two can never disagree about a hit.
///
/// Written out rather than derived, because a derived `PartialEq` would compare
/// `distance` with `==` — and `0.0 == -0.0` is true while `total_cmp` calls
/// them different. `Ord` promises that equal means equal order, and this keeps
/// that promise without giving up the total order the tie-break needs.
impl PartialEq for Hit {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Hit {}

/// The snap chosen on one axis, plus the line that explains it.
struct AxisSnap {
    /// What to add to the proposed delta on this axis.
    correction: f32,
    /// The line that was landed on.
    at: f32,
    /// Extent of the guide along the other axis.
    start: f32,
    end: f32,
}

/// Choose one axis's snap.
///
/// `moving` is the box as the pointer has proposed it, which is what the guide
/// spans; `lines` are the lines on it that may snap — three of them for a move,
/// one for a resize handle, because a handle moves a single edge.
fn best_on_axis(
    axis: Axis,
    moving: Rect,
    lines: &[(f32, Side)],
    candidates: &[Candidate],
    rects: &[Rect],
    threshold: f32,
) -> Option<AxisSnap> {
    let mut best: Option<Hit> = None;
    for candidate in candidates {
        for (from, side) in lines {
            let correction = candidate.at - from;
            let distance = correction.abs();
            // A correction of exactly zero is *not* a snap. It happens whenever
            // the box is already in line, and treating it as one would make a
            // keyboard nudge of an aligned object a no-op: the box would be
            // pulled straight back to where it was. Alignment that the user did
            // not move towards is not a snap, and the guide for it is noise.
            if distance == 0.0 || distance > threshold {
                continue;
            }
            let hit = Hit {
                at: candidate.at,
                correction,
                distance,
                moving: *side,
                kind: AlignKind::of(*side, candidate.side),
                source: candidate.source,
            };
            if best.is_none_or(|current| hit < current) {
                best = Some(hit);
            }
        }
    }
    let best = best?;
    let (start, end) = axis.across(moving);
    let mut span = (start, end);
    // Every rectangle offering the line that was landed on extends the guide,
    // so three objects sharing an edge draw one line through all three rather
    // than three lines on top of each other.
    //
    // Any candidate at that coordinate is a hit with the same correction and the
    // same distance — the winner came from this very list — so this cannot pull
    // in a line the object did not reach.
    for candidate in candidates {
        if candidate.at != best.at {
            continue;
        }
        let (near, far) = axis.across(rects[candidate.source]);
        span.0 = span.0.min(near);
        span.1 = span.1.max(far);
    }
    Some(AxisSnap {
        correction: best.correction,
        at: best.at,
        start: span.0,
        end: span.1,
    })
}

/// The three lines of `moving`, labelled outermost first.
fn lines_of(moving: Rect, axis: Axis) -> [(f32, Side); 3] {
    let coordinates = axis.lines(moving);
    [
        (coordinates[0], Side::Start),
        (coordinates[1], Side::Middle),
        (coordinates[2], Side::End),
    ]
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
/// `lock` is any axis the gesture has been constrained away from.
///
/// Guides come back in a fixed order — vertical before horizontal — so a
/// caller that renders them or compares them in a test sees the same thing
/// every time.
pub fn snap_translation(
    bounds: Rect,
    delta: (f32, f32),
    targets: &[Rect],
    zoom: f32,
    lock: AxisLock,
) -> Snap {
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
    let rects = candidates_with_origin(targets);
    let threshold = world_threshold(zoom, DEFAULT_THRESHOLD_PX);
    let moved = bounds.translated(delta);
    let mut corrected = delta;
    let mut guides = Vec::new();
    for axis in [Axis::Vertical, Axis::Horizontal] {
        if !lock.allows(axis) {
            continue;
        }
        let candidates = candidate_lines(&rects, axis);
        let Some(snap) = best_on_axis(
            axis,
            moved,
            &lines_of(moved, axis),
            &candidates,
            &rects,
            threshold,
        ) else {
            continue;
        };
        corrected = axis.with(corrected, axis.component(delta) + snap.correction);
        guides.push(Guide {
            axis,
            at: snap.at,
            start: snap.start,
            end: snap.end,
        });
    }
    Snap {
        delta: corrected,
        guides,
    }
}

/// Which edges of a resized box the pointer is moving.
///
/// A handle moves at most one edge per axis — `Left` moves the left edge and
/// leaves the right one where it is — and that is the whole reason snapping a
/// resize needs no constraint solver. Each moving edge is tested against the
/// candidate lines on its own axis, and the two axes are independent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MovedEdges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl MovedEdges {
    fn on(self, axis: Axis) -> (bool, bool) {
        match axis {
            Axis::Vertical => (self.left, self.right),
            Axis::Horizontal => (self.top, self.bottom),
        }
    }
}

/// Snap the edges a resize handle is moving against a set of candidates.
///
/// `start` is the box as it was, `delta` the proposed world-space translation of
/// the pointer, and `moved` which edges that translation is allowed to change.
/// The result is the corrected delta, which the caller feeds back through its
/// own resize arithmetic so the size clamps stay in one place.
///
/// Deliberately not modelled: proportional resize, where the pointer's delta is
/// converted into a scale before any edge has a position to align. A caller
/// scales first, so there is no single edge whose correction survives the scale,
/// and guessing would mean re-deriving the scale factor here. Callers that scale
/// ask for no snap, and say so.
///
/// A correction that would push an edge past the opposite one by less than
/// `min_size` is refused rather than applied, so the caller never ends up with a
/// guide claiming an alignment its own minimum-size clamp then threw away.
pub fn snap_resize(
    start: Rect,
    delta: (f32, f32),
    moved: MovedEdges,
    targets: &[Rect],
    zoom: f32,
    min_size: f32,
) -> Snap {
    // The same rule as a move: a pointer that has not moved has proposed
    // nothing, so there is nothing to correct.
    if delta == (0.0, 0.0) {
        return Snap {
            delta,
            guides: Vec::new(),
        };
    }
    let rects = candidates_with_origin(targets);
    let threshold = world_threshold(zoom, DEFAULT_THRESHOLD_PX);
    let proposed = start.translated(delta);
    let mut corrected = delta;
    let mut guides = Vec::new();
    for axis in [Axis::Vertical, Axis::Horizontal] {
        let (moving_start, moving_end) = moved.on(axis);
        // Neither edge moving means there is nothing on this axis to align.
        // Both edges moving is a stretch, and is declined rather than guessed at.
        if moving_start == moving_end {
            continue;
        }
        let side = if moving_start { Side::Start } else { Side::End };
        let moving_at = if moving_start {
            axis.lines(proposed)[0]
        } else {
            axis.lines(proposed)[2]
        };
        let opposite_at = if moving_start {
            axis.lines(start)[2]
        } else {
            axis.lines(start)[0]
        };
        let candidates = candidate_lines(&rects, axis);
        let Some(snap) = best_on_axis(
            axis,
            proposed,
            &[(moving_at, side)],
            &candidates,
            &rects,
            threshold,
        ) else {
            continue;
        };
        let fits = if moving_start {
            snap.at <= opposite_at - min_size
        } else {
            snap.at >= opposite_at + min_size
        };
        if !fits {
            continue;
        }
        corrected = axis.with(corrected, axis.component(delta) + snap.correction);
        guides.push(Guide {
            axis,
            at: snap.at,
            start: snap.start,
            end: snap.end,
        });
    }
    Snap {
        delta: corrected,
        guides,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The free-axis form, for the cases that are not about the lock.
    fn snap_free(bounds: Rect, delta: (f32, f32), targets: &[Rect], zoom: f32) -> Snap {
        snap_translation(bounds, delta, targets, zoom, AxisLock::Free)
    }

    #[test]
    fn a_box_near_another_boxs_left_edge_snaps_to_it() {
        // The Figma gesture: drag until the left edge is almost level, and it
        // clicks into line. The box starts already in the right place and the
        // drag is the delta, which is how a caller actually calls this.
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(0.0, 200.0, 60.0, 20.0);
        let snap = snap_free(start, (96.0, 0.0), &[target], 1.0);
        assert!((snap.delta.0 - 100.0).abs() < 0.001, "{:?}", snap.delta);
        assert_eq!(snap.delta.1, 0.0, "only the axis that snapped moves");
        assert_eq!(snap.guides.len(), 1, "the snap says why: {:?}", snap.guides);
        let guide = snap.guides[0];
        assert_eq!(guide.axis, Axis::Vertical);
        assert!((guide.at - 100.0).abs() < 0.001);
    }

    #[test]
    fn a_right_edge_snaps_to_a_neighbours_left_edge_from_either_direction() {
        // The other half of edge snapping, and the one a designer uses most:
        // butting two boxes up against each other. Aligning right-to-right and
        // left-to-right are the same rule read the other way round.
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(0.0, 200.0, 60.0, 20.0);
        // Right edge lands at 184.3 against the target's right edge at 180.
        let snap = snap_free(start, (124.3, 0.0), &[target], 1.0);
        assert!(
            (start.right() + snap.delta.0 - 180.0).abs() < 0.001,
            "right edges in line: {:?}",
            snap.delta
        );
        assert_eq!(snap.guides[0].at, 180.0);
    }

    #[test]
    fn a_guide_spans_both_boxes_so_the_user_sees_what_aligned() {
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(0.0, 200.0, 60.0, 20.0);
        let snap = snap_free(start, (96.0, 0.0), &[target], 1.0);
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
        let snap = snap_free(start, (96.0, 0.0), &[target], 1.0);
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
            snap_free(start, (60.0, 0.0), &[target], 1.0)
                .guides
                .is_empty(),
            "20 screen px away at 100%"
        );
        assert_eq!(
            snap_free(start, (60.0, 0.0), &[target], 0.2).guides.len(),
            1,
            "4 screen px away at 20%"
        );
    }

    #[test]
    fn the_threshold_is_measured_in_screen_pixels_at_every_zoom() {
        // The same screen-space gap, held constant while the zoom changes. If
        // the threshold were world-space these four would disagree: the gap
        // inside the magnet at one zoom and outside it at the next.
        //
        // The two boxes are deliberately different widths, so the moving box's
        // three lines cannot land on the target's three lines all at once. Same
        // width would make every line a tie and the test would measure the
        // tie-break instead of the threshold.
        let target = Rect::new(0.0, 0.0, 10.0, 10.0);
        let start = Rect::new(0.0, 500.0, 30.0, 10.0);
        for zoom in [0.25, 0.5, 1.0, 2.0, 4.0] {
            let inside = 6.0 / zoom;
            let snap = snap_free(start, (10.0 + inside, 0.0), &[target], zoom);
            assert_eq!(
                snap.guides.len(),
                1,
                "6 screen px must snap at {zoom}x (gap {inside})"
            );
            assert_eq!(snap.guides[0].at, 10.0, "the target's left edge");

            let outside = (DEFAULT_THRESHOLD_PX + 0.5) / zoom;
            assert!(
                snap_free(start, (10.0 + outside, 0.0), &[target], zoom)
                    .guides
                    .is_empty(),
                "8.5 screen px must not snap at {zoom}x (gap {outside})"
            );
        }
    }

    #[test]
    fn centres_align_as_well_as_edges() {
        let target = Rect::new(120.0, 0.0, 200.0, 20.0);
        let start = Rect::new(0.0, 100.0, 40.0, 20.0);
        // Centre 200 on the left, 220 on the right: 20 out, so nothing snaps yet.
        assert!(
            snap_free(start, (180.0, 0.0), &[target], 1.0)
                .guides
                .is_empty(),
            "centres 20 apart"
        );
        let snap = snap_free(start, (204.0, 0.0), &[target], 1.0);
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
        let snap = snap_free(start, (3.0, 7.0), &[target], 1.0);
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
    fn the_axes_are_considered_independently() {
        // Each axis has its own candidate lines and its own nearest line. A
        // shared list would compare vertical positions against horizontal ones
        // and produce corrections in both axes for a purely horizontal drag.
        //
        // The moving box is 60 tall so that only *one* of its three horizontal
        // lines is within reach of the target's; with a 20-tall box all three
        // sit near a target line and the result measures the tie-break instead.
        let target = Rect::new(0.0, 0.0, 200.0, 40.0);
        let start = Rect::new(0.0, 200.0, 40.0, 60.0);
        // Moved to left 3 and top 23: 3 from the target's left edge, and 3 from
        // its horizontal centre at 20. Nothing else is in range on either axis.
        let snap = snap_free(start, (3.0, -177.0), &[target], 1.0);
        assert_eq!(snap.delta, (0.0, -180.0), "{:?}", snap.delta);
        assert_eq!(snap.guides.len(), 2, "{:?}", snap.guides);
        assert_eq!(snap.guides[0].axis, Axis::Vertical);
        assert_eq!(snap.guides[0].at, 0.0, "the target's left edge");
        assert_eq!(snap.guides[1].axis, Axis::Horizontal);
        assert_eq!(snap.guides[1].at, 20.0, "the target's vertical centre");
    }

    #[test]
    fn the_nearest_of_several_candidates_wins() {
        // Three objects, all in range, all with an edge the moving box could
        // reach. The nearest one is the answer, whatever order they arrived in.
        let near = Rect::new(100.0, 40.0, 80.0, 30.0);
        let middle = Rect::new(104.0, 200.0, 80.0, 30.0);
        let far = Rect::new(120.0, 400.0, 80.0, 30.0);
        let start = Rect::new(0.0, 600.0, 60.0, 20.0);
        for targets in [
            vec![near, middle, far],
            vec![far, middle, near],
            vec![middle, far, near],
        ] {
            let snap = snap_free(start, (98.0, 0.0), &targets, 1.0);
            assert!(
                (snap.delta.0 - 100.0).abs() < 0.001,
                "nearest wins regardless of order: {:?}",
                snap.delta
            );
            assert_eq!(snap.guides[0].at, 100.0);
        }
    }

    #[test]
    fn an_equally_close_edge_beats_an_equally_close_centre() {
        // The moving box's left edge is 4 from one target's left edge and its
        // centre is 4 from the same target's right edge. Both are exactly as
        // close, so distance alone cannot decide; sharing an edge is the more
        // common intent and wins.
        //
        // The moving box is 384 wide so that its left edge and its centre can
        // each land 4 from a *different* edge of a 200-wide target. Any other
        // width puts one of the three lines somewhere that changes the answer.
        let target = Rect::new(100.0, 40.0, 200.0, 30.0);
        let start = Rect::new(96.0, 200.0, 384.0, 20.0);
        let snap = snap_free(start, (8.0, 0.0), &[target], 1.0);
        // Moved: left 104, centre 296, right 488. Left is 4 from the target's
        // left edge at 100; centre is 4 from its right edge at 300.
        assert_eq!(
            snap.guides[0].at, 100.0,
            "the shared edge: {:?}",
            snap.guides
        );
        assert!(
            (start.left() + snap.delta.0 - 100.0).abs() < 0.001,
            "pulled onto the shared edge: {:?}",
            snap.delta
        );
    }

    #[test]
    fn two_candidates_at_the_same_distance_resolve_the_same_way_every_time() {
        // The tie-break that used not to exist: two objects equally far away,
        // and the answer was whichever one the engine visited first — which is
        // document order, and therefore changes when the document does. Now it
        // is the lower coordinate, and only the lower coordinate, whatever
        // order the two arrive in.
        let before = Rect::new(60.0, 40.0, 40.0, 30.0);
        let after = Rect::new(108.0, 200.0, 40.0, 30.0);
        let start = Rect::new(96.0, 400.0, 40.0, 20.0);
        // Moved left edge 104: 4 from the first box's right edge at 100, and 4
        // from the second box's left edge at 108. Two hits, one distance, two
        // different corrections — and no third line within reach either.
        let snap_forward = snap_free(start, (8.0, 0.0), &[before, after], 1.0);
        let snap_reverse = snap_free(start, (8.0, 0.0), &[after, before], 1.0);
        assert_eq!(snap_forward.guides.len(), 1, "{:?}", snap_forward.guides);
        assert_eq!(snap_reverse.guides.len(), 1, "{:?}", snap_reverse.guides);
        assert_eq!(
            snap_forward.guides[0].at, 100.0,
            "the lower of two equidistant lines"
        );
        assert_eq!(
            snap_reverse.guides[0], snap_forward.guides[0],
            "and the same answer whatever order they arrive in"
        );
    }

    #[test]
    fn every_object_on_the_line_it_snapped_to_gets_a_guide() {
        // Three objects whose left edges all sit at 100. tldraw documents that
        // collinear snap points "appear as one continuous line", so the answer
        // is one guide spanning all three rather than three guides on top of
        // each other.
        let a = Rect::new(100.0, 40.0, 40.0, 30.0);
        let b = Rect::new(100.0, 200.0, 40.0, 30.0);
        let c = Rect::new(100.0, 400.0, 40.0, 30.0);
        let start = Rect::new(0.0, 600.0, 60.0, 20.0);
        let snap = snap_free(start, (97.0, 0.0), &[a, b, c], 1.0);
        assert_eq!(snap.guides.len(), 1, "{:?}", snap.guides);
        let guide = snap.guides[0];
        assert_eq!(guide.at, 100.0);
        assert_eq!(guide.start, 40.0, "up to the topmost of the three");
        assert_eq!(guide.end, 620.0, "down to the moving box");
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
        let snap = snap_free(union, (-56.0, 0.0), &[target], 1.0);
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
        let snap = snap_free(start, (5.0, 0.0), &[], 1.0);
        assert!((snap.delta.0 - 0.0).abs() < 0.001, "{:?}", snap.delta);
        assert_eq!(snap.guides.len(), 1, "the origin still explains itself");
    }

    #[test]
    fn the_page_origin_competes_with_a_real_edge_on_ordinary_terms() {
        // The origin is an ordinary edge candidate, not a special case that
        // always wins or always loses: a real edge that is nearer takes the
        // snap, and the origin is what remains when nothing else is in reach.
        let target = Rect::new(96.0, 40.0, 60.0, 30.0);
        let start = Rect::new(0.0, 300.0, 40.0, 20.0);
        // Left edge 100: 4 from the target's left edge at 96, and 100 from the
        // origin. The object's edge is nearer, so the object's edge wins.
        let snap = snap_free(start, (100.0, 0.0), &[target], 1.0);
        assert_eq!(snap.guides[0].at, 96.0, "{:?}", snap.guides);
        assert!((snap.delta.0 - 96.0).abs() < 0.001, "{:?}", snap.delta);
    }

    #[test]
    fn an_axis_lock_holds_the_axis_the_pointer_is_not_moving() {
        // The mapping between a movement axis and a line axis is the one thing
        // here that can be written backwards and still look right, so it is
        // pinned from both ends.
        assert!(AxisLock::X.allows(Axis::Vertical), "x may move");
        assert!(!AxisLock::X.allows(Axis::Horizontal), "y is held");
        assert!(!AxisLock::Y.allows(Axis::Vertical), "x is held");
        assert!(AxisLock::Y.allows(Axis::Horizontal), "y may move");
        assert!(AxisLock::Free.allows(Axis::Vertical));
        assert!(AxisLock::Free.allows(Axis::Horizontal));
        // A vertical line is the one that lines x up — the inversion is the
        // whole reason the mapping is asked about rather than inlined.
        assert!(Axis::Vertical.lines_x());
        assert!(!Axis::Horizontal.lines_x());
    }

    #[test]
    fn an_alignment_the_user_never_moved_towards_is_not_a_snap() {
        // The box already shares an edge before the pointer has moved. There is
        // no correction to apply, so claiming a snap would mean a guide drawn
        // for an alignment the user did not create.
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(100.0, 200.0, 60.0, 20.0);
        let snap = snap_free(start, (0.0, 0.0), &[target], 1.0);
        assert_eq!(snap.delta, (0.0, 0.0));
        assert!(snap.guides.is_empty());
    }

    #[test]
    fn arriving_exactly_on_a_line_is_not_a_correction() {
        // The one case the zero-distance guard actually decides. The pointer has
        // moved — one pixel left, which is why this reaches `best_on_axis` at
        // all — and the box lands exactly on the target's left edge. There is no
        // correction to apply and no line the user is being pulled towards, so
        // claiming a snap here would draw a guide for a drag that already did
        // what was asked.
        let target = Rect::new(100.0, 40.0, 80.0, 30.0);
        let start = Rect::new(101.0, 200.0, 60.0, 20.0);
        let snap = snap_free(start, (-1.0, 0.0), &[target], 1.0);
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
        let snap = snap_free(start, (1.0, 0.0), &[target], 1.0);
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
        let snap = snap_free(start, (0.0, 0.0), &[target], 1.0);
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
        let snap = snap_free(start, (1.0, 0.0), &[target], 1.0);
        assert_eq!(snap.delta, (0.0, 0.0), "held in line");
        assert_eq!(snap.guides.len(), 1, "and it says why");
        assert_eq!(snap.guides[0].at, 50.0);
    }

    #[test]
    fn a_constrained_axis_is_never_snapped() {
        // `⇧` promises the box moves on one axis only. A snap is a correction
        // on an axis the pointer is not otherwise moving, so without the lock
        // the magnet would break the promise the constraint just made: this
        // drag has delta.y == 0, and the box would still rise to meet the
        // target's centre line.
        let target = Rect::new(0.0, 0.0, 200.0, 100.0);
        let start = Rect::new(0.0, 107.0, 40.0, 20.0);
        let unconstrained = snap_free(start, (3.0, 0.0), &[target], 1.0);
        assert_eq!(
            unconstrained.delta,
            (0.0, -7.0),
            "free axes snap vertically too: {:?}",
            unconstrained.delta
        );
        let constrained = snap_translation(start, (3.0, 0.0), &[target], 1.0, AxisLock::X);
        assert_eq!(
            constrained.delta,
            (0.0, 0.0),
            "the locked axis is untouched: {:?}",
            constrained.delta
        );
        assert_eq!(constrained.guides.len(), 1, "{:?}", constrained.guides);
        assert_eq!(
            constrained.guides[0].axis,
            Axis::Vertical,
            "and no guide is drawn for an axis that did not move"
        );
    }

    #[test]
    fn a_zero_delta_component_is_not_taken_for_a_constraint() {
        // The other half of the rule above. Dragging purely sideways across a
        // neighbour's centre line should still click into line vertically: the
        // guide is what makes that legible, and every product does it. So the
        // lock has to be stated by the caller, never inferred from a zero.
        let target = Rect::new(0.0, 0.0, 200.0, 100.0);
        let start = Rect::new(0.0, 107.0, 40.0, 20.0);
        let snap = snap_free(start, (3.0, 0.0), &[target], 1.0);
        assert_eq!(snap.delta, (0.0, -7.0), "{:?}", snap.delta);
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
        assert!(snap_free(start, (60.0, 0.0), &[target], 0.0)
            .guides
            .is_empty());
    }

    #[test]
    fn a_resized_edge_snaps_like_a_moved_one() {
        // Dragging the right handle to within a few pixels of a neighbour's left
        // edge butts the two boxes up together, which is the resize gesture
        // every product in the corpus provides.
        let target = Rect::new(300.0, 40.0, 60.0, 30.0);
        let start = Rect::new(0.0, 40.0, 200.0, 30.0);
        let moved = MovedEdges {
            right: true,
            ..MovedEdges::default()
        };
        // Right edge 297, three short of the target's left edge at 300.
        let snap = snap_resize(start, (97.0, 0.0), moved, &[target], 1.0, 1.0);
        assert!(
            (start.right() + snap.delta.0 - 300.0).abs() < 0.001,
            "right edges in line: {:?}",
            snap.delta
        );
        assert_eq!(snap.guides.len(), 1, "{:?}", snap.guides);
        assert_eq!(snap.guides[0].axis, Axis::Vertical);
        assert_eq!(snap.guides[0].at, 300.0);
    }

    #[test]
    fn a_corner_handle_snaps_on_both_axes() {
        let target = Rect::new(0.0, 0.0, 200.0, 100.0);
        let start = Rect::new(300.0, 300.0, 80.0, 40.0);
        let moved = MovedEdges {
            left: true,
            top: true,
            ..MovedEdges::default()
        };
        // Moved to left 4 and top 96: four short of the target's left edge at
        // 0 and of its bottom edge at 100.
        let snap = snap_resize(start, (-296.0, -204.0), moved, &[target], 1.0, 1.0);
        assert!(
            (start.left() + snap.delta.0 - 0.0).abs() < 0.001,
            "the left edges meet: {:?}",
            snap.delta
        );
        assert!(
            (start.top() + snap.delta.1 - 100.0).abs() < 0.001,
            "and the tops land on the target's bottom: {:?}",
            snap.delta
        );
        assert_eq!(snap.guides.len(), 2, "{:?}", snap.guides);
        assert_eq!(snap.guides[0].axis, Axis::Vertical);
        assert_eq!(snap.guides[0].at, 0.0);
        assert_eq!(snap.guides[1].axis, Axis::Horizontal);
        assert_eq!(snap.guides[1].at, 100.0);
    }

    #[test]
    fn a_handle_that_moves_no_edge_on_an_axis_snaps_nothing_on_it() {
        // `Right` moves the right edge and leaves the top and bottom exactly
        // where they were. Aligning the untouched vertical extent to a centre
        // line would drag the box's height around to suit the pointer's
        // horizontal travel.
        let target = Rect::new(0.0, 0.0, 200.0, 100.0);
        let start = Rect::new(0.0, 103.0, 196.0, 20.0);
        let moved = MovedEdges {
            right: true,
            ..MovedEdges::default()
        };
        // Right edge 196, four short of the target's right edge at 200. The
        // vertical travel lands the top at 110, and the target's horizontal
        // lines at 0, 50 and 100 are all far from it.
        let snap = snap_resize(start, (0.0, 7.0), moved, &[target], 1.0, 1.0);
        assert_eq!(
            snap.delta,
            (4.0, 7.0),
            "the vertical pointer travel is kept verbatim: {:?}",
            snap.delta
        );
        assert_eq!(snap.guides.len(), 1, "{:?}", snap.guides);
        assert_eq!(snap.guides[0].axis, Axis::Vertical);
        assert_eq!(snap.guides[0].at, 200.0);
    }

    #[test]
    fn a_resize_snap_that_would_break_the_minimum_size_is_declined() {
        // The target's left edge is 300 away and the box is 200 wide, so
        // snapping the right handle there would leave a box of minus 100. The
        // guide would claim an alignment the caller's own minimum-size clamp
        // then threw away, which is worse than not snapping.
        let target = Rect::new(300.0, 40.0, 60.0, 30.0);
        let start = Rect::new(0.0, 40.0, 40.0, 30.0);
        let moved = MovedEdges {
            right: true,
            ..MovedEdges::default()
        };
        let snap = snap_resize(start, (100.0, 0.0), moved, &[target], 1.0, 1.0);
        assert_eq!(snap.delta, (100.0, 0.0), "no correction: {:?}", snap.delta);
        assert!(snap.guides.is_empty(), "no alignment, no guide");
    }

    #[test]
    fn a_resize_nearest_candidate_wins_and_guides_its_line() {
        // Two targets, both within reach of the right handle; the nearer one
        // decides, and the guide spans both boxes so the user can see which.
        let near = Rect::new(250.0, 0.0, 40.0, 20.0);
        let far = Rect::new(254.0, 200.0, 40.0, 20.0);
        let start = Rect::new(0.0, 100.0, 200.0, 60.0);
        let moved = MovedEdges {
            right: true,
            ..MovedEdges::default()
        };
        let snap = snap_resize(start, (47.0, 0.0), moved, &[near, far], 1.0, 1.0);
        assert!(
            (start.right() + snap.delta.0 - 250.0).abs() < 0.001,
            "{:?}",
            snap.delta
        );
        assert_eq!(snap.guides[0].at, 250.0);
        assert_eq!(snap.guides[0].start, 0.0, "up to the target's top");
        assert_eq!(snap.guides[0].end, 160.0, "down to the box's bottom");
    }

    #[test]
    fn a_resize_beats_the_threshold_on_the_same_screen_space_rule() {
        let target = Rect::new(0.0, 0.0, 200.0, 100.0);
        let start = Rect::new(0.0, 0.0, 100.0, 50.0);
        let moved = MovedEdges {
            right: true,
            bottom: true,
            ..MovedEdges::default()
        };
        for zoom in [0.25, 1.0, 4.0] {
            let gap = 6.0 / zoom;
            let inside = snap_resize(start, (100.0 + gap, 0.0), moved, &[target], zoom, 1.0);
            assert_eq!(
                inside.guides.len(),
                1,
                "6 screen px must snap the right edge at {zoom}x"
            );
            let outside = 100.0 + (DEFAULT_THRESHOLD_PX + 0.5) / zoom;
            let past = snap_resize(start, (outside, 0.0), moved, &[target], zoom, 1.0);
            assert!(
                past.guides.is_empty(),
                "8.5 screen px must not snap at {zoom}x"
            );
        }
    }

    #[test]
    fn a_resize_that_has_not_moved_proposes_nothing() {
        // The same rule as a move, and for the same reason: a click on a handle
        // must not resize anything, so it must not align anything either.
        let target = Rect::new(0.0, 0.0, 200.0, 100.0);
        let start = Rect::new(0.0, 0.0, 103.0, 50.0);
        let moved = MovedEdges {
            right: true,
            ..MovedEdges::default()
        };
        let snap = snap_resize(start, (0.0, 0.0), moved, &[target], 1.0, 1.0);
        assert_eq!(snap.delta, (0.0, 0.0));
        assert!(snap.guides.is_empty());
    }

    #[test]
    fn an_axis_whose_two_edges_both_move_is_declined_rather_than_guessed() {
        // A stretch has no single edge to align, and resolving it would mean
        // solving for two corrections at once. Declining is the honest answer:
        // the drag still works, it just does not snap on that axis.
        let target = Rect::new(0.0, 0.0, 200.0, 100.0);
        let start = Rect::new(0.0, 0.0, 100.0, 50.0);
        let moved = MovedEdges {
            left: true,
            right: true,
            ..MovedEdges::default()
        };
        let snap = snap_resize(start, (7.0, 0.0), moved, &[target], 1.0, 1.0);
        assert_eq!(snap.delta, (7.0, 0.0));
        assert!(snap.guides.is_empty(), "{:?}", snap.guides);
    }
}
