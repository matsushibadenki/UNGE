# UNGE

Reusable Rust libraries for a Tauri 2 node graph application using wgpu/WGSL.
This v0.1 implements the foundation; consult [the roadmap](ROADMAP.md) before relying on advanced features.

## Quick start

```sh
cargo test --locked
cargo run --locked -p unge-headless
cargo run --locked -p unge-tauri-host
```

The desktop sample needs the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
It uses a WebView for controls and a separate native window for GPU rendering.
The sample includes Dark/Light switching for controls and GPU rendering, with saved preferences; see [theme integration](THEMES.md#english). It offers node creation, undo/redo, dragging, box selection, port connections, panning and zoom. Node and port labels now use a Rust-owned glyph atlas with three-language host metadata; see [GPU text integration](GPU_TEXT.md#english). See [pointer integration](POINTER_INPUT.md#english) for logical coordinates and host setup.

## Reuse

Copy the whole directory into your project and reference the required crates with Cargo path dependencies.
The core and executor work without Tauri. The renderer accepts a Rust-owned texture or native surface.
`bindings/typescript` accepts an injected Tauri invoke function and includes English, Japanese and Simplified Chinese error messages.

Rust owns documents, history, viewport state and GPU buffers. WebViews send commands with an expected revision and receive small summaries.
Large images, tensors and video frames stay in host-owned resource stores; execution values carry immutable resource IDs.

Start an AI handoff with [AGENTS.md](../AGENTS.md), [AI_INTEGRATION.md](AI_INTEGRATION.md) and [ARCHITECTURE.md](ARCHITECTURE.md).
The headless example computes `20 + 22 = 42` and can save a versioned graph JSON file.
Only trusted Rust executors are supported today. WASM sandboxing, streaming, subgraphs, collaboration and browser-only hosting remain planned.
The 10,000-node / 30,000-edge / 60-FPS goal has not been benchmarked.

## Agent control with ACX

`cargo build -p unge-acx-provider` then `python3 examples/acx-provider/agent.py` runs discovery, approved editing, execution, receipt verification and recovery. The same adapter connects to the live Tauri Engine. See [ACX integration](ACX_INTEGRATION.md). The optional ACX symlink is not needed at runtime.

## Validated editing and bounded history

Definitions now describe property types, bounds, choices and defaults. Install a Registry validator in the shared Editor to validate edits, undo and redo atomically. History is bounded by total undo/redo steps and serialized bytes. All examples enable validation. See [integration and compatibility](PROPERTY_VALIDATION.md#english).

Execution caching uses exact-key hash lookup and entry/byte limits. See [cache integration](EXECUTION_CACHE.md#english) for host configuration and the pure-node contract.

Rust hosts can observe node progress and provide a future for the run deadline. See [execution progress and cancellation](EXECUTION_PROGRESS.md#english).

Share RunService between Tauri and ACX for run identity, frozen revisions, bounded progress state and host cancellation. See [execution service](EXECUTION_SERVICE.md#english).

The control window now includes run/cancel buttons and progress, with polling recovery and a notice when the document has changed since execution began.

Use `python3 examples/acx-provider/agent.py --async-run` for the optional async job lifecycle. Agents can query/cancel approved runs on the same pipe; see [the job profile](ACX_ASYNC_JOBS.md#english).

Rust contracts generate TypeScript types and JSON Schema. See [contract generation](CONTRACT_GENERATION.md) for regeneration and property value constraints.

A reproducible 10,000-node/30,000-edge CPU benchmark and median-partition BVH construction are available. See [performance measurements](PERFORMANCE.md); 60 FPS remains unverified.

Small node moves update retained render indexes incrementally across UI, pointer and ACX edits. See [index updates](INDEX_UPDATES.md).

Adaptive cubic subdivision reduces geometry for straight and distant edges. See [curve tessellation](CURVE_TESSELLATION.md) for error targets and bounds.

GPU group frames and titles follow member geometry and drag previews. See [group rendering](GROUP_RENDERING.md).

Added keyboard node selection, coordinate movement and deletion through HTML controls backed by bounded Rust semantic pages. [ACCESSIBILITY.md](ACCESSIBILITY.md)。

Group creation, rename, membership edits, deletion and member selection are available from the three-language UI. [GROUP_EDITING.md](GROUP_EDITING.md)。

Per-definition input/output value schemas and Rust validation APIs are available. [PORT_VALUE_SCHEMAS.md](PORT_VALUE_SCHEMAS.md)。
