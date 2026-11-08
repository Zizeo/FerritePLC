# FerritePLC — Whole-Workspace Code Review (2026-11-06)

> **FIX STATUS (2026-11-06):** All 2 Critical, all 32 Important findings, and the
> release-profile guard are FIXED (via 7 parallel fix lanes; 30 files changed, 56 tests added
> — workspace now 210/210 green, clippy `-D warnings` clean). Cross-cutting core follow-ups
> fixed directly: `parse_iec_time_literal` h/d + composite durations (C14/I-6), REAL→INT/DINT
> coercion rounding unified with LD (I-2 improvement). Deferred (Improvement tier, second
> pass): GUI menu x-offsets, help-manual default scan text, theme persistence, nerd-font
> `OnceLock` cache, `"??"` icon fallback, `detect_os_language` dead helper, trend Y-scale
> windowing, TUI `step_scan` error display, compile-time address→tag index, full scan-path
> symbol interning, CI allocation gate.

Full audit of all 14 crates (~46k lines) by 6 parallel review lanes. Objective evidence at
review time: `cargo clippy --workspace --all-targets -- -D warnings` **clean**, `cargo test
--workspace` **154/154 passing**. All lanes complied with the no-`unwrap()`/`expect()` rule;
`thiserror` is used correctly in library crates (one exception: `ferrite-twin`).

Findings threshold: Critical/Important ≥ 80 confidence, Improvements ≥ 60. Dropped candidates
were adversarially verified as non-reachable and are omitted (details in lane logs).

---

## CRITICAL

### CR-1 · Modbus server: u16 address arithmetic overflow → wrong-output corruption / worker panic
**Conf 90** · `crates/ferrite-fieldbus-modbus/src/lib.rs:391,404,417,431,455,467`
All coil/register loops compute `address + (i as u16)` without bounding `address` vs `count`
(count is validated, address is not). Workspace has no `[profile.release]` `overflow-checks`:
- Debug: panic kills the server worker thread silently; all subsequent clients hang.
- Release: `WriteMultipleCoils { address: 0xFFF0, values: [16] }` wraps and **writes `%QX0.0`**
  instead of returning `IllegalDataAddress`; reads similarly alias `%MW0`.
**Fix:** validate `address + count` per FC before looping (early `IllegalDataAddress`), use
checked/u32 arithmetic. Test: `handle_request(WriteMultipleCoils { address: 0xFFF0, … })` errors
and the process image is untouched.

### CR-2 · PLCopen import silently corrupts the project on POU name collision or parse failure
**Conf 90** · `crates/ferrite-project/src/plcopen.rs:456-499` (+ `lib.rs:1220-1228`)
Importing XML whose `<pou name="batch_seq">` body is ST replaces the scaffold SFC entry but the
manifest keeps `language: Sfc, path: pous/batch_seq.sfc.json`; `save_pou` then writes an ST blob
into the sfc.json file → `Project::load` fails permanently. Independently, a POU that fails to
parse still gets its raw content written and a manifest entry pushed → unloadable project, no
import-time diagnostic.
**Fix:** derive output format from the source's actual language and reject mismatch; fail import
on unparseable POUs. Tests: import colliding-name ST POU → load succeeds; import foreign POU →
import errors.

---

## IMPORTANT (all ≥ 80 confidence)

### Ferrite-core / runtime / HAL
| # | Finding | Conf | Location |
|---|---------|------|----------|
| I-1 | `DirectAddress::byte_offset` USize overflow wraps in release → `%IW<addGroupive>` aliases `%IW0`, bounds check bypassed; debug panics via project/GUI/CLI parse | 90 | `ferrite-core/src/address.rs:83-89,157-159` |
| I-2 | `TagTable::set_tag_value` bypasses ProcessImage + coercion; write to mapped tag is silently lost; also lets NON-SFC writers overwrite `X<i>.state` (`read_only_outside_sfc` only enforced in `ScanContext`) | 85 | `ferrite-core/src/tag_table.rs:266-274` vs `context.rs:155` |
| I-3 | Word/Byte PIP accesses silently truncate/misinterpret Real/DInt/Time (write `Real 3.75` to `%IW0` stores `3`; Time > 49.7 days truncates in DWord) — no error | 85 | `ferrite-core/src/process_image.rs:250-316` |
| I-4 | HAL fail-open: sysfs read errors → input `FALSE` (inverts active-low safety inputs to "healthy"); hard GPIO-init failures silently fall back to Mock driver | 82 | `ferrite-hal/src/lib.rs:249-334` |
| I-5 | `Breakpoint::Pou` condition never checks whether the POU actually executed — fires every scan; batch stepping with Pou breakpoints is useless | 85 | `ferrite-runtime/src/lib.rs:517-524` |
| I-6 | `save_retain_snapshot` uses plain `fs::write` (non-atomic): torn write destroys the exact file retained memory protects; no fallback snapshot | 80 | `ferrite-runtime/src/lib.rs:709-719` |

### Ferrite-ir / ST / LD / FBD
| # | Finding | Conf | Location |
|---|---------|------|----------|
| I-7 | Unknown standard function (`SQRTT(x)`) or zero-arg `ABS()` passes `validate` + `compile` (typed as Real), then returns `RuntimeError` on **every scan** — validation/runtime share no function registry | 85 | `ferrite-ir/src/lib.rs:219,724-728` |
| I-8 | FBD `validate` accepts duplicate block IDs (last wins, `DuplicateBlockId` variant never constructed); `compile` silently drops one and rewires links → wrong signal | 85 | `ferrite-lang-fbd/src/lib.rs:197-204,270-286` |
| I-9 | Composite FB sub-`blocks` network never validated, never executed: loops inside composites undetected; without `st_code` the composite silently degenerates to a boolean AND of inputs | 80 | `ferrite-lang-fbd/src/lib.rs:96-106,322-341,569-638` |
| I-10 | LD combinational-loop detection **absent** — violates AGENTS.md §4.2 (`ferrite check` must detect loops in FBD/Ladder); FBD has it, LD doesn't | 85 | `ferrite-lang-ld/src/lib.rs:210-236` |
| I-11 | `InlineFb.enable_pin` not validated against `pin_type` and its `write_pin` error is swallowed (`let _ =`) → TON with `enable_pin: "EN"` silently never runs | 80 | `ferrite-lang-ld/src/lib.rs:267-321,657-660` |

### Ferrite-lang-sfc / ferrite-project
| # | Finding | Conf | Location |
|---|---------|------|----------|
| I-12 | External `[R]`/`[S]` step override (cross-POU path) stomps `prev_active` → activation/deactivation edge lost: N/L/D outputs stay stuck TRUE after external reset; P never fires on external set | 92 | `ferrite-lang-sfc/src/lib.rs:1177-1193,1311-1321,1341-1360` |
| I-13 | N/L/D qualifier with **statement** targets never reset on step exit (only BooleanSymbol is cleaned up) → behaves like S, assigns latch forever | 88 | `ferrite-lang-sfc/src/lib.rs:1334-1352` |
| I-14 | `S`/`R` re-applied every scan while step active (level-driven) — clobbers legal external writers; IEC semantics = once at activation | 82 | `ferrite-lang-sfc/src/lib.rs:1373-1392` |
| I-15 | Bare `X<i>.state`/`.t` refs bind to the **first-registered** SFC POU — in multi-SFC projects a POU's own self-reference silently reads another chart | 82 | `ferrite-core/src/tag_table.rs:172-178` |
| I-16 | Duplicate step `name` shadows another step's `id` in `step_map` → transitions silently wire to the wrong step; topology validation uses the same map and reports nothing | 80 | `ferrite-lang-sfc/src/lib.rs:927-934,362-372` |
| I-17 | Two AND-flagged transitions sharing from_steps fire simultaneously (no exclusivity check; `InvalidAlternation` validator variant is dead code) → IEC-prohibited double divergence accepted | 80 | `ferrite-lang-sfc/src/lib.rs:1260-1292,440-454` |
| I-18 | Macro steps (`StepKind::Macro` + `macro_body`) are entirely inert at runtime: no substep activation, no validation — GUI shows "MACRO(n)" while runtime diverges silently | 85 | `ferrite-lang-sfc/src/lib.rs:141,936-1031` |
| I-19 | `atomic_write_str` never `sync_all()`s the temp file before rename → power-loss can publish an empty/corrupt manifest despite temp+rename | 85 | `ferrite-project/src/lib.rs:206-227` |
| I-20 | `add_pou` doesn't validate the name (`"fo:o"`, `"../evil"`) and saves the manifest **before** the POU file → failed write leaves an unloadable project; traversal writes outside `pous/` | 90 | `ferrite-project/src/lib.rs:1262-1320` |
| I-21 | `rename_pou` doesn't rewrite cross-POU qualified symbol references (`old.X<i>.state` in other POUs) → project left broken after rename, only discoverable via `ferrite check` | 85 | `ferrite-project/src/lib.rs:1323-1402` |

### Ferrite-gui
| # | Finding | Conf | Location |
|---|---------|------|----------|
| I-22 | `SelectTab` doesn't clear per-editor selections → switching tabs keeps a stale selection and `Delete` mutates the **wrong POU'**s elements | 90 | `ferrite-gui/src/app.rs` `SelectTab` ~1669, `DeletePressed` ~2833 |
| I-23 | `st_contents` not resynced after project load / PLCopen import → ST editor shows the previous project's source; first keystroke corrupts the new project in memory | 85 | `app.rs:2341,2460; sync_st_contents @882` |
| I-24 | SFC element IDs computed as `len()+1` → after any deletion, new steps/transitions collide with existing IDs → transitions wire into the wrong topology | 85 | `app.rs` `SfcAddTransition` ~3876, `SfcConnectTransitions` ~4707 |
| I-25 | `SavePou` clears `modified` while ignoring unsaved tag-table/FB-instance/manifest edits → closing silently loses those edits | 85 | `app.rs:2321-2334` + `ferrite-project` `save_pou` |
| I-26 | `ConfirmCloseSaveAndExit` discards the save error (`let _ = …save()`) and exits regardless → all edits lost under IO failure | 85 | `app.rs` ~2065 |
| I-27 | FBD "+ Add" names blocks `TON_{count+1}` with no duplicate check after deletes → two blocks share one ID and one runtime FB instance; wrong block mutated downstream | 90 | `ferrite-gui/src/fbd_library_window.rs:602-608` + `app.rs:4836-5046` |
| I-28 | Numerically-bound text fields re-format on every keystroke → `0.5`/fractions can't be typed, backspace can't traverse formatted decimals | 80 | `ferrite-gui/src/editor_symbols.rs:105-348(266-306)` |

### Fieldbus / twin / CLI
| # | Finding | Conf | Location |
|---|---------|------|----------|
| I-29 | Modbus server has no frame accumulator: coalesced 2nd request silently dropped (client timeout); fragmented PDU desyncs the stream (garbage exceptions); > 1024-byte frames truncated | 85 | `ferrite-fieldbus-modbus/src/lib.rs:661-731` |
| I-30 | Headless sim: first failing scenario aborts the whole run (`?` propagates) — remaining scenarios never run, no `[FAIL]` summary | 82 | `ferrite-cli/src/main.rs:294-305` |
| I-31 | Twin `blink.interval_ms == 0` from JSON → division-by-zero panic on first render (app crash) | 82 | `ferrite-twin/src/workspace.rs:576` |
| I-32 | Malformed `digital_twin.json` silently replaced by starter plant → user's canvas permanently overwritten on next save | 80 | `ferrite-twin/src/workspace.rs:1065-1079` |

---

## Theme: heap allocations in the scan hot path (AGENTS.md §5.1 violation)

Found independently in three lanes; one systemic fix:
- `ferrite-core/src/context.rs:141-153` — `write_tag_id` clones `tag.name` per tagged write (error path needs it; success path doesn't). Reached from every language's assignment path, and 4x per step per scan from SFC introspection.
- `ferrite-ir/src/lib.rs:164,941-954,963-985,1153-1159` — `to_ascii_uppercase()` + O(env) scan per symbol read; fresh HashMap env per `execute_block`; per-FB-call `format!`s.
- `ferrite-lang-fbd/src/lib.rs:476,569-585,641-647` — `kind.clone()`, fresh env HashMap, `target_sym.clone()` per block per scan.
- `ferrite-core/src/context.rs:213-224` — `write_symbol` by address scans the entire TagTable O(N) per write.

**Fix direction:** intern symbols to indices at compile time, pre-resolve TagIds, reuse scratch
buffers, clone names only in error branches. Regression test: counting `GlobalAlloc` asserting
zero allocations for a representative `execute()` after warmup. Make it a CI gate.

---

## IMPROVEMENTS (≥ 60 confidence)

### Core / runtime / HAL
- **Cyclic scheduler discards overshoot** (`elapsed_ms = 0` on fire): interval-15/dt-10 cadence runs every 20ms forever; use `elapsed_ms -= interval`. *runtime:444-451* (70)
- **Warm restart resets retain-flagged INPUT tags** (branch order wrong). *runtime:365-375* (65)
- **`from_project`/`cold_restart`/`warm_restart` swallow init errors** (`let _ =`) — misconfigured addresses invisible. *runtime:123-126,321-372* (75)
- **Alarm journal `Vec::remove(0)` ring buffer** O(n) per event past cap; cap is silent/undeclared; use `VecDeque`. *runtime:663-665* (70)
- **HAL sysfs re-opens files per pin per scan** (no cached FDs) and config `pull` setting is loaded but never applied — blows cycle budget at high scan rates. *hal:249-334* (60)
- **Dead code:** `IecValue::coerce_to` String→String arm parses a time literal and discards it. *types.rs:156-161* (65)

### IR / language crates
- **FBD arity not validated:** binops with <2 inputs silently evaluate `false` forever; `Not` uses only first input. *fbd:515-530,479-486* (75)
- **FBD questionable output-pin names** map to `PinSlot::Named` silently; missing required pins (TON without IN) run with defaults silently. *fbd:380-391,428-435,532-567* (70)
- **Truncation vs rounding inconsistency:** LD Move/`coerce_to` truncates REAL→INT, `REAL_TO_INT` rounds. Unify on IEC round-to-nearest. *ld:717-723; types.rs:144-148; ir:622-640* (72)
- **`MAX_LOOP_ITERATIONS = 10_000` aborts legitimate 12k-iteration FOR loops; `step_val == 0` reports misleading `IterationLimitExceeded`.** Make cap configurable. *ir:10,1048-1062* (72)
- **LD Compare operands not type-checked:** BOOL tag vs literal `50` silently evaluates as `1 >= 50`. Add compatibility check for BOOL-vs-numeric. *ld:322-333; ir:787-798* (70)
- **ST time literals lack `h`/`d` units** (`T#1h` rejected) and `parse_iec_time_literal` rejects composite `T#1m30s`. *st:96; types.rs:250-277* (65)
- **LD Div with omitted input2 passes validation** (validated as "0") then `DivisionByZero` every scan when powered. *ld:369,704-714* (66)
- **MUX clamps out-of-range K** to the last input instead of erroring. *ir:562-564* (68)
- **CASE duplicate labels not detected** — first matching arm silently shadows later arms. *ir:1218-1240* (66)
- **`PouGraph` cycle diagnostic lists non-cycle nodes** (every unsorted node); use SCC for the error payload. *ir:1504-1515* (62)
- **Unbounded parser recursion** on deeply nested ST expressions → stack-overflow abort during `ferrite check`; add a depth counter. *st:674-736* (60)

### SFC / project
- **F/FZ actions with statement targets compile but are silent no-ops** (statement arm absent). *sfc:1393-1402* (75)
- **Wired-but-unreachable steps not detected** — reachability is undirected BFS, not directed-from-initial. *sfc:489-566* (65)
- **`Project::load` doesn't sandbox manifest POU paths** (`pous/../../…` reads arbitrary files). Canonicalize under root. *project:1165-1186* (65)
- **Swallowed errors in maintenance ops:** `reorder_pou` ignores `save_manifest` result; `delete_pou` ignores `remove_file` failure (orphan files); `save()` silently skips POUs absent from the map. *project:1471-1494,1252-1256* (62)
- **PLCopen type mapping collapses BYTE/WORD into INT** — round-trip loses width fidelity. *plcopen:340-363* (62)

### GUI
- **Tag/FB-instance/scan-config edits never trigger `recompile_runtime`** — I/O panel stale until an unrelated POU edit. *app.rs TagNameChanged etc.* (70)
- **Custom FBD block save/delete errors ignored** (`let _ = save_global_library`). *app.rs* (70)
- **Rename in custom FBD builder can silently overwrite a differently-named block** (`b.name == target_name || b.name == name` conflation). *app.rs:1901* (65)
- **Menus positioned by hardcoded pixel `x_offset`s** — French dropdowns open under the wrong header. *app.rs ~6666* (65)
- **Alarm panel headers not i18n'd** ("Severity/ID/Tag/…" bypass `lang.tr`). *alarms_panel.rs* (65)
- **Twin context menu clamped to fixed 820/520 px** canvas assumptions — menu can render partly offscreen; clamp against real canvas size. *digital_twin.rs:984-985* (65)
- **Theme choice not persisted** (language is; theme is not — restart resets to Dark). *settings.rs:19-22* (65)
- **Help manual states wrong default scan interval** (10 ms; scaffold default is 50 ms). *help_window.rs:692,1346* (75)
- **Nerd-font check runs under write lock per icon per frame** — cache the once-constant result in `OnceLock`. *icons.rs:5-82* (70)
- **Save icon fallback renders literal `"??"`** (siblings use real Unicode glyphs). *icons.rs:72* (65)
- **`detect_os_language()` dead helper with stale doc** contradicting startup behavior. *i18n.rs:68-85* (60)
- **Trend Y-autoscale includes points outside the visible window** — a spike half a window ago compresses the current trace. *trend_plotter.rs:190-205* (60)

### Fieldbus / twin / CLI
- **Modbus client ignores `transaction_id`** in responses — accepts stale/reordered replies as current. *modbus:828-846* (68)
- **Modbus client connection permanently desynced after mid-read timeout** — no poisoned-connection state; next request reads garbage. *modbus:796,828-833* (62)
- **Modbus server: junk ≥7-byte reads inflate `requests_total`; unbounded client set, no idle disconnect** — stats and DoS surface. *modbus:668-669,643-654* (60)
- **MQTT COV alias = positional tag index** — aliases shift when the tag set changes between BIRTH and COV, corrupting Sparkplug B consumers. Use stable per-tag IDs. *iot/mqtt.rs:86-128* (60)
- **OPC UA `messageSize` header field doesn't match encoded length** (declares 32/28, actually 52) and endpoint URL not length-prefixed per spec. *iot/opcua.rs:187-215* (65)
- **TUI sim silently discards `step_scan` errors** (`let _ =`) — shows RUNNING with no failure indicator; render last error and stop on repeated failure. *cli:473-476,509-510* (65)
- **`ferrite-twin` errors as `Result<(), String>`** while declaring an unused `thiserror` dep — define a `TwinError` enum (pairs with I-32). *twin/workspace.rs:1082-1091* (60)

### Cross-cutting
- **Test coverage is thin where it matters:** 154 workspace tests, but `ferrite-cli` has **0** tests, `ferrite-ir` and `ferrite-hal` 1 each; no crate has integration tests (`tests/` dirs absent everywhere). Every Important finding above corresponds to an untested path.
- **No `[profile.release]` guard:** add `overflow-checks = true` (at minimum for this PLC domain — wrapping silently redirected real outputs, see CR-1/I-1) and consider `debug-assertions`/`panic = "abort"` tradeoff explicitly.
- **No allocation-instrumented or release-mode regression tests** despite the real-time no-alloc rule.
