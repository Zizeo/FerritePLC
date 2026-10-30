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
  - **Batch Delete**: Pressing `Delete` key or clicking the trash toolbar button removes all selected elements in one atomic operation, automatically closing wire gaps and pruning degenerate branches.
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

## 5. Detailed Technical Specification & Builder Calls

### 5.1 Element Addressing & Branch Path Semantics (Builder Call: Unambiguous AST Addressing)
To uniquely address elements for selection, dragging, and deletion inside arbitrarily nested rungs and parallel branches, we define unambiguous AST paths:

```rust
/// Identifies a specific parallel branch step-down within an element list:
/// `(branch_element_index, path_index)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct LdBranchPathSegment {
    pub branch_idx: usize,
    pub path_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LdElementKey {
    Rung(usize),
    Contact {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    Coil {
        rung: usize,
        index: usize,
    },
    Branch {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    InlineFb {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
}
```

### 5.2 Element Creation Workflow (Builder Call: Top Toolbar & Canvas Insertion)
Modeled after the SFC editor and industrial PLC tools (RSLogix/Codesys):
- Top toolbar provides creation buttons: `+ Rung`, `+ NO Contact`, `+ NC Contact`, `+ |P|`, `+ |N|`, `+ Coil`, `+ Parallel Branch`.
- **Insertion Rules:**
  - If a Contact or Branch is selected: New elements are inserted immediately after the selected element.
  - If a Rung is selected: New elements append to the selected rung.
  - If a Coil is selected: New coils append to the coil group of that rung.
  - If nothing is selected: Appends to the last rung in the POU (or creates Rung 0 if empty).

### 5.3 Dynamic Power Rail Geometry & Multiple Coils (Builder Call: Two-Pass Dynamic Layout)
- `RAIL_LEFT_X = 60.0`: Continuous vertical left power rail.
- `SLOT_WIDTH = 96.0`: Horizontal width per contact/block card.
- `SLOT_HEIGHT = 44.0`: Card height.
- `COIL_CARD_WIDTH = 100.0`: Width per coil card.
- `BRANCH_VERTICAL_STEP = 54.0`: Vertical displacement per parallel branch path.
- `RUNG_GAP_Y = 36.0`: Vertical gap between rungs.
- **Dynamic Right Rail (`RAIL_RIGHT_X`):**
  Calculated across all rungs in the program:
  $$\text{max\_contact\_extent} = \max_{\text{rungs}} (\text{contact count along widest branch} \times \text{SLOT\_WIDTH})$$
  $$\text{RAIL\_RIGHT\_X} = \max(650.0, \text{RAIL\_LEFT\_X} + \text{max\_contact\_extent} + \text{COIL\_CARD\_WIDTH} + 120.0)$$
- **Multiple Coils:**
  Rendered in parallel stacked vertically before the right rail. The rung height dynamically takes the maximum of the contact branch depth and the coil stack height:
  $$\text{effective\_paths} = \max(\text{contact\_branch\_depth}, \text{rung.coils.len()})$$
  $$\text{Rung Height} = \text{SLOT\_HEIGHT} + (\text{effective\_paths} - 1) \times \text{BRANCH\_VERTICAL_STEP} + \text{RUNG\_GAP\_Y}$$

### 5.4 Branch Pruning & Dissolution Rules (Builder Call: Logic-Preserving Dissolution)
During deletion (`LdDeleteSelected`) or drag-out operations:
1. **Empty Paths:** If elements are deleted from a branch path, a bare wire segment remains (representing an unconditional bypass / short circuit).
2. **Branch Dissolution:** If an entire path in a 2-path branch is deleted (leaving 1 path remaining), the branch automatically dissolves and unwraps its remaining elements back into the parent rung sequence, eliminating redundant branch structures.
3. **Empty Branch Removal:** If all paths in a branch are deleted, the branch element itself is removed from `rung.elements`.

### 5.5 Drag-and-Drop & Multi-Element Operations (Builder Call: Multi-Selection Drag & Drop)
- `LdCanvasState`:
  ```rust
  #[derive(Debug, Clone)]
  pub struct LdCanvasState {
      pub pan: Vector,
      pub zoom: f32,
      pub selected: Option<LdElementKey>,
      pub selected_elements: Vec<LdElementKey>,
      pub drag_marquee: Option<(Point, Point)>,
      pub dragging: Option<(Vec<LdElementKey>, Point)>,
      pub drop_hover: Option<LdDropTarget>,
      pub is_panning: bool,
      pub pan_start: Option<Point>,
  }
  ```
- **Dragging:** Dragging any element inside `selected_elements` drags the whole selection group. A semi-transparent ghost preview renders under the mouse.
- **Drop Target Validation:**
  - Contacts and Inline FBs drop into contact slots (`BeforeElement`, `AfterElement`, `NewBranch`).
  - Coils drop into `CoilSlot` near the right rail.
  - Incompatible drop operations are rejected cleanly without mutating the AST.

### 5.6 Canvas Actions & Messages in `ferrite-gui`
- `LdSelectElement(String, Option<LdElementKey>)`
- `LdSelectElements(String, Vec<LdElementKey>)`
- `LdDeleteSelected(String)`
- `LdMoveElements(String, Vec<LdElementKey>, LdDropTarget)`
- `LdAddParallelBranch(String, usize, Vec<LdBranchPathSegment>)`
- `LdAddContactWithKind(String, usize, ContactKind)`
- `LdAddCoilWithKind(String, usize, CoilKind)`
- `LdCanvasPanChanged(String, Vector)`
- `LdCanvasZoomChanged(String, f32)`

---

## 6. What's Left to the Builder
- Visual styling constants (colors for energized power wires: `#10B981`, inactive wires: `#334155`, selection cyan glow: `#38BDF8`).
- Drag ghost rendering and marquee rectangle dashed stroke styling.
- Comprehensive unit tests covering layout calculations, marquee hit-testing, branch dissolution, and drag-and-drop transactions.
