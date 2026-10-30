# Structured Rung Grid Canvas for Ladder Diagram (LD) POUs Design Spec

## 1. Intent & The Why
FerritePLC currently provides a rich interactive vector 2D canvas for Sequential Function Charts (SFC / Grafcet), featuring smooth pan/zoom, grid movement, marquee multi-selection, mass deletion, and interactive wiring. In contrast, the Ladder Diagram (LD) editor in `ferrite-gui` (`canvas_ld.rs`) uses basic stacked container widgets in a linear scrollable list.

This design elevates the Ladder editor to the same first-class vector canvas experience as SFC:
- Structured industrial rung grid (modeled after Rockwell RSLogix, Siemens TIA Portal, and Codesys).
- Full interactive canvas powered by `iced::widget::canvas::Canvas`.
- Free movement and reordering of contacts, coils, and branches on the grid with drag-and-drop.
- Multi-selection (single-click, `Ctrl+Click`, and click-drag marquee box).
- Mass deletion of selected elements and rungs in one go.
- True graphical parallel branches (OR branches) with bypass loop rendering.
- Live energized power-flow visualization (emerald green glow) and bottom inspector bar.

---

## 2. Goals, Non-Goals, & Anti-Goals

### Goals
- **Vector Canvas Engine**: Implement an `iced::widget::canvas::Program` for Ladder logic (`LdCanvas`) in `crates/ferrite-gui/src/canvas_ld.rs`.
- **Rung & Rail Geometry**:
  - Vertical left (energized) and right (neutral) power rails spanning all rungs.
  - Horizontal rungs on a structured grid with discrete slots for contacts, inline function blocks, coils, and parallel branches.
  - Parallel branches render as graphical step-down bypass loops with vertical drop wires, horizontal sub-paths, and vertical step-up merge wires.
- **Interactions**:
  - **Single & Multi-Select**: Click to select, `Ctrl+Click` to toggle/add, and click-drag marquee box to select multiple elements across or within rungs.
  - **Drag-and-Drop**: Drag contacts, coils, or entire branches to move them along a rung, move them between rungs, or nest them into parallel branches, with clear visual drop targets.
  - **Batch Delete**: Pressing `Delete` key or clicking the trash toolbar button removes all selected elements in one atomic operation, automatically closing wire gaps.
  - **Pan & Zoom**: Mouse wheel zoom (0.25x to 3.0x) and middle-mouse or space-drag panning.
  - **Inspector**: Bottom context bar displaying selected element properties (tag name, contact kind NO/NC/P/N, coil kind ( )/[S]/[R], inline FB instance, rung comments).
  - **Live Power Flow**: Real-time evaluation of energized state from `PlcRuntime`, highlighting active rails, wires, contacts, and energized coils in emerald green.
- **Backward Compatibility**:
  - Works transparently with existing `LdProgram`, `LdRung`, `LdElement`, and `LdCoil` in `ferrite-lang-ld`. No schema migration or breaking changes to `.ld.json` files.

### Non-Goals
- Modifying the underlying `ferrite-lang-ld` compiler IR or runtime execution loop (they already evaluate `LdProgram` and `Branch { paths }`).
- Arbitrary diagonal or unconstrained electronic schematic wiring (ladder strictly adheres to orthogonal horizontal rungs and vertical branch lines).

### Anti-Goals
- Breaking existing `.ld.json` project files.
- Sluggish performance or high latency during marquee selection or canvas rendering.
- Unhandled unwrap or panic states on empty rungs or invalid drop operations.

---

## 3. Constraints
- Rust 2024 Edition.
- Zero `unwrap()` / `expect()` calls in library crates and GUI codebase.
- Maintain strict real-time safety: zero memory allocations in runtime scan loops.
- Pass all unit and integration tests across the workspace (`cargo test --workspace`).
- Clean clippy with zero warnings (`cargo clippy --workspace --all-targets -- -D warnings`).

---

## 4. Approaches Considered
- **Approach A: Vector `iced::Canvas` with Grid-Slot Architecture (Picked)**
  - Replaces iced container widget list with vector `iced::Canvas` implementing `iced::widget::canvas::Program`.
  - Derives slot layout deterministically from `LdRung` and `LdElement::Branch` structures.
  - Delivers authentic industrial PLC ladder look and feel, multi-selection, marquee drag, and parallel branch loops without breaking AST schema.
- **Approach B: Explicit Coordinate Grid (`grid_pos`) with Free Routing (Rejected)**
  - Would require changing `LdElement` and `LdCoil` in `ferrite-lang-ld` to store `grid_pos: Option<(i32, i32)>`.
  - Introduces floating contact ambiguities and breaks existing `.ld.json` project files.
- **Approach C: Hybrid Container Drag-and-Drop (Rejected)**
  - Keeping iced `row`/`column` widgets with drag overlays cannot provide smooth vector zoom/pan, marquee selection, or clean graphical parallel bypass loop rendering.

---

## 5. Detailed Technical Specification

### 5.1 Element Addressing & Identification
To uniquely address elements for selection, dragging, and deletion inside nested rungs and parallel branches:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LdElementKey {
    Rung(usize),
    Contact {
        rung: usize,
        /// Path of branch indices if inside nested parallel branches (e.g. `vec![0, 1]`)
        branch_path: Vec<usize>,
        index: usize,
    },
    Coil {
        rung: usize,
        index: usize,
    },
    Branch {
        rung: usize,
        branch_path: Vec<usize>,
        index: usize,
    },
    InlineFb {
        rung: usize,
        branch_path: Vec<usize>,
        index: usize,
    },
}
```

### 5.2 Drop Target Identification
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LdDropTarget {
    BeforeElement {
        rung: usize,
        branch_path: Vec<usize>,
        index: usize,
    },
    AfterElement {
        rung: usize,
        branch_path: Vec<usize>,
        index: usize,
    },
    NewBranch {
        rung: usize,
        branch_path: Vec<usize>,
    },
    CoilSlot {
        rung: usize,
        index: usize,
    },
    NewRung {
        index: usize,
    },
}
```

### 5.3 Canvas State & Interaction Model
In `crates/ferrite-gui/src/canvas_ld.rs`:
```rust
#[derive(Debug, Clone)]
pub struct LdCanvasState {
    pub pan: Vector,
    pub zoom: f32,
    pub selected: Option<LdElementKey>,
    pub selected_elements: Vec<LdElementKey>,
    pub drag_marquee: Option<(Point, Point)>,
    pub dragging: Option<(LdElementKey, Point)>,
    pub drop_hover: Option<LdDropTarget>,
    pub is_panning: bool,
    pub pan_start: Option<Point>,
}
```

### 5.4 Layout Geometry & Rung Bounds
The layout engine calculates the geometry of each rung:
- `RAIL_LEFT_X = 60.0`: Left power rail position.
- `RAIL_WIDTH = 5.0`: Power rail stroke.
- `SLOT_WIDTH = 96.0`: Standard horizontal spacing per contact/block card.
- `SLOT_HEIGHT = 44.0`: Card height.
- `BRANCH_VERTICAL_STEP = 54.0`: Vertical displacement per parallel branch path.
- `RUNG_GAP_Y = 32.0`: Vertical space between consecutive rungs.
- `COIL_AREA_WIDTH = 120.0`: Width reserved near right power rail for coils.

Each rung's total height is dynamically calculated based on the maximum parallel branch depth:
$$\text{Rung Height} = \text{SLOT_HEIGHT} + \sum (\text{branch paths} - 1) \times \text{BRANCH_VERTICAL_STEP} + \text{RUNG_GAP_Y}$$

### 5.5 Graphical Parallel Branches (OR Branches)
When an `LdElement::Branch { paths }` is encountered:
1. A vertical drop line descends from the main horizontal rail at the split point `split_x`.
2. Each path $p \in \text{paths}$ renders as its own horizontal sub-track at `y + p * BRANCH_VERTICAL_STEP`.
3. Elements within each sub-track are placed sequentially along that branch.
4. At the merge point `merge_x`, a vertical rise line connects all branch tracks back into the main horizontal rail.
5. In live simulation, if any path in the branch evaluates to `true` (power flows), the merge point and subsequent wire segments are energized in emerald green.

### 5.6 Messages in `crates/ferrite-gui/src/app.rs`
- `LdSelectElement(String, Option<LdElementKey>)`: Select or deselect a single element.
- `LdSelectElements(String, Vec<LdElementKey>)`: Set multi-selection (from marquee or Ctrl+Click).
- `LdDeleteSelected(String)`: Delete all elements in `selected_elements` in a single transaction.
- `LdMoveElements(String, Vec<LdElementKey>, LdDropTarget)`: Reorder or move elements to a new slot.
- `LdAddParallelBranch(String, usize, Vec<usize>)`: Convert selected contacts or insert a new parallel bypass loop.
- `LdCanvasPanChanged(String, Vector)`: Update pan offset.
- `LdCanvasZoomChanged(String, f32)`: Update zoom level.

### 5.7 Inspector & Context Bar
Located at the bottom of the LD editor view:
- When a **Contact** is selected: Tag name text input, dropdown for `NO`, `NC`, `|P|`, `|N|`, delete button.
- When a **Coil** is selected: Tag name text input, dropdown for `( )`, `[S]`, `[R]`, delete button.
- When a **Rung** is selected: Rung ID and comment editor, delete rung button.
- When **Multiple Elements** are selected: Shows badge `"N elements selected"`, bulk delete button, and bulk action helpers.

---

## 6. What's Left to the Builder
- Precise mouse click hit-testing thresholds for wire segments vs element cards.
- Rendering styling details (e.g. rounded corners, contact glyph font size, rail glow intensity).
- Unit tests verifying layout calculations, multi-selection bounds, and move/delete transactions.
