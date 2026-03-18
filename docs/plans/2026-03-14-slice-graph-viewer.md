# Slice Graph Viewer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a static interactive frontend that visualizes slice graph JSON, groups blocks by scope, shows code in a sidebar, and highlights the first driver of the queried target signal.

**Architecture:** Add a self-contained browser viewer under a new `viewer/` directory using plain HTML, CSS, and ES module JavaScript. The viewer loads stable slice JSON produced by the existing CLI, computes a deterministic non-overlapping SVG layout grouped by `scope`, and renders interactive blocks, dashed scope containers, labeled edges, and a metadata/code sidebar. Keep Rust changes minimal and limited to documentation or optional helper fixture generation; the graph rendering logic lives entirely in the static frontend.

**Tech Stack:** HTML5, CSS3, SVG, vanilla JavaScript ES modules, existing slice JSON from `cargo run -- slice`, optional `python3 -m http.server` for local serving, Chrome DevTools for manual verification

---

## Ground Rules

- Do not modify anything under `sv-analysis/`.
- Keep the first iteration dependency-light: no React, no Vite, no Node build chain.
- Preserve the existing stable slice JSON contract; adapt the viewer to current output instead of changing backend data casually.
- Treat both static and dynamic slice JSON as supported inputs.
- Use the existing demo slice workflow and CLI fixtures before adding new backend features.
- Keep the layout deterministic so repeated renders of the same JSON produce the same block positions.

### Task 1: Create the viewer shell and input workflow

**Files:**
- Create: `viewer/index.html`
- Create: `viewer/styles.css`
- Create: `viewer/app.js`
- Create: `viewer/README.md`
- Create: `viewer/examples/.gitkeep`

**Step 1: Write the failing verification target**

Create `viewer/index.html` with placeholder regions that the future code must fill:

```html
<main class="app-shell">
  <header class="topbar"></header>
  <section class="workspace">
    <aside class="controls"></aside>
    <section class="graph-stage"></section>
    <aside class="sidebar"></aside>
  </section>
</main>
```

**Step 2: Run verification to confirm the viewer does not exist yet**

Run: `python3 -m http.server 4173 --directory viewer`
Expected: fail before files exist, or serve an empty/non-functional page.

**Step 3: Write the minimal shell implementation**

Implement:

- a top bar with title and short help text
- a left controls panel with:
  - file upload input for slice JSON
  - textarea for paste-in JSON
  - `Load Graph` button
  - `Load Demo` button
- a central SVG graph stage
- a right sidebar with placeholders for selected block metadata and code snippet

Use a split layout like:

```css
.workspace {
  display: grid;
  grid-template-columns: 320px minmax(0, 1fr) 380px;
}
```

**Step 4: Run verification to confirm the shell loads**

Run: `python3 -m http.server 4173 --directory viewer`
Expected: page loads at `http://127.0.0.1:4173/index.html` with visible control panel, empty graph canvas, and sidebar.

**Step 5: Commit**

```bash
git add viewer
git commit -m "feat: scaffold slice graph viewer shell"
```

### Task 2: Normalize slice JSON into frontend graph data

**Files:**
- Modify: `viewer/app.js`
- Create: `viewer/sample-data.js`
- Create: `viewer/examples/demo-static-slice.json`

**Step 1: Write the failing parsing check**

Add a temporary in-browser assertion helper in `viewer/app.js` that throws if a loaded graph cannot produce these derived collections:

```js
{
  nodesById,
  blocksById,
  blockNodes,
  literalNodes,
  edges,
  scopeGroups,
}
```

**Step 2: Run verification to confirm parsing fails on the placeholder app**

Run: `python3 -m http.server 4173 --directory viewer`
Expected: loading pasted slice JSON does not yet populate block cards, scope groups, or node maps.

**Step 3: Write the minimal parser implementation**

Implement a `normalizeSliceGraph(raw)` function that:

- indexes `raw.nodes` by `id`
- indexes `raw.blocks` by `block_id`
- joins block nodes with their `scope` and `block_type`
- groups block nodes by exact `scope`
- preserves dynamic `time` values when present
- preserves edge labels from `edge.signal?.name`

Use a shape like:

```js
function normalizeSliceGraph(raw) {
  const blocksById = new Map(raw.blocks.map(block => [block.id, block]))
  const nodesById = new Map(raw.nodes.map(node => [node.id, node]))
  const blockNodes = raw.nodes
    .filter(node => node.kind === 'block')
    .map(node => ({ ...node, meta: blocksById.get(node.block_id) }))
  // ...build scopeGroups and labeled edges
  return { blocksById, nodesById, blockNodes, literalNodes, edges, scopeGroups }
}
```

Generate `viewer/examples/demo-static-slice.json` from the existing CLI using the demo design so the viewer always has a checked-in sample.

Run:
`cargo run -- slice --static --sv demo/trace_coverage_demo/design.sv --sv demo/trace_coverage_demo/tb.sv --signal TOP.tb.result > viewer/examples/demo-static-slice.json`

**Step 4: Run verification to confirm normalized data loads**

Run: `python3 -m http.server 4173 --directory viewer`
Expected: `Load Demo` populates in-memory graph state without console errors.

**Step 5: Commit**

```bash
git add viewer cargo-generated-demo-command-notes
git commit -m "feat: parse stable slice json for viewer"
```

### Task 3: Render scope-aware layout with non-overlapping blocks

**Files:**
- Modify: `viewer/app.js`
- Modify: `viewer/styles.css`

**Step 1: Write the failing layout check**

Add a temporary debug overlay toggle that dumps computed positions for:

- each block node
- each scope rectangle
- each edge anchor

and assert that every block node receives a unique `(x, y)` pair.

**Step 2: Run verification to confirm there is no layout yet**

Run: `python3 -m http.server 4173 --directory viewer`
Expected: the graph stage remains blank or all nodes stack at one origin.

**Step 3: Write the minimal deterministic layout engine**

Implement a scope-first layered algorithm:

- group block nodes by `scope`
- within each scope:
  - left column: `ModInput`
  - center lanes: `Assign`, `Always`, and any other interior blocks
  - right column: `ModOutput`
- vertically stack blocks with fixed row gaps
- size each scope rect after measuring its columns
- render each scope rect with dashed borders
- place nested scopes independently, then translate child block coordinates into the global canvas

Recommended block sizing constants:

```js
const BLOCK_W = 180
const BLOCK_H = 56
const COL_GAP = 56
const ROW_GAP = 24
const SCOPE_PAD = 24
const SCOPE_GAP = 40
```

Store positions like:

```js
layout.blocks.set(blockId, { x, y, width: BLOCK_W, height: BLOCK_H })
layout.scopes.set(scopeName, { x, y, width, height })
```

**Step 4: Run verification to confirm blocks do not overlap**

Run: `python3 -m http.server 4173 --directory viewer`
Expected: blocks render without overlap, each scope is wrapped by a dashed rectangle, and `ModInput` / `ModOutput` blocks sit on the left / right edges of their scope group.

**Step 5: Commit**

```bash
git add viewer
git commit -m "feat: render scope-grouped slice graph layout"
```

### Task 4: Render links, target-driver highlighting, and block selection sidebar

**Files:**
- Modify: `viewer/app.js`
- Modify: `viewer/styles.css`

**Step 1: Write the failing interaction check**

Add a temporary assertion path so clicking a rendered block must populate:

```js
{
  blockId,
  scope,
  blockType,
  sourceFile,
  lineStart,
  lineEnd,
  codeSnippet,
}
```

**Step 2: Run verification to confirm interaction is missing**

Run: `python3 -m http.server 4173 --directory viewer`
Expected: clicking a block does nothing and edges have no labels yet.

**Step 3: Write the minimal interactive rendering**

Implement:

- SVG edges between block centers or side anchors
- edge labels centered near the edge path using `edge.signal?.name`
- a highlight style for the first block that directly drives the queried signal
- clickable block rectangles that populate the sidebar code panel
- a selected state separate from the target-driver highlight

Compute the target-driver block by scanning block-to-block or block-to-target edges whose signal name matches the requested target. If the viewer input workflow also captures the user-entered target signal, use that exact string as the primary lookup key.

Sidebar content should include:

- scope
- block type
- block id
- source file
- line range
- code snippet in a monospace panel

**Step 4: Run verification to confirm edge labels and sidebar behavior**

Run: `python3 -m http.server 4173 --directory viewer`
Expected: edges display labels like `result` in `b1 --result--> b2` style, the first direct driver of the target is visually highlighted, and clicking any block updates the sidebar code view.

**Step 5: Commit**

```bash
git add viewer
git commit -m "feat: add interactive slice graph sidebar and edge labels"
```

### Task 5: Polish responsiveness, accessibility, and documentation

**Files:**
- Modify: `viewer/index.html`
- Modify: `viewer/styles.css`
- Modify: `viewer/README.md`
- Modify: `README.md`

**Step 1: Write the failing usability checklist**

Add a short manual checklist to `viewer/README.md` that initially cannot be satisfied:

- keyboard focus visible on blocks and controls
- sidebar remains usable on laptop width
- page works at 375px, 768px, and 1440px
- reduced-motion users are not forced into animated transitions

**Step 2: Run verification to confirm polish gaps**

Run: `python3 -m http.server 4173 --directory viewer`
Expected: at least one mobile/tablet breakpoint or keyboard flow needs improvement before final polish.

**Step 3: Write the minimal polish implementation**

Implement:

- responsive layout that collapses the right sidebar below the graph on narrow screens
- visible focus rings on interactive blocks and buttons
- `prefers-reduced-motion` support for transitions
- skip link to main content
- concise docs in `viewer/README.md` for:
  - serving the viewer
  - loading CLI-generated JSON
  - interpreting scope rectangles, edge labels, highlight state, and sidebar details

Update the root `README.md` with a short “Graph Viewer” section pointing to `viewer/`.

**Step 4: Run verification to confirm viewer readiness**

Run these commands:

- `cargo run -- slice --static --sv demo/trace_coverage_demo/design.sv --sv demo/trace_coverage_demo/tb.sv --signal TOP.tb.result > viewer/examples/demo-static-slice.json`
- `cargo run -- slice --sv demo/trace_coverage_demo/design.sv --sv demo/trace_coverage_demo/tb.sv --vcd demo/trace_coverage_demo/logs/sim.vcd --signal TOP.tb.result --time 40 --min-time 0 > viewer/examples/demo-dynamic-slice.json`
- `python3 -m http.server 4173 --directory viewer`

Expected:

- both demo files load in the viewer
- static graphs render without time labels in nodes
- dynamic graphs retain time-aware block nodes where relevant
- no block overlap is visible
- scope rectangles and edge labels are legible

**Step 5: Commit**

```bash
git add viewer README.md
git commit -m "feat: document and polish slice graph viewer"
```

## Manual verification checklist

- Load `viewer/examples/demo-static-slice.json` and confirm `TOP.tb` and `TOP.tb.dut` each have dashed scope wrappers.
- Confirm `ModInput` blocks sit at the left edge of each scope group.
- Confirm `ModOutput` blocks sit at the right edge of each scope group.
- Confirm the first driver of `TOP.tb.result` is highlighted distinctly from the selected block state.
- Click at least three blocks and confirm the sidebar code snippet changes correctly.
- Inspect at least one labeled edge and confirm the signal label matches `edges[*].signal.name`.
- Resize to tablet and mobile widths and confirm the viewer remains usable.

## Notes for implementation

- The current stable slice JSON does not include full source metadata in `blocks`; if sidebar code display needs richer metadata than the graph currently exposes, add the narrowest possible backend extension and corresponding regression tests before relying on it in the viewer.
- If nested scope layout becomes too dense, first ship exact-scope grouping and deterministic placement before adding more advanced routing.
- Keep the visual language technical and precise: monospaced accents, clean dashboard palette, dashed scope borders, restrained motion, and no decorative icon clutter.

Plan complete and saved to `docs/plans/2026-03-14-slice-graph-viewer.md`. Two execution options:

1. Subagent-Driven (this session) - I dispatch fresh subagent per task, review between tasks, fast iteration

2. Parallel Session (separate) - Open new session with executing-plans, batch execution with checkpoints

Which approach?
