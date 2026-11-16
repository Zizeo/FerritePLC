//! Pure geometry and drop-resolution helpers for the FBD canvas.
//!
//! No rendering and no `Message` knowledge lives here: every helper takes a
//! block (plus its origin in world pixels) and returns a `Rectangle` in world
//! pixels, or resolves a cursor position to an input pin. That keeps the
//! hit-test rules testable without a renderer.

use std::collections::HashMap;

use ferrite_lang_fbd::{CompositeFbDef, FbdBlock, FbdBlockKind, FbdInputSource};
use iced::{Point, Rectangle, Size};

/// Vertical pitch of one pin row.
pub const ROW_H: f32 = 22.0;
/// Height of the block header (glyph, kind badge, id field).
pub const HEADER_H: f32 = 24.0;
/// Minimum block width; wider text grows the block up to [`MAX_BLOCK_W`].
pub const MIN_BLOCK_W: f32 = 180.0;
/// Upper bound on block width; long expressions truncate at rest.
pub const MAX_BLOCK_W: f32 = 480.0;
/// How far outside a block body a wire drop still counts as a hit.
pub const DROP_INFLATE: f32 = 12.0;
/// Width of the `×` affordance at the end of a row.
pub const DELETE_W: f32 = 14.0;
/// Height of the `+ pin` footer affordance.
pub const ADD_PIN_H: f32 = 20.0;
/// Monospace advance used to size text-derived geometry.
const CHAR_W: f32 = 7.0;
/// Left inset of the pin label inside a row.
pub const ROW_INSET: f32 = 8.0;
/// Width reserved for the pin label (`IN10`, `ET`, …).
const PIN_LABEL_W: f32 = 44.0;

/// Kind glyph used by the header and the kind badge.
pub fn block_glyph(kind: &FbdBlockKind) -> &'static str {
    match kind {
        FbdBlockKind::And => "&",
        FbdBlockKind::Or => "\u{2265}1",
        FbdBlockKind::Xor => "=1",
        FbdBlockKind::Not => "1",
        FbdBlockKind::Add => "+",
        FbdBlockKind::Sub => "\u{2212}",
        FbdBlockKind::Mul => "\u{00d7}",
        FbdBlockKind::Div => "\u{00f7}",
        FbdBlockKind::Eq => "=",
        FbdBlockKind::Ne => "\u{2260}",
        FbdBlockKind::Lt => "<",
        FbdBlockKind::Le => "\u{2264}",
        FbdBlockKind::Gt => ">",
        FbdBlockKind::Ge => "\u{2265}",
        FbdBlockKind::StandardFb { .. } => "FB",
        FbdBlockKind::Composite { .. } => "CFB",
    }
}

/// Kinds whose input list the user may grow and shrink with `+ pin` / `×`.
pub fn is_variadic(kind: &FbdBlockKind) -> bool {
    matches!(
        kind,
        FbdBlockKind::And
            | FbdBlockKind::Or
            | FbdBlockKind::Xor
            | FbdBlockKind::Add
            | FbdBlockKind::Mul
    )
}

/// Input pin names in display order: numeric `IN*` pins first by number, then
/// the rest alphabetically (timers expose `IN`/`PT`, counters `CU`/`R`/`PV`).
pub fn input_pins(blk: &FbdBlock) -> Vec<String> {
    let mut pins: Vec<String> = blk.inputs.keys().cloned().collect();
    pins.sort_by(|a, b| {
        let rank = |p: &str| {
            p.strip_prefix("IN")
                .and_then(|n| n.parse::<u32>().ok())
                .map_or((1_u8, 0_u32, p.to_string()), |n| (0, n, p.to_string()))
        };
        rank(a).cmp(&rank(b))
    });
    pins
}

/// Output pin names the block should render rows for: the kind's own pins
/// first, then any already bound in `outputs` that the kind does not declare.
pub fn output_pins(blk: &FbdBlock, composites: &[CompositeFbDef]) -> Vec<String> {
    let mut pins = default_output_pins(&blk.kind, composites);
    let mut configured: Vec<String> = blk.outputs.keys().cloned().collect();
    configured.sort();
    for pin in configured {
        if !pins.iter().any(|p| p.eq_ignore_ascii_case(&pin)) {
            pins.push(pin);
        }
    }
    pins
}

/// Output pins a freshly created block of `kind` can bind, before any binding
/// exists. This is what makes an untouched block show bindable outputs.
pub fn default_output_pins(kind: &FbdBlockKind, composites: &[CompositeFbDef]) -> Vec<String> {
    match kind {
        FbdBlockKind::StandardFb {
            fb_type: Some(fb_kind),
            ..
        } => match fb_kind {
            ferrite_core::StandardFbKind::Ton
            | ferrite_core::StandardFbKind::Tof
            | ferrite_core::StandardFbKind::Tp => {
                vec!["Q".to_string(), "ET".to_string()]
            }
            ferrite_core::StandardFbKind::Ctu | ferrite_core::StandardFbKind::Ctd => {
                vec!["Q".to_string(), "CV".to_string()]
            }
            ferrite_core::StandardFbKind::Ctud => {
                vec!["QU".to_string(), "QD".to_string(), "CV".to_string()]
            }
            ferrite_core::StandardFbKind::ScaleLin
            | ferrite_core::StandardFbKind::P
            | ferrite_core::StandardFbKind::Pi
            | ferrite_core::StandardFbKind::PidParallel
            | ferrite_core::StandardFbKind::PidSeries
            | ferrite_core::StandardFbKind::PidMixed => {
                vec!["OUT".to_string(), "ERR".to_string()]
            }
            _ => vec!["Q".to_string()],
        },
        FbdBlockKind::StandardFb { fb_type: None, .. } => {
            vec!["Q".to_string(), "OUT".to_string()]
        }
        FbdBlockKind::Composite { type_name } => composites
            .iter()
            .find(|c| &c.name == type_name)
            .map(|c| c.output_pins.clone())
            .filter(|pins| !pins.is_empty())
            .unwrap_or_else(|| vec!["OUT".to_string()]),
        _ => vec!["OUT".to_string()],
    }
}

/// Text a row shows at rest: the link source, the literal, or `(0)`/`(unbound)`.
pub fn input_row_text(src: Option<&FbdInputSource>) -> String {
    match src {
        Some(FbdInputSource::Value { expr }) if !expr.trim().is_empty() => expr.clone(),
        Some(FbdInputSource::Link {
            from_block,
            from_pin,
        }) => format!("{from_block}.{from_pin}"),
        _ => String::new(),
    }
}

/// Width of the block: the longest row wins, clamped to
/// [`MIN_BLOCK_W`]..[`MAX_BLOCK_W`]. `editing_text` lets the row being edited
/// grow the block so the caret never clips.
pub fn block_width(blk: &FbdBlock, composites: &[CompositeFbDef], editing_text: Option<&str>) -> f32 {
    let mut longest = 0_usize;
    let id_len = blk.id.chars().count() + 2;
    longest = longest.max(id_len);

    for pin in input_pins(blk) {
        let text = input_row_text(blk.inputs.get(&pin));
        longest = longest.max(pin.chars().count() + 2 + text.chars().count());
    }
    for pin in output_pins(blk, composites) {
        let text = blk.outputs.get(&pin).map(String::as_str).unwrap_or("");
        longest = longest.max(pin.chars().count() + 2 + text.chars().count());
    }
    if let Some(extra) = editing_text {
        longest = longest.max(extra.chars().count() + PIN_LABEL_W as usize + 4);
    }

    let needed = longest as f32 * CHAR_W + PIN_LABEL_W + DELETE_W + ROW_INSET * 2.0 + 12.0;
    needed.clamp(MIN_BLOCK_W, MAX_BLOCK_W)
}

/// Size of the whole block: header, one row per input and output, then the
/// `+ pin` footer for variadic kinds.
pub fn block_size(blk: &FbdBlock, composites: &[CompositeFbDef]) -> Size {
    let rows = input_pins(blk).len().max(output_pins(blk, composites).len());
    let footer = if is_variadic(&blk.kind) { ADD_PIN_H } else { 0.0 };
    Size::new(
        block_width(blk, composites, None),
        HEADER_H + rows as f32 * ROW_H + footer,
    )
}

/// Block body rectangle at `origin`.
pub fn block_rect(blk: &FbdBlock, composites: &[CompositeFbDef], origin: Point) -> Rectangle {
    Rectangle::new(origin, block_size(blk, composites))
}

/// Whole header strip.
pub fn header_rect(blk: &FbdBlock, composites: &[CompositeFbDef], origin: Point) -> Rectangle {
    let width = block_size(blk, composites).width;
    Rectangle::new(origin, Size::new(width, HEADER_H))
}

/// Kind badge at the left of the header; clicking it opens the kind menu.
pub fn kind_badge_rect(blk: &FbdBlock, composites: &[CompositeFbDef], origin: Point) -> Rectangle {
    Rectangle::new(
        Point::new(origin.x + ROW_INSET, origin.y + 3.0),
        Size::new(30.0, HEADER_H - 6.0),
    )
    .intersection(&header_rect(blk, composites, origin))
    .unwrap_or_default()
}

/// Editable block-id field in the header, right of the kind badge.
pub fn id_rect(blk: &FbdBlock, composites: &[CompositeFbDef], origin: Point) -> Rectangle {
    let header = header_rect(blk, composites, origin);
    let badge = kind_badge_rect(blk, composites, origin);
    let x = badge.x + badge.width + 6.0;
    Rectangle::new(
        Point::new(x, origin.y + 3.0),
        Size::new((header.x + header.width - x - ROW_INSET).max(0.0), HEADER_H - 6.0),
    )
}

/// Full-width hit strip of input row `i`.
pub fn input_row_rect(origin: Point, width: f32, i: usize) -> Rectangle {
    Rectangle::new(
        Point::new(origin.x, origin.y + HEADER_H + i as f32 * ROW_H),
        Size::new(width, ROW_H),
    )
}

/// Full-width hit strip of output row `i`, below every input row.
pub fn output_row_rect(origin: Point, width: f32, inputs: usize, i: usize) -> Rectangle {
    Rectangle::new(
        Point::new(
            origin.x,
            origin.y + HEADER_H + (inputs + i) as f32 * ROW_H,
        ),
        Size::new(width, ROW_H),
    )
}

/// Editable value area of input row `i` (right of the pin label, left of `×`).
pub fn input_value_rect(origin: Point, width: f32, i: usize) -> Rectangle {
    value_area(input_row_rect(origin, width, i))
}

/// Editable target area of output row `i`.
pub fn output_value_rect(origin: Point, width: f32, inputs: usize, i: usize) -> Rectangle {
    value_area(output_row_rect(origin, width, inputs, i))
}

fn value_area(row: Rectangle) -> Rectangle {
    let x = row.x + ROW_INSET + PIN_LABEL_W;
    let right = row.x + row.width - ROW_INSET - DELETE_W - 2.0;
    Rectangle::new(
        Point::new(x, row.y + 2.0),
        Size::new((right - x).max(0.0), row.height - 4.0),
    )
}

/// The `×` affordance at the end of input row `i`.
pub fn input_delete_rect(origin: Point, width: f32, i: usize) -> Rectangle {
    delete_area(input_row_rect(origin, width, i))
}

/// The `×` affordance at the end of output row `i`.
pub fn output_delete_rect(origin: Point, width: f32, inputs: usize, i: usize) -> Rectangle {
    delete_area(output_row_rect(origin, width, inputs, i))
}

fn delete_area(row: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(row.x + row.width - ROW_INSET - DELETE_W, row.y + 3.0),
        Size::new(DELETE_W, ROW_H - 6.0),
    )
}

/// The `+ pin` footer affordance; `None` for non-variadic kinds.
pub fn add_pin_rect(blk: &FbdBlock, composites: &[CompositeFbDef], origin: Point) -> Option<Rectangle> {
    if !is_variadic(&blk.kind) {
        return None;
    }
    let size = block_size(blk, composites);
    let rows = input_pins(blk).len().max(output_pins(blk, composites).len());
    Some(Rectangle::new(
        Point::new(origin.x + ROW_INSET, origin.y + HEADER_H + rows as f32 * ROW_H),
        Size::new((size.width - ROW_INSET * 2.0).max(0.0), ADD_PIN_H),
    ))
}

/// Horizontal centre of the input pin circle on row `i`.
pub fn input_pin_point(origin: Point, i: usize) -> Point {
    Point::new(origin.x, origin.y + HEADER_H + i as f32 * ROW_H + ROW_H * 0.5)
}

/// Horizontal centre of the output pin circle on output row `i`.
pub fn output_pin_point(origin: Point, width: f32, inputs: usize, i: usize) -> Point {
    Point::new(
        origin.x + width,
        origin.y + HEADER_H + (inputs + i) as f32 * ROW_H + ROW_H * 0.5,
    )
}

fn inflate(rect: Rectangle, by: f32) -> Rectangle {
    Rectangle::new(
        Point::new(rect.x - by, rect.y - by),
        Size::new(rect.width + by * 2.0, rect.height + by * 2.0),
    )
}

/// Resolves a wire drop to an input pin.
///
/// Blocks are scanned in reverse draw order (topmost wins). A hit needs the
/// cursor inside the block's body inflated by [`DROP_INFLATE`]. Inside the
/// block the target is the input row whose centre is nearest; a drop far from
/// every row goes to the first **unlinked** pin instead, and only when every
/// pin is linked does it fall back to the nearest row.
///
/// The source block is excluded: `exclude_block` is the block the wire is
/// dragged from, so a block may never wire into itself.
pub fn resolve_input_drop<'a>(
    blocks: &'a [FbdBlock],
    positions: &HashMap<String, Point>,
    composites: &[CompositeFbDef],
    exclude_block: Option<&str>,
    cursor: Point,
) -> Option<(&'a FbdBlock, String)> {
    for blk in blocks.iter().rev() {
        if exclude_block == Some(blk.id.as_str()) {
            continue;
        }
        let Some(&origin) = positions.get(&blk.id) else {
            continue;
        };
        let body = block_rect(blk, composites, origin);
        if !inflate(body, DROP_INFLATE).contains(cursor) {
            continue;
        }
        let pins = input_pins(blk);
        if pins.is_empty() {
            continue;
        }
        let row = ((cursor.y - origin.y - HEADER_H) / ROW_H).floor();
        let nearest = if row.is_finite() && row >= 0.0 {
            (row as usize).min(pins.len() - 1)
        } else {
            usize::MAX
        };
        if nearest < pins.len() {
            let centre = input_pin_point(origin, nearest).y;
            if (cursor.y - centre).abs() <= ROW_H * 0.5 {
                return Some((blk, pins[nearest].clone()));
            }
        }
        if let Some(free) = pins
            .iter()
            .find(|pin| !matches!(blk.inputs.get(*pin), Some(FbdInputSource::Link { .. })))
        {
            return Some((blk, free.clone()));
        }
        let nearest = if nearest == usize::MAX { 0 } else { nearest };
        return Some((blk, pins[nearest].clone()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blk(id: &str, kind: FbdBlockKind, pins: &[&str]) -> FbdBlock {
        FbdBlock {
            id: id.to_string(),
            kind,
            inputs: pins
                .iter()
                .map(|p| {
                    (
                        p.to_string(),
                        FbdInputSource::Value {
                            expr: "FALSE".to_string(),
                        },
                    )
                })
                .collect(),
            outputs: HashMap::new(),
            grid_pos: None,
        }
    }

    fn linked(id: &str, kind: FbdBlockKind, pins: &[&str]) -> FbdBlock {
        FbdBlock {
            id: id.to_string(),
            kind,
            inputs: pins
                .iter()
                .map(|p| {
                    (
                        p.to_string(),
                        FbdInputSource::Link {
                            from_block: "SRC".to_string(),
                            from_pin: "OUT".to_string(),
                        },
                    )
                })
                .collect(),
            outputs: HashMap::new(),
            grid_pos: None,
        }
    }

    fn positions(entries: &[(&str, Point)]) -> HashMap<String, Point> {
        entries
            .iter()
            .map(|(id, pt)| (id.to_string(), *pt))
            .collect()
    }

    #[test]
    fn test_block_height_grows_with_variadic_pins() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        let two = blk("A", FbdBlockKind::And, &["IN1", "IN2"]);
        let three = blk("A", FbdBlockKind::And, &["IN1", "IN2", "IN3"]);
        let h2 = block_size(&two, &composites).height;
        let h3 = block_size(&three, &composites).height;
        assert_eq!(h3 - h2, ROW_H, "one extra input adds exactly one row");
    }

    #[test]
    fn test_block_width_respects_bounds() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        let narrow = blk("A", FbdBlockKind::And, &["IN1"]);
        assert!(block_width(&narrow, &composites, None) >= MIN_BLOCK_W);

        let mut wide = blk("A", FbdBlockKind::And, &["IN1"]);
        wide.inputs.insert(
            "IN1".to_string(),
            FbdInputSource::Value {
                expr: "x".repeat(400),
            },
        );
        assert_eq!(block_width(&wide, &composites, None), MAX_BLOCK_W);
    }

    #[test]
    fn test_input_pins_sort_numerically() {
        let b = blk("A", FbdBlockKind::And, &["IN10", "IN2", "IN1"]);
        assert_eq!(input_pins(&b), vec!["IN1", "IN2", "IN10"]);
    }

    #[test]
    fn test_drop_resolves_nearest_row() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        let target = blk("B", FbdBlockKind::And, &["IN1", "IN2", "IN3"]);
        let blocks = vec![target];
        let origin = Point::new(100.0, 60.0);
        let pos = positions(&[("B", origin)]);

        // Vertical centre of input row 2 (IN2).
        let cursor = Point::new(origin.x + 80.0, input_pin_point(origin, 1).y);
        let hit = resolve_input_drop(&blocks, &pos, &composites, Some("A"), cursor);
        assert_eq!(hit.map(|(b, pin)| (b.id.clone(), pin)), Some(("B".to_string(), "IN2".to_string())));
    }

    #[test]
    fn test_drop_prefers_first_unlinked_pin() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        // IN1 and IN2 are linked; IN3 is still a literal.
        let mut target = linked("B", FbdBlockKind::And, &["IN1", "IN2"]);
        target.inputs.insert(
            "IN3".to_string(),
            FbdInputSource::Value {
                expr: "FALSE".to_string(),
            },
        );
        let blocks = vec![target];
        let origin = Point::new(0.0, 0.0);
        let pos = positions(&[("B", origin)]);

        // Drop in the footer area, far from every row centre.
        let cursor = Point::new(origin.x + 40.0, origin.y + HEADER_H + 3.0 * ROW_H + 8.0);
        let hit = resolve_input_drop(&blocks, &pos, &composites, None, cursor);
        assert_eq!(hit.map(|(_, pin)| pin), Some("IN3".to_string()));
    }

    #[test]
    fn test_drop_falls_back_to_nearest_when_all_linked() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        let target = linked("B", FbdBlockKind::And, &["IN1", "IN2"]);
        let blocks = vec![target];
        let origin = Point::new(0.0, 0.0);
        let pos = positions(&[("B", origin)]);

        let cursor = Point::new(origin.x + 40.0, origin.y + HEADER_H + 3.0 * ROW_H);
        let hit = resolve_input_drop(&blocks, &pos, &composites, None, cursor);
        assert_eq!(hit.map(|(_, pin)| pin), Some("IN2".to_string()));
    }

    #[test]
    fn test_drop_misses_outside_inflated_box() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        let target = blk("B", FbdBlockKind::And, &["IN1", "IN2"]);
        let size = block_size(&target, &composites);
        let blocks = vec![target];
        let origin = Point::new(0.0, 0.0);
        let pos = positions(&[("B", origin)]);

        let cursor = Point::new(size.width + DROP_INFLATE + 1.0, size.height * 0.5);
        assert!(resolve_input_drop(&blocks, &pos, &composites, None, cursor).is_none());
    }

    #[test]
    fn test_drop_skips_source_block() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        let target = blk("B", FbdBlockKind::And, &["IN1", "IN2"]);
        let blocks = vec![target];
        let origin = Point::new(0.0, 0.0);
        let pos = positions(&[("B", origin)]);
        let cursor = Point::new(40.0, input_pin_point(origin, 0).y);
        assert!(resolve_input_drop(&blocks, &pos, &composites, Some("B"), cursor).is_none());
    }

    #[test]
    fn test_output_rows_present_for_unbound_block() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        let and = blk("A", FbdBlockKind::And, &["IN1", "IN2"]);
        assert_eq!(default_output_pins(&and.kind, &composites), vec!["OUT"]);
        assert_eq!(output_pins(&and, &composites), vec!["OUT"]);
        assert!(output_value_rect(Point::ORIGIN, 200.0, 2, 0).width > 0.0);

        let ton = FbdBlock {
            id: "T1".to_string(),
            kind: FbdBlockKind::StandardFb {
                instance: "T1".to_string(),
                fb_type: Some(ferrite_core::StandardFbKind::Ton),
            },
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            grid_pos: None,
        };
        assert_eq!(output_pins(&ton, &composites), vec!["Q", "ET"]);
        // Two output rows, zero inputs: the block still has height for both.
        let size = block_size(&ton, &composites);
        assert_eq!(size.height, HEADER_H + 2.0 * ROW_H);
    }

    #[test]
    fn test_variadic_footer_only_for_growable_kinds() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        let and = blk("A", FbdBlockKind::And, &["IN1", "IN2"]);
        let not = blk("N", FbdBlockKind::Not, &["IN"]);
        assert!(add_pin_rect(&and, &composites, Point::ORIGIN).is_some());
        assert!(add_pin_rect(&not, &composites, Point::ORIGIN).is_none());
        assert!(is_variadic(&FbdBlockKind::Mul));
        assert!(!is_variadic(&FbdBlockKind::Sub));
    }

    #[test]
    fn test_row_rects_do_not_overlap() {
        let composites: Vec<CompositeFbDef> = Vec::new();
        let b = blk("A", FbdBlockKind::And, &["IN1", "IN2"]);
        let size = block_size(&b, &composites);
        let r0 = input_row_rect(Point::ORIGIN, size.width, 0);
        let r1 = input_row_rect(Point::ORIGIN, size.width, 1);
        // Two inputs, one output: the output row reuses row 0's slot, so it
        // must start at the header, not below the input rows.
        let out = output_row_rect(Point::ORIGIN, size.width, 0, 0);
        assert_eq!(r0.y + r0.height, r1.y);
        assert_eq!(out.y, r0.y, "one output shares row 0's slot");
        assert!(r1.y + r1.height <= size.height);
        // The value field sits strictly inside its row.
        let v = input_value_rect(Point::ORIGIN, size.width, 0);
        assert!(v.x > r0.x && v.x + v.width < r0.x + r0.width);
    }

    #[test]
    fn test_input_row_text_renders_links_and_literals() {
        assert_eq!(
            input_row_text(Some(&FbdInputSource::Link {
                from_block: "A".to_string(),
                from_pin: "OUT".to_string(),
            })),
            "A.OUT"
        );
        assert_eq!(
            input_row_text(Some(&FbdInputSource::Value {
                expr: "TRUE".to_string()
            })),
            "TRUE"
        );
        assert_eq!(input_row_text(None), "");
    }
}