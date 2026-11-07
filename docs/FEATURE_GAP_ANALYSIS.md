# FerritePLC Feature Gap Analysis — vs. Professional PLC Software (2026-11-06)

Method: code-level feature inventory (all 14 crates, claims verified with file:line) compared
against a researched feature set for CODESYS 3.5, TIA Portal/S7-1500, TwinCAT 3, PLCnext,
OpenPLC/Beremiz, and the IEC 61131-3 (3rd ed.) conformance baseline. Sources cited at the end.

**Verdict:** FerritePLC is unusually strong for its size in *runtime determinism, cross-language
integration, test/scenario tooling, and Git-friendly persistence* — several of these (headless
scenario runner, digital-twin canvas, ISA-18.2 alarms, Modbus TCP client+server in one binary)
are things even mature open-source peers like OpenPLC/Beremiz do not ship. The gap is
concentrated in two areas: (1) **IEC 61131-3 data model & modularity conformance** and
(2) **live/online tooling and real communication stacks**.

---

## 1. Status matrix (35-feature landscape)

Legend: ✅ present · 🟡 partial · ❌ absent

| # | Feature | Class | FerritePLC status |
|---|---------|-------|-------------------|
| 1 | Elementary & bit-wide data types | Stakes | 🟡 Only BOOL/INT/DINT/REAL/TIME/STRING — no LREAL, no unsigned, no BYTE/WORD/DWORD/LWORD |
| 2 | Standard FBs (TON/TOF/TP/CTU/CTD/CTUD/R_TRIG/F_TRIG/SR/RS) | Stakes | ✅ + extras (BLINK, HYSTERESIS, RAMP, SCALE_LIN, P/PI/PID×3) — `ferrite-core/src/fb.rs` |
| 3 | UDT: STRUCT / ARRAY / ENUM | Stakes | ❌ none in any language (`ferrite-core/src/types.rs:10-17`) |
| 4 | Direct addressing %I/%Q/%M + symbols | Stakes | ✅ X/B/W/D widths, force masks; 🟡 no %L 64-bit, no %S/%R areas |
| 5 | Pointers / references / ADR() | Advanced | ❌ |
| 6 | OOP extensions (methods, interfaces, inheritance) | Advanced | ❌ |
| 7 | Deterministic 4-phase scan + watchdog | Stakes | ✅ global watchdog only (`ferrite-runtime/src/lib.rs:502-540`) |
| 8 | Live monitoring while running | Stakes | 🟡 GUI/force panel + TUI watch exist; no sampling of arbitrary expressions |
| 9 | Variable forcing | Stakes | ✅ force/unforce/unforce_all on %I/%Q/%M/tags |
| 10 | Online change (hot patch w/o losing state) | Advanced | ❌ recompile rebuilds runtime and resets all state (`ferrite-gui/src/app.rs` recompile_runtime) |
| 11 | Breakpoints / single-step / call stack | Advanced | 🟡 scan/POU/symbol-true breakpoints + batch stepping; no call stack or rung-level stepper |
| 12 | Real-time trace buffer (high-freq sampling) | Advanced | ❌ (trend plotter exists at GUI level; no runtime-side synchronized trace) |
| 13 | Modbus TCP client & server | Stakes | ✅ 8 FCs, MBAP, multi-client server + client with txn/poison handling |
| 14 | OPC UA server (info model + listener) | Advanced | ❌ real server: 🟡 address-space model only, no TCP binary stack/sessions |
| 15 | EtherCAT / PROFINET / EIP cyclic masters | Advanced | ❌ |
| 16 | MQTT / Sparkplug B push | Advanced | 🟡 payload/alias/COV engine; ❌ no live TCP/TLS client connection |
| 17 | HAL (GPIO) | Stakes | 🟡 Mock + legacy sysfs only; no libgpiod/uio, no SPI/I2C |
| 18 | Multi-task scheduler (cyclic/continuous/event) | Stakes | 🟡 all three kinds exist but cooperative single-thread (vs preemptive raster + per-task watchdog) |
| 19 | Hard real-time / multi-core isolation | Advanced | ❌ (by design; single-thread = determinism) |
| 20 | Retentive memory | Stakes | ✅ retain tags, %M, atomic snapshot + .bak |
| 21 | Visual editors (LD/SFC/FBD) | Stakes | ✅ all three with live power-flow/step glow; ST editor with highlighting |
| 22 | ST LSP / autocomplete / hover | Stakes | ❌ |
| 23 | Cross-language interoperability | Stakes | ✅ unified symbol table + ExecutablePou contract (stronger than most peers) |
| 24 | Topo sort + combinational loop detection | Stakes | ✅ FBD + LD (recently fixed in LD), Tarjan SCC diagnostics |
| 25 | FDT/DTM device integration | Advanced | ❌ |
| 26 | Headless scenario runner (stimulus/assert) | Stakes | ✅ `sim/scenarios.toml`, all-scenarios reporting with [FAIL] lines |
| 27 | Integrated soft-PLC simulation | Stakes | ✅ GUI sim panel + TUI + headless |
| 28 | Digital twin canvas | Advanced | ✅ 2D actuator/sensor/tank/pipe canvas with bidirectional IO binding (rare at any scale) |
| 29 | Git-friendly text serialization | Stakes | ✅ TOML manifest/symbols, JSON POUs, .st text (better than CODESYS/TIA binaries) |
| 30 | Static analysis/linting | Advanced | 🟡 compile checks + loop detection only; no unused-var/dead-reachability/naming rules |
| 31 | Headless build API / CLI | Advanced | 🟡 `ferrite check/sim/export/import` CLI exists, no JSON/CI output modes |
| 32 | Unit-test framework for POUs | Advanced | ❌ (scenarios cover integration; no per-FB assert harness like TcUnit) |
| 33 | Functional safety partition (SIL, PROFIsafe) | Advanced | ❌ |
| 34 | PLCopen motion control | Advanced | ❌ |
| 35 | Multi-axis kinematics/CNC | Advanced | ❌ |

---

## 2. What is genuinely missing (from the code inventory), grouped

### A. IEC 61131-3 conformance — the largest and most consequential gap
1. **Data types.** Only 6 elementary types (`types.rs:10-17`). Missing: LREAL, all unsigned
   (USINT/UINT/UDINT/ULINT), signed support below DINT (SINT), bit strings (BYTE/WORD/DWORD/
   LWORD), LTIME, DATE/TOD/DT, WSTRING. Ripple effect: no typed bit manipulation, no natural
   HMI/DINT comm mapping (Modbus u16/i16 reinterpretation is ad-hoc today).
2. **Derived types.** No `TYPE ... END_TYPE`, no STRUCT, no ARRAY, no ENUM. Instruction on how
   to even express "10 pumps" today: prefix-named tags. This blocks most real-world projects.
3. **Local variables & modularity.** No `VAR/END_VAR` statement blocks in ST; everything is
   global in `global_tags.toml`. No user `FUNCTION` (return-value) POUs, no authorable FBs in
   ST/LD with private state (composite FBs exist only inside FBD).
4. **Standard function coverage.** 43 functions exist; missing from the IEC table: string
   family `MID/INSERT/DELETE/REPLACE/FIND`, missing conversions (e.g. `*_TO_TIME`,
   `REAL_TO_TIME` semantics), `MOD` exists but `EXPT` arity, `BCD` conversions, conversion
   functions to/from STRING.
5. **SFC (IEC 60848):** Set/Reset variants beyond S/R (SET_SFC/RESET_SFC/FREEZE qualifiers),
   macro steps exist in schema but rejected at compile, no enclosing steps.
6. **LD:** no JMP/LBL, no MCR zones, no rung subroutines.
7. **Configuration model:** no CONFIGURATION/RESOURCE hierarchy, no per-task watchdog, no
   per-POU scheduling config beyond the single flat task loop.

### B. Live/online tooling — where everyday engineering workflows hurt
8. **Online change:** every POU edit resets the runtime (timers, active SFC steps, counters).
   In CODESYS/TIA/TwinCAT this is the definition of "would never ship". A state-preserving
   reload (patch POU bytecode, migrate symbol values by name) is realistic for FerritePLC
   since state lives in TagTable/ProcessImage, not in POU closures.
9. **True live OPC UA + MQTT.** Both crates model payloads/address spaces but have no sockets:
   no MQTT connect (although MQTT payloads/aliases/COV engineered), no OPC UA session server.
   Modbus is the only fully functional live field protocol.
10. **Trace buffer** sampled inside the scan (the GUI trend plotter samples at UI refresh rate,
    then remains alias-prone for fast scans).
11. **Call-stack/POU-level stepping and rung breakpoints** (breakpoints exist at scan/POU/symbol
    granularity only), plus **watch expressions (arbitrary expression watches)**.
12. **Per-task watchdog** (currently only global cycle watchdog `watchdog_us`).

### C. Communication breadth
13. Modbus RTU (serial RS-485) — highest-value next fieldbus for hobby/industrial alike.
14. OPC UA binary server (sessions, subscriptions/monitored items) and MQTT network client
    (with tls/rumqttc-style connection).
15. EtherCAT/CANopen/PROFINET — large effort, mark as Advanced-tier backlog, not short term.
16. libgpiod + Linux UIO for modern boards; sysfs GPIO is deprecated upstream.

### D. Engineering/quality tooling
17. **Static analysis rules** (unused tags, unreachable SFC steps beyond topology check, type-
    narrowing lint, forbidden constructs in fast tasks) — cheap, high-value; the compiler
    architecture already supports it (PouGraph/SymbolResolver).
18. **Unit-test framework for POUs** (assert FB state per simulated scan; scenarios are the
    substrate — an assert/steps TOML extension or a `#[test]`-like POU could reach TcUnit
    utility levels quickly).
19. **CLI JSON output for check/sim** (`--format json`) for CI gating; **exit codes stable API**.
20. **LSP server** (`ferrite lsp`) for ST autocomplete/hover/renames — small ecosystem payoff,
    moderate effort (compiler already has symbol + type layers to drive it).
21. **Full PLCopen TC6 graphical schema export** (currently proprietary JSON embedded in CDATA
    for LD/SFC/FBD; ST is standard). This breaks the "interoperate with other IDEs" promise
    for graphical languages.

### E. Documentation/branding nit found by inventory
22. README claims egui/eframe; actual GUI is iced 0.13. Fix copy.

---

## 3. Suggested roadmap (priority order)

**P0 — Close the conformance floor (enables everything else):**
1. Base data types: LREAL, UDINT, all bit-strings (BYTE..LWORD), STRING ops family.
2. Derived types: STRUCT/UDT + arrays + enums in TagTable and the 4 language compilers.
3. ST `VAR/END_VAR` local scoping + user FUNCTION POUs.
4. Online change (state-preserving delta recompile in ferrite-runtime).

**P1 — Make it operational in the field:**
5. Real MQTT client + real OPC UA server sessions on top of the existing engines.
6. Modbus RTU master/master-slave over serial.
7. Per-task watchdog + trace buffer sampled in scan Phase 4.
8. Static-analysis rule set + tests per POU; `ferrite check --format json`.

**P2 — Differentiators/advanced tier:**
9. LSP, full PLCopen graphical schema, CFC-style freeform FBD, CANopen/EtherCAT (large),
   safety partition concepts, motion control, multi-core task groups.

**Explicit strategic advice:**
- Do **not** chase multi-core/preemptive scheduling or safety certification next — the
  single-threaded deterministic scan is a legitimate architectural choice for this scale and
  is the reason the runtime is reproducible and easy to test. Competitors need it only for
  field-scale deployments.
- The strongest existing differentiators to keep building on: cross-language unified symbol
  model, headless scenarios, digital twin, ISA-18.2 alarms, atomic Git-friendly persistence.

## Sources
- CODESYS: helpme-codesys.com (online help), codesys.com products; Static Analysis & Test
  Manager listings on store.codesys.com.
- TIA Portal/S7: support.industry.siemens.com (SIOS), SIMATIC AX docs (ax.siemens.com).
- TwinCAT 3: infosys.beckhoff.com (TC3 overview, ADS intro).
- OpenPLC/Beremiz/MatIEC: beremiz.org/doc, gitlab.com/openplcproject/matiec, openplcproject.com.
- PLCnext: phoenixcontact.com/plcnext; engineer.plcnext.help.
- IEC 61131-3 (3rd ed.): standard functions Annex A / conformance claim requirements.
