<!--
Cited: graycart-gb & graycart-gba Machine Monitor & egui shell
  (graycart-gba/src/frontend/shell/debug/ui/{mod,graphs,panels,widgets,health,header}.rs,
   graycart-gba/src/frontend/shell/fonts.rs, graycart-gba/src/frontend/shell/ui/mod.rs);
  Graycart N64 Development Plan (docs/DEVELOPMENT-PLAN.md §1, §3.6, M5/M6/M11);
  N64brew Wiki (VR4300, RSP, RDP, VI, AI, MI/PI/SI/RI)
Note: Visual design, telemetry, live graphs, and debugger architecture for graycart-n64,
  aligned directly with the graycart-gb and graycart-gba family aesthetic.
-->

# graycart-n64 — Debugger, Telemetry & UI Architecture

**Audience:** Dave + implementers.  
**Design Precedent:** `graycart-gb` (`v0.11.5+`) and `graycart-gba` (`0.2.8+`, `greenfield`).  
**Library Boundary:** Core crate (`graycart-n64` default features) stays **100% zero-GUI** so headless CI (`cargo test`) compiles fast and requires zero windowing/audio system libraries. All desktop UI, graphs, and interactive inspectors live behind an explicit opt-in `frontend` Cargo feature (matching `graycart-gb` and `graycart-gba`).

---

## 1. Graycart Family Aesthetic & Tech Stack

We match `graycart-gb` and `graycart-gba` verbatim so every console in the Graycart family looks and feels cohesive:

| Layer | Family Standard (`graycart-gb` / `graycart-gba`) | `graycart-n64` Adoption |
|---|---|---|
| **Typeface** | **Departure Mono** (`assets/fonts/DepartureMono-Regular.otf`) installed as primary Proportional & Monospace face across all egui surfaces | Copy `DepartureMono-Regular.otf` + `LICENSE-DepartureMono.txt` into `assets/fonts/` when `frontend` feature lands |
| **UI & Presenter** | `egui` + `egui-wgpu` + `egui-winit` + `winit` + `pixels` (`wgpu`) | Same stack, with `EdgeWalker` Vulkan/wgpu compute sharing the `wgpu` device seamlessly |
| **Audio / Input / Utility** | `cpal`, `rtrb`, `gilrs`, `rfd`, `png`, `arboard`, `pollster` | Same stack, adding true N64 ~80-unit octagonal analog stick calibration & 4-port controller + pak bindings |
| **Color Palette (Machine Monitor)** | Dark carbon panel fill (`rgb(18, 20, 22)`), graph tracks (`gray(20)` / `gray(35)`), sage-teal card headers (`rgb(140, 180, 160)`), title (`rgb(180, 220, 200)`), crisp `2.0` corner radius, fixed-width cells (zero digit jitter) | Identical palette & severity scale: Good `rgb(80, 200, 120)`, Warn `rgb(220, 180, 60)`, Bad `rgb(220, 80, 80)`, Info `rgb(100, 160, 220)` |
| **Hotkeys** | `F12` Machine Monitor toggle · `F9` rolling diagnostic capture (`-15s .. +5s`) · `F6` clean framebuffer screenshot (excluding playback overlay) | Identical hotkeys + single-step (`F10` / `F11`) and Oracle Lens split toggle |

---

## 2. Machine Monitor (`btop`-Inspired Full-System Debugger)

Every subsystem in `graycart-n64` exposes a lightweight, read-only `DebugSnapshot` / `TelemetrySample` struct from the core (zero GUI dependency in core) so both the CLI progress runner and the `egui` Machine Monitor read the exact same ground truth.

### 2.1 Fixed Header & Capture Toolbar
- **Status Banner:** `BUILD DEBUG · not representative of play performance` in debug builds; in release builds: `BUILD RELEASE | REALTIME 100.0% OK | AUDIO OK | RDP EDGEWALKER`.
- **Title Row:** `Graycart // Machine Monitor` · live `HOST FPS` · `● RUNNING / IDLE` · ROM Title + Region + **CIC badge** (`CIC-6102`, `CIC-6105`, etc.) + `N64 8MiB`.
- **Capture Toolbar:** `CAPTURE F9` (rolling `-15s .. +5s` history bundle), `COPY SNAPSHOT` (instant state dump), and `CAPTURE READY [COPY] [SAVE…] [VIEW]` expandable inline report viewer.

### 2.2 Fixed-Width `OVERVIEW` Health Strip
Zero layout jitter across changing digits:
`REALTIME %` · `EMU FPS` · `AUDIO` (`OK/WARN/BAD/OFF`) · `QUEUE %` · `FRAME P99 (ms)` · `RCP / RDP (ms)`

### 2.3 Live Rolling Graphs Card (`GRAPHS` — 30 s @ 15 Hz Ring Buffers)

Matches `graycart-gba`'s `Sparkline` ring buffer (`450` samples = 30 s @ 15 Hz), expanded for full N64 visibility across **CPU, Memory/Bus, GPU/RCP, and Audio**:

| Graph Group | Sparklines / Charts | Color / Scale | What It Catches Immediately |
|---|---|---|---|
| **CPU Graphs** | 1. **VR4300 Effective MHz / Realtime %** (`93.75 MHz` target)<br/>2. **Frame Time (ms)** (avg & P99 spike overlay, `16.67 ms` NTSC line)<br/>3. **L1 Cache Hit Rate %** (I-cache & D-cache dual trace)<br/>4. **Exceptions / TLB Misses / IRQs per Frame** | Mint `rgb(120, 220, 140)`<br/>Amber `rgb(200, 160, 80)`<br/>Teal `rgb(100, 200, 200)`<br/>Coral `rgb(220, 110, 100)` | Spin-loops, cache thrashing, runaway TLB refill storms, missing IRQ acks |
| **Memory & Bus Graphs** | 1. **Total RDRAM Bandwidth (MB/frame)**<br/>2. **DMA Channel Activity** (PI / SI / SP / AI / DP bytes/frame stacked bars + sparkline)<br/>3. **SysAD Stall / Open-Bank / Uncached-RDRAM Access Rate** | Cyan `rgb(90, 190, 230)`<br/>Per-channel legend<br/>Gold `rgb(220, 180, 60)` | PI DMA stalls, uncached tight loops, cache/DMA coherence bugs |
| **GPU / RCP Graphs** | 1. **RSP Utilization %** (SU vs VU active cycles/frame)<br/>2. **RDP Throughput** (commands/frame & pixels rasterized/frame)<br/>3. **Renderer Time (ms)** (`CpuRdpLle` vs `EdgeWalker` GPU compute pass + tile bin occupancy)<br/>4. **VI Scanline / Field Cadence** (`VI_V_CURRENT`, `VI_V_SYNC` period) | Violet `rgb(175, 130, 240)`<br/>Magenta `rgb(220, 110, 180)`<br/>Emerald `rgb(80, 210, 140)`<br/>Sky `rgb(100, 160, 255)` | Microcode hangs, RDP FIFO stalls, tile hotspots, VI timing drift |
| **Audio Graphs** | 1. **AI DMA Queue Fill Bar + Rolling Sparkline** (target buffer line)<br/>2. **L / R Stereo Peak & Clipping Meters** | Blue `rgb(100, 160, 255)`<br/>Green→Amber→Red peak | AI underruns, double-buffer length latch bugs, RSP audio mixer drift |

### 2.4 Responsive 1–3 Column Collapsible Inspector Panels

Same `column_count(width)` grid (`>=980px`: 3 cols, `>=640px`: 2 cols, `<640px`: 1 col) as `graycart-gba`:

1. **`PERFORMANCE`**: Cycles/s vs `93.75M` CPU & `62.5M` RCP, realtime %, emu FPS, frame avg/min/max/P99, render ms, pace sleep ms, missed frames.
2. **`PROFILE` (Avg / Max Toggle)**: Per-subsystem host-time breakdown with horizontal percentage bars and hover tooltips: `VR4300`, `MMU/Cache`, `Bus/DMA`, `RSP SU`, `RSP VU`, `RDP / EdgeWalker`, `VI Scanout`, `AI Audio`, `Frontend/UI`, plus `TOTAL / 16.67 ms`.
3. **`VR4300 CPU`**: All 32 64-bit GPRs (ABI names `$zero..$ra`, highlight modified registers since last step), `PC`, `HI`, `LO`, delay-slot state, `LLBit`/`LLAddr`, **COP0** (`Status`, `Cause`, `Count`, `Compare`, `EPC`, `BadVAddr`, `Index/Random/EntryHi/EntryLo0/1/PageMask`, `Config`), **COP1 FPU** (32× `f0..f31` in `.S`/`.D`/hex + `FCR31` rounding/flags), live scrolling **MIPS III Disassembler**, PC/watchpoint breakpoints, step-in / step-over / run-to-frame.
4. **`TLB & MMU`**: 32-entry joint TLB table (`VPN2`, `ASID`, `G`, `PageMask`, `PFN0/1`, `C/D/V` flags) + interactive VA→PA translator showing segment (`KUSEG`/`KSEG0`/`KSEG1`/`KSSEG`/`KSEG3`) and `CacheAttr`.
5. **`MEMORY / BUS / DMA / ISVIEWER`**:
   - Interactive Hex + ASCII viewer for **RDRAM** (4/8 MiB + optional **9th-bit overlay**), **RSP DMEM/IMEM**, **PIF RAM**, and **Cart ROM**.
   - Live register panels for **MI** (`INTR`/`MASK` bit badges), **PI** (active DMA, BSD domain timings), **SI**, **RI** (`RI_SELECT`), and live scrolling **ISViewer 64 Console** (essential for watching `n64-systemtest` / `nemu64-test` / `libdragon` logs in real time!).
6. **`RSP (SU + VU)`**: PC, `SP_STATUS` flags (`HALT`, `BROKE`, `DMA_BUSY`, `SIG0..7`), scalar `$r0..$r31`, 32× 128-bit vector registers (`v0..v31`, 8×16-bit lanes), 48-bit `ACC` (Hi/Mid/Lo), `VCO`/`VCC`/`VCE` flags, live IMEM disassembler.
7. **`RDP / EDGEWALKER / VI` (Live Frame Inspector & Oracle Lens)**:
   - Draw-call / command scrubber across the current display list.
   - Live **4 KiB TMEM texture viewer** (decoded by active tile descriptor format/size/TLUT).
   - **9th-bit coverage visualizer** & **16-bit Z-buffer heatmap**.
   - Human-readable **Color Combiner** `(A - B) * C + D` & **Blender** equation decoder.
   - **Oracle Lens**: Live split-screen slider or pixel-diff heatmap comparing `EdgeWalker` vs `CpuRdpLle` (or Native 1× vs Enhanced 4×).
   - **VI panel**: `VI_CTRL`, `VI_ORIGIN`, `VI_WIDTH`, `VI_V_SYNC`, `VI_V_INTR`, `VI_V_CURRENT`, active filter flags (AA, divot, gamma, dither).
8. **`AUDIO (AI)`**: Double-buffer DMA queue slots (`AI_DRAM_ADDR`, `AI_LEN`, `AI_STATUS`), `AI_DACRATE` / computed sample rate, host stream rate/device/callbacks/underruns, L/R peak meters.
9. **`INPUT & PAKS`**: 4 controller ports, live button matrix, **2D octagonal analog stick plot** (raw host vs calibrated ~80-unit N64 octagon), attached Pak badge (`None` / `Controller Pak` / `Rumble Pak` / `Transfer Pak` + GB cart link status).

---

## 3. Core-First Instrumentation Plan (Active From M1)

We do **not** wait until M9/M11 to get visibility. Debugging hooks grow alongside each milestone:

| Milestone | Core Instrumentation (Zero-GUI) | Developer Visibility |
|---|---|---|
| **M1 (CPU)** | `CpuDebugSnapshot` (GPRs, PC, HI/LO, COP0, COP1, TLB, instruction/exception counters), ring-buffer instruction trace (`--trace`), MIPS III disassembler | Step-by-step **Progress Board** against Dillonb (26 ROMs), `n64-systemtest` / `nemu64-test`, and local `carts/n64/` probe; instant trap PC + decoded instruction + register dump |
| **M2 (Bus + Boot)** | Bus/DMA byte & stall counters per channel (PI/SI/SP/AI), MI IRQ history, live ISViewer stream | Full `n64-systemtest` / `nemu64-test` category scoreboards + `n64memtest` + `n64-systemcrash` |
| **M3–M4 (RSP)** | `RspDebugSnapshot` (SU/VU regs, 48-bit ACC, DMA counters, IMEM disassembler) | RSP cycle share & vector lane diff dumps |
| **M5–M6 (RDP + `EdgeWalker`)** | RDP command log, TMEM/coverage/Z dumpers, `EdgeWalker` vs `CpuRdpLle` frame-hash comparator | First `frontend` egui window + **Machine Monitor** with CPU/Memory/GPU/Audio graphs + Live RDP Inspector + Oracle Lens |
| **M7–M11** | AI audio telemetry + full `F9` capture bundles + M11 enhancements | Full production `Graycart // Machine Monitor` matching `graycart-gb` & `graycart-gba` |

---

## 4. Per-Step Progress Reporting Contract

After **every milestone step or worker wave**, the Orchestrator runs and reports a concise **Progress Board** containing:

1. **Workspace CI Gate (ROM-free):** `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test` exact counts.
2. **N64 Conformance Fixtures (`tests/fixtures/fetched/`):** Exact pass/fail/trap counts across every fetched suite applicable to the current milestone (starting with Dillonb 26/26 in M1, adding `n64memtest`, `n64-systemtest` / `nemu64-test`, `PeterLemon/N64`, and `n64-systemcrash` as subsystems come online).
3. **Local Real-ROM Smoke Probe (`C:\Users\axolatl-tank\Projects\graycart\carts\n64`, local-only, never committed):** Automated short-run check across local `.z64` carts reporting CIC, boot stage reached, cycles executed, unique PCs / instructions executed, ISViewer/framebuffer hash, and the exact next `Trap` encountered so we visibly watch real games march deeper into IPL3 → OS init → RSP/RDP → title screen at every step.
