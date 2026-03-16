import { DEMO_SLICE_LABEL, loadDemoSliceText } from "./sample-data.js"
import {
  applyLayoutOverrides,
  buildArrowHead,
  buildCurvedPath,
  buildPolylinePath,
  computeEdgeRoutes,
  computeLayout as computeGraphLayout,
  edgeLabelPosition as computeEdgeLabelPosition,
} from "./graph-core.js"

const SVG_NS = "http://www.w3.org/2000/svg"
const BLOCK_W = 180
const BLOCK_H = 56
const COL_GAP = 56
const ROW_GAP = 24
const SCOPE_PAD = 24
const SCOPE_GAP = 40
const SCOPE_TITLE_H = 28
const STAGE_PAD = 24
const DEBUG_STORAGE_KEY = "slice-viewer-debug-layout"

const elements = {
  fileInput: document.querySelector("#file-input"),
  jsonInput: document.querySelector("#json-input"),
  loadGraphButton: document.querySelector("#load-graph"),
  loadDemoButton: document.querySelector("#load-demo"),
  statusCard: document.querySelector("#status-card"),
  statusMessage: document.querySelector("#status-message"),
  statusAlert: document.querySelector("#status-alert"),
  metaScope: document.querySelector("#meta-scope"),
  metaType: document.querySelector("#meta-type"),
  metaId: document.querySelector("#meta-id"),
  metadataList: document.querySelector(".metadata-list"),
  codeSnippet: document.querySelector("#code-snippet"),
  graphCanvas: document.querySelector("#graph-canvas"),
  graphHeader: document.querySelector(".graph-header"),
  graphSurface: document.querySelector(".graph-surface"),
}

const sidebarFields = {
  sourceFile: null,
  lineRange: null,
}

const appState = {
  source: null,
  rawGraph: null,
  normalizedGraph: null,
  baseLayout: null,
  layout: null,
  selectedBlockId: null,
  selectedEdgeId: null,
  targetSignal: null,
  targetDriverBlockId: null,
  blockOffsets: new Map(),
  scopeSizeAdjustments: new Map(),
  debugVisible: readStoredDebugPreference(),
}

let debugToggleButton = null
let debugOverlay = null
let activePointerAction = null
let suppressNextBlockClick = false

function setStatus(message, tone = "info") {
  if (elements.statusCard) {
    elements.statusCard.dataset.tone = tone
  }
  elements.statusMessage.textContent = message
  elements.statusMessage.dataset.tone = tone

  if (elements.statusAlert) {
    if (tone === "error") {
      elements.statusAlert.hidden = false
      elements.statusAlert.textContent = message
    } else {
      elements.statusAlert.hidden = true
      elements.statusAlert.textContent = ""
    }
  }
}

function resetSelectionPlaceholders() {
  elements.metaScope.textContent = "Not selected"
  elements.metaType.textContent = "Not selected"
  elements.metaId.textContent = "Not selected"
  if (sidebarFields.sourceFile) {
    sidebarFields.sourceFile.textContent = "Not selected"
  }
  if (sidebarFields.lineRange) {
    sidebarFields.lineRange.textContent = "Not selected"
  }
  elements.codeSnippet.textContent = "// Select a block to inspect its source metadata and snippet."
}

function ensureSidebarField(id, label) {
  if (!elements.metadataList) {
    return null
  }

  let value = document.querySelector(`#${id}`)
  if (value) {
    return value
  }

  const row = document.createElement("div")
  const term = document.createElement("dt")
  term.textContent = label
  value = document.createElement("dd")
  value.id = id
  value.textContent = "Not selected"
  row.append(term, value)
  elements.metadataList.append(row)
  return value
}

function getSignalLeafName(signalName) {
  if (typeof signalName !== "string" || !signalName) {
    return null
  }

  const segments = signalName.split(".")
  return segments[segments.length - 1] ?? null
}

function signalMatchesTarget(signalName, targetSignal) {
  if (typeof signalName !== "string" || !signalName || typeof targetSignal !== "string" || !targetSignal) {
    return false
  }

  if (signalName === targetSignal) {
    return true
  }

  const signalLeaf = getSignalLeafName(signalName)
  const targetLeaf = getSignalLeafName(targetSignal)
  return Boolean(signalLeaf && targetLeaf && signalLeaf === targetLeaf)
}

function computeTargetDriverBlockId(graph, targetSignal) {
  if (!graph || !targetSignal) {
    return null
  }

  const pickFirstDriverNode = (matches) => {
    if (matches.length === 0) {
      return null
    }
    if (matches.length === 1) {
      return matches[0].fromNode.id
    }

    const fromIds = new Set(matches.map((edge) => edge.fromNode.id))
    const sinkMatches = matches.filter((edge) => !fromIds.has(edge.toNode.id))
    if (sinkMatches.length === 1) {
      return sinkMatches[0].fromNode.id
    }
    if (sinkMatches.length > 1) {
      matches = sinkMatches
    }

    const sameDriver = matches.every((edge) => edge.fromNode.id === matches[0].fromNode.id)
    if (sameDriver) {
      return matches[0].fromNode.id
    }

    const timedMatches = matches.filter((edge) => typeof edge.fromNode.time === "number")
    if (timedMatches.length > 0) {
      const earliestTime = Math.min(...timedMatches.map((edge) => edge.fromNode.time))
      const earliestMatches = timedMatches.filter((edge) => edge.fromNode.time === earliestTime)
      if (earliestMatches.length === 1) {
        return earliestMatches[0].fromNode.id
      }
      if (earliestMatches.every((edge) => edge.fromNode.id === earliestMatches[0].fromNode.id)) {
        return earliestMatches[0].fromNode.id
      }
    }

    return null
  }

  const blockEdges = graph.edges.filter((edge) => edge.fromNode?.kind === "block")
  const exactMatches = blockEdges.filter((edge) => (edge.signal?.name ?? edge.label ?? null) === targetSignal)
  const exactDriver = pickFirstDriverNode(exactMatches)
  if (exactDriver !== null) {
    return exactDriver
  }

  const targetLeaf = getSignalLeafName(targetSignal)
  if (!targetLeaf) {
    return null
  }

  const leafMatches = blockEdges.filter((edge) => {
    const signalName = edge.signal?.name ?? edge.label ?? null
    return getSignalLeafName(signalName) === targetLeaf
  })

  return pickFirstDriverNode(leafMatches)
}

function pickSourceFields(blockNode) {
  const meta = blockNode?.meta ?? {}
  const source = typeof meta.source === "object" && meta.source !== null ? meta.source : {}
  const lineInfo = typeof meta.lines === "object" && meta.lines !== null ? meta.lines : {}

  const sourceFile =
    meta.source_file ??
    meta.file ??
    source.file ??
    source.path ??
    source.filename ??
    null

  const lineStart =
    meta.line_start ??
    meta.start_line ??
    lineInfo.start ??
    source.line_start ??
    source.start_line ??
    null

  const lineEnd =
    meta.line_end ??
    meta.end_line ??
    lineInfo.end ??
    source.line_end ??
    source.end_line ??
    lineStart ??
    null

  const codeSnippet =
    meta.code ??
    meta.code_snippet ??
    meta.snippet ??
    source.code ??
    source.snippet ??
    null

  return {
    sourceFile,
    lineStart,
    lineEnd,
    codeSnippet,
  }
}

function buildSelectionDetails(blockNode) {
  const sourceFields = pickSourceFields(blockNode)
  const missingFields = []
  if (!sourceFields.sourceFile) {
    missingFields.push("source file")
  }
  if (sourceFields.lineStart === null && sourceFields.lineEnd === null) {
    missingFields.push("line range")
  }
  if (!sourceFields.codeSnippet) {
    missingFields.push("code snippet")
  }
  const hasSourceMetadata = missingFields.length < 3
  const fallbackSnippet = [
    `// No code snippet is present for block ${blockNode.id}.`,
    `// Missing fields in the current slice JSON: ${missingFields.join(", ") || "none"}.`,
    "// Add the missing block source fields to populate this panel more completely.",
  ].join("\n")

  return {
    blockId: `b${blockNode.meta?.id ?? blockNode.id}`,
    scope: blockNode.scope,
    blockType: blockNode.block_type,
    sourceFile: sourceFields.sourceFile ?? "Unavailable in current slice JSON",
    lineStart: sourceFields.lineStart,
    lineEnd: sourceFields.lineEnd,
    lineRange:
      sourceFields.lineStart !== null && sourceFields.lineEnd !== null
        ? `${sourceFields.lineStart}-${sourceFields.lineEnd}`
        : sourceFields.lineStart !== null
          ? String(sourceFields.lineStart)
          : "Unavailable in current slice JSON",
    codeSnippet: sourceFields.codeSnippet ?? fallbackSnippet,
    hasSourceMetadata,
  }
}

function assertSelectionDetails(details) {
  const requiredKeys = [
    "blockId",
    "scope",
    "blockType",
    "sourceFile",
    "lineStart",
    "lineEnd",
    "codeSnippet",
  ]

  for (const key of requiredKeys) {
    if (!(key in details)) {
      throw new Error(`Selected block details must include ${key}`)
    }
  }
}

function updateSelectionSidebar(blockNode) {
  const details = buildSelectionDetails(blockNode)
  assertSelectionDetails(details)

  elements.metaScope.textContent = details.scope
  elements.metaType.textContent = details.blockType
  elements.metaId.textContent = details.blockId
  if (sidebarFields.sourceFile) {
    sidebarFields.sourceFile.textContent = details.sourceFile
  }
  if (sidebarFields.lineRange) {
    sidebarFields.lineRange.textContent = details.lineRange
  }
  elements.codeSnippet.textContent = details.codeSnippet
}

function selectBlock(blockId) {
  if (!appState.normalizedGraph) {
    return
  }

  const blockNode = appState.normalizedGraph.blockNodes.find((node) => node.id === blockId)
  if (!blockNode) {
    return
  }

  appState.selectedBlockId = blockId
  appState.selectedEdgeId = null
  updateSelectionSidebar(blockNode)
  renderGraph()
  updateDebugState()
}

function selectEdge(edgeId) {
  appState.selectedEdgeId = appState.selectedEdgeId === edgeId ? null : edgeId
  renderGraph()
  updateDebugState()
}

function recomputeInteractiveLayout() {
  if (!appState.baseLayout || !appState.normalizedGraph) {
    appState.layout = null
    return
  }

  const layout = applyLayoutOverrides(appState.baseLayout, {
    blockOffsets: appState.blockOffsets,
    scopeSizeAdjustments: appState.scopeSizeAdjustments,
  })
  layout.edgeRoutes = computeEdgeRoutes(appState.normalizedGraph, layout)
  appState.layout = layout
}

function getPointerPosition(event) {
  if (!elements.graphCanvas) {
    return { x: 0, y: 0 }
  }
  const rect = elements.graphCanvas.getBoundingClientRect()
  const viewBox = elements.graphCanvas.viewBox.baseVal
  const scaleX = viewBox.width / rect.width
  const scaleY = viewBox.height / rect.height
  return {
    x: (event.clientX - rect.left) * scaleX,
    y: (event.clientY - rect.top) * scaleY,
  }
}

function finishPointerAction() {
  activePointerAction = null
  window.removeEventListener("pointermove", handlePointerMove)
  window.removeEventListener("pointerup", handlePointerUp)
  window.removeEventListener("pointercancel", handlePointerUp)
  window.removeEventListener("mousemove", handlePointerMove)
  window.removeEventListener("mouseup", handlePointerUp)
}

function handlePointerMove(event) {
  if (!activePointerAction) {
    return
  }

  const point = getPointerPosition(event)
  if (activePointerAction.type === "drag-block") {
    const delta = {
      x: point.x - activePointerAction.start.x,
      y: point.y - activePointerAction.start.y,
    }
    if (Math.abs(delta.x) > 2 || Math.abs(delta.y) > 2) {
      activePointerAction.moved = true
      suppressNextBlockClick = true
    }
    appState.blockOffsets.set(activePointerAction.blockId, {
      x: activePointerAction.initial.x + delta.x,
      y: activePointerAction.initial.y + delta.y,
    })
  }

  if (activePointerAction.type === "resize-scope") {
    const delta = {
      width: point.x - activePointerAction.start.x,
      height: point.y - activePointerAction.start.y,
    }
    appState.scopeSizeAdjustments.set(activePointerAction.scopeName, {
      width: activePointerAction.initial.width + delta.width,
      height: activePointerAction.initial.height + delta.height,
    })
  }

  recomputeInteractiveLayout()
  renderGraph()
  updateDebugState()
}

function handlePointerUp() {
  if (activePointerAction?.moved) {
    queueMicrotask(() => {
      suppressNextBlockClick = false
    })
  }
  finishPointerAction()
}

function startBlockDrag(event, blockId) {
  const current = appState.blockOffsets.get(blockId) ?? { x: 0, y: 0 }
  activePointerAction = {
    type: "drag-block",
    blockId,
    start: getPointerPosition(event),
    initial: { ...current },
    moved: false,
  }
  window.addEventListener("pointermove", handlePointerMove)
  window.addEventListener("pointerup", handlePointerUp)
  window.addEventListener("pointercancel", handlePointerUp)
  window.addEventListener("mousemove", handlePointerMove)
  window.addEventListener("mouseup", handlePointerUp)
}

function startScopeResize(event, scopeName) {
  const current = appState.scopeSizeAdjustments.get(scopeName) ?? { width: 0, height: 0 }
  activePointerAction = {
    type: "resize-scope",
    scopeName,
    start: getPointerPosition(event),
    initial: { ...current },
    moved: true,
  }
  window.addEventListener("pointermove", handlePointerMove)
  window.addEventListener("pointerup", handlePointerUp)
  window.addEventListener("pointercancel", handlePointerUp)
  window.addEventListener("mousemove", handlePointerMove)
  window.addEventListener("mouseup", handlePointerUp)
}

function summarizeGraph(raw, normalized) {
  const blockCount = Array.isArray(raw.blocks) ? raw.blocks.length : 0
  const nodeCount = Array.isArray(raw.nodes) ? raw.nodes.length : 0
  const edgeCount = Array.isArray(raw.edges) ? raw.edges.length : 0
  const target = typeof raw.target === "string" && raw.target ? raw.target : "unknown target"
  const scopeCount = normalized.scopeGroups.length

  return `Loaded graph for ${target} (${blockCount} blocks, ${nodeCount} nodes, ${edgeCount} edges, ${scopeCount} scope groups).`
}

function readStoredDebugPreference() {
  try {
    return window.localStorage.getItem(DEBUG_STORAGE_KEY) === "true"
  } catch {
    return false
  }
}

function storeDebugPreference(value) {
  try {
    window.localStorage.setItem(DEBUG_STORAGE_KEY, value ? "true" : "false")
  } catch {
    // Ignore storage failures in restricted environments.
  }
}

function is_slice_graph_shape(raw) {
  return (
    typeof raw === "object" &&
    raw !== null &&
    !Array.isArray(raw) &&
    Array.isArray(raw.blocks) &&
    Array.isArray(raw.nodes) &&
    Array.isArray(raw.edges)
  )
}

function normalizeSliceGraph(raw) {
  const seenNodeIds = new Set()
  for (const node of raw.nodes) {
    if (seenNodeIds.has(node.id)) {
      throw new Error(`Duplicate node id found: ${node.id}`)
    }
    seenNodeIds.add(node.id)
  }

  const seenBlockIds = new Set()
  for (const block of raw.blocks) {
    if (seenBlockIds.has(block.id)) {
      throw new Error(`Duplicate block id found: ${block.id}`)
    }
    seenBlockIds.add(block.id)
  }

  const blocksById = new Map(raw.blocks.map((block) => [block.id, block]))
  const scopeGroupsByName = new Map()
  const nodesById = new Map()
  const blockNodes = []
  const literalNodes = []

  for (const node of raw.nodes) {
    if (node.kind === "block") {
      const blockMeta = blocksById.get(node.block_id) ?? null
      if (!blockMeta) {
        throw new Error(`Missing block metadata for block_id ${node.block_id}`)
      }
      const blockNode = {
        ...node,
        scope: blockMeta.scope,
        block_type: blockMeta.block_type,
        meta: blockMeta,
      }
      blockNodes.push(blockNode)
      nodesById.set(blockNode.id, blockNode)

      const scopeName = blockNode.scope
      if (!scopeGroupsByName.has(scopeName)) {
        scopeGroupsByName.set(scopeName, [])
      }
      scopeGroupsByName.get(scopeName).push(blockNode)
      continue
    }

    if (node.kind === "literal") {
      const literalNode = { ...node }
      literalNodes.push(literalNode)
      nodesById.set(literalNode.id, literalNode)
      continue
    }

    throw new Error(`Unsupported node kind: ${node.kind}`)
  }

  const edges = raw.edges.map((edge, index) => {
    const fromNode = nodesById.get(edge.from)
    const toNode = nodesById.get(edge.to)
    if (!fromNode) {
      throw new Error(`Edge ${index} references missing from node ${edge.from}`)
    }
    if (!toNode) {
      throw new Error(`Edge ${index} references missing to node ${edge.to}`)
    }

    return {
      ...edge,
      id: index,
      label: edge.signal?.name ?? null,
      fromNode,
      toNode,
    }
  })

  const scopeGroups = Array.from(scopeGroupsByName, ([scope, nodes]) => ({
    scope,
    nodes,
  }))

  return {
    nodesById,
    blocksById,
    blockNodes,
    literalNodes,
    edges,
    scopeGroups,
  }
}

function assertNormalizedGraphData(graph) {
  if (!(graph.nodesById instanceof Map)) {
    throw new Error("normalizeSliceGraph must return nodesById")
  }
  if (!(graph.blocksById instanceof Map)) {
    throw new Error("normalizeSliceGraph must return blocksById")
  }
  if (!Array.isArray(graph.blockNodes)) {
    throw new Error("normalizeSliceGraph must return blockNodes")
  }
  if (!Array.isArray(graph.literalNodes)) {
    throw new Error("normalizeSliceGraph must return literalNodes")
  }
  if (!Array.isArray(graph.edges)) {
    throw new Error("normalizeSliceGraph must return edges")
  }
  if (!Array.isArray(graph.scopeGroups)) {
    throw new Error("normalizeSliceGraph must return scopeGroups")
  }
  if (graph.blockNodes.some((node) => !node.meta || !node.scope || !node.block_type)) {
    throw new Error("normalizeSliceGraph must join block nodes with block metadata")
  }
  if (graph.edges.some((edge) => !edge.fromNode || !edge.toNode)) {
    throw new Error("normalizeSliceGraph must resolve edge endpoint nodes")
  }
}

function blockTypeRank(blockType) {
  if (blockType === "ModInput") {
    return 0
  }
  if (blockType === "Assign") {
    return 1
  }
  if (blockType === "Always") {
    return 2
  }
  if (blockType === "ModOutput") {
    return 4
  }
  return 3
}

function classifyScopeColumn(blockNode) {
  if (blockNode.block_type === "ModInput") {
    return "left"
  }
  if (blockNode.block_type === "ModOutput") {
    return "right"
  }
  return "center"
}

function sortBlockNodes(nodes) {
  return [...nodes].sort((left, right) => {
    const rankDelta = blockTypeRank(left.block_type) - blockTypeRank(right.block_type)
    if (rankDelta !== 0) {
      return rankDelta
    }
    return left.id - right.id
  })
}

function buildScopeTree(graph) {
  const scopeNames = graph.scopeGroups
    .map((group) => group.scope)
    .sort((left, right) => left.localeCompare(right))
  const scopeNodes = new Map()

  for (const scopeName of scopeNames) {
    scopeNodes.set(scopeName, {
      name: scopeName,
      blockNodes: sortBlockNodes(graph.scopeGroups.find((group) => group.scope === scopeName)?.nodes ?? []),
      children: [],
    })
  }

  const roots = []
  for (const scopeName of scopeNames) {
    const scopeNode = scopeNodes.get(scopeName)
    const segments = scopeName.split(".")
    let parent = null

    for (let size = segments.length - 1; size > 0; size -= 1) {
      const candidate = segments.slice(0, size).join(".")
      if (scopeNodes.has(candidate)) {
        parent = scopeNodes.get(candidate)
        break
      }
    }

    if (parent) {
      parent.children.push(scopeNode)
    } else {
      roots.push(scopeNode)
    }
  }

  for (const scopeNode of scopeNodes.values()) {
    scopeNode.children.sort((left, right) => left.name.localeCompare(right.name))
  }

  roots.sort((left, right) => left.name.localeCompare(right.name))
  return roots
}

function layoutScope(scopeNode) {
  const columns = {
    left: [],
    center: [],
    right: [],
  }

  for (const blockNode of scopeNode.blockNodes) {
    columns[classifyScopeColumn(blockNode)].push(blockNode)
  }

  const blockEntries = []
  let contentHeight = 0

  for (const columnName of ["left", "center", "right"]) {
    const nodes = columns[columnName]
    nodes.forEach((blockNode, index) => {
      const y = SCOPE_PAD + SCOPE_TITLE_H + index * (BLOCK_H + ROW_GAP)
      blockEntries.push([blockNode.id, { column: columnName, y, width: BLOCK_W, height: BLOCK_H }])
    })

    const columnHeight = nodes.length > 0 ? nodes.length * BLOCK_H + (nodes.length - 1) * ROW_GAP : 0
    contentHeight = Math.max(contentHeight, columnHeight)
  }

  const ownInnerWidth = 3 * BLOCK_W + 2 * COL_GAP
  let childOffsetY = SCOPE_PAD + SCOPE_TITLE_H + contentHeight
  if (contentHeight > 0 && scopeNode.children.length > 0) {
    childOffsetY += SCOPE_GAP
  }

  const translatedScopes = new Map()
  const childLayouts = []
  let childInnerWidth = 0

  for (const childNode of scopeNode.children) {
    const childLayout = layoutScope(childNode)
    childLayouts.push(childLayout)
    childInnerWidth = Math.max(childInnerWidth, childLayout.width)
  }

  const innerWidth = Math.max(ownInnerWidth, childInnerWidth)
  const columnX = new Map([
    ["left", SCOPE_PAD],
    ["center", SCOPE_PAD + (innerWidth - BLOCK_W) / 2],
    ["right", SCOPE_PAD + innerWidth - BLOCK_W],
  ])
  const translatedBlocks = new Map(
    blockEntries.map(([blockId, rect]) => [
      blockId,
      {
        x: columnX.get(rect.column),
        y: rect.y,
        width: rect.width,
        height: rect.height,
      },
    ]),
  )
  let runningChildY = childOffsetY

  for (const childLayout of childLayouts) {
    const childX = SCOPE_PAD
    const childY = runningChildY

    for (const [blockId, rect] of childLayout.blocks) {
      translatedBlocks.set(blockId, {
        ...rect,
        x: rect.x + childX,
        y: rect.y + childY,
      })
    }

    for (const [scopeName, rect] of childLayout.scopes) {
      translatedScopes.set(scopeName, {
        ...rect,
        x: rect.x + childX,
        y: rect.y + childY,
      })
    }

    runningChildY += childLayout.height + SCOPE_GAP
  }

  const childrenHeight = childLayouts.length > 0
    ? childLayouts.reduce((total, childLayout) => total + childLayout.height, 0) + SCOPE_GAP * Math.max(0, childLayouts.length - 1)
    : 0
  const height = Math.max(
    SCOPE_PAD * 2 + SCOPE_TITLE_H,
    childLayouts.length > 0
      ? childOffsetY + childrenHeight + SCOPE_PAD
      : SCOPE_PAD + SCOPE_TITLE_H + contentHeight + SCOPE_PAD,
  )
  const width = innerWidth + SCOPE_PAD * 2

  translatedScopes.set(scopeNode.name, {
    x: 0,
    y: 0,
    width,
    height,
    title: scopeNode.name,
    depth: scopeNode.name.split(".").length - 1,
  })

  return {
    width,
    height,
    blocks: translatedBlocks,
    scopes: translatedScopes,
  }
}

function computeEdgeAnchors(graph, blockLayout) {
  return graph.edges.map((edge) => {
    const fromRect = blockLayout.blocks.get(edge.fromNode.id)
    const toRect = blockLayout.blocks.get(edge.toNode.id)
    if (!fromRect || !toRect) {
      return null
    }

    const fromCenterX = fromRect.x + fromRect.width / 2
    const toCenterX = toRect.x + toRect.width / 2
    const useHorizontal = fromCenterX <= toCenterX

    return {
      edgeId: edge.id,
      from: {
        x: useHorizontal ? fromRect.x + fromRect.width : fromCenterX,
        y: fromRect.y + fromRect.height / 2,
      },
      to: {
        x: useHorizontal ? toRect.x : toCenterX,
        y: toRect.y + toRect.height / 2,
      },
      label: edge.label,
    }
  }).filter(Boolean)
}

function assertUniqueBlockPositions(layout) {
  const seen = new Map()

  for (const [blockId, rect] of layout.blocks) {
    const key = `${rect.x},${rect.y}`
    if (seen.has(key)) {
      throw new Error(`Duplicate block position for blocks ${seen.get(key)} and ${blockId} at ${key}`)
    }
    seen.set(key, blockId)
  }
}

function computeLayout(graph) {
  const roots = buildScopeTree(graph)
  const blocks = new Map()
  const scopes = new Map()
  let contentWidth = 0
  let contentHeight = 0
  let cursorY = STAGE_PAD

  for (const rootScope of roots) {
    const rootLayout = layoutScope(rootScope)

    for (const [blockId, rect] of rootLayout.blocks) {
      blocks.set(blockId, {
        ...rect,
        x: rect.x + STAGE_PAD,
        y: rect.y + cursorY,
      })
    }

    for (const [scopeName, rect] of rootLayout.scopes) {
      scopes.set(scopeName, {
        ...rect,
        x: rect.x + STAGE_PAD,
        y: rect.y + cursorY,
      })
      contentWidth = Math.max(contentWidth, rect.x + STAGE_PAD + rect.width)
    }

    contentHeight = Math.max(contentHeight, cursorY + rootLayout.height)
    cursorY += rootLayout.height + SCOPE_GAP
  }

  const layout = {
    width: Math.max(960, contentWidth + STAGE_PAD),
    height: Math.max(720, contentHeight + STAGE_PAD),
    blocks,
    scopes,
  }

  assertUniqueBlockPositions(layout)
  layout.edgeAnchors = computeEdgeAnchors(graph, layout)
  return layout
}

function createSvgElement(name, attributes = {}) {
  const element = document.createElementNS(SVG_NS, name)
  for (const [key, value] of Object.entries(attributes)) {
    element.setAttribute(key, String(value))
  }
  return element
}

function ensureDebugUi() {
  if (!debugToggleButton && elements.graphHeader) {
    debugToggleButton = document.createElement("button")
    debugToggleButton.type = "button"
    debugToggleButton.className = "debug-toggle button-secondary"
    debugToggleButton.addEventListener("click", () => {
      appState.debugVisible = !appState.debugVisible
      storeDebugPreference(appState.debugVisible)
      syncDebugUi()
      updateDebugState()
    })
    elements.graphHeader.append(debugToggleButton)
  }

  if (!debugOverlay && elements.graphSurface) {
    debugOverlay = document.createElement("pre")
    debugOverlay.className = "debug-overlay"
    debugOverlay.hidden = true
    elements.graphSurface.append(debugOverlay)
  }

  syncDebugUi()
}

function buildDebugOverlayText() {
  if (!appState.normalizedGraph || !appState.layout) {
    return "Debug layout: load a graph to inspect computed positions."
  }

  const blockLines = appState.normalizedGraph.blockNodes
    .slice()
    .sort((left, right) => left.id - right.id)
    .map((blockNode) => {
      const rect = appState.layout.blocks.get(blockNode.id)
      return `block ${blockNode.id} ${blockNode.block_type} @ ${blockNode.scope}: (${rect.x}, ${rect.y}) ${rect.width}x${rect.height}`
    })

  const scopeLines = Array.from(appState.layout.scopes.entries())
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([scopeName, rect]) => `scope ${scopeName}: (${rect.x}, ${rect.y}) ${rect.width}x${rect.height}`)

  const edgeLines = appState.layout.edgeRoutes
    .map((route) => `edge ${route.edgeId}: ${route.points.map((point) => `(${point.x}, ${point.y})`).join(" -> ")} ${route.label ?? ""}`)

  return [
    "Debug layout",
    "",
    ...blockLines,
    "",
    ...scopeLines,
    "",
    ...edgeLines,
  ].join("\n")
}

function syncDebugUi() {
  if (debugToggleButton) {
    debugToggleButton.textContent = appState.debugVisible ? "Hide Debug" : "Show Debug"
    debugToggleButton.setAttribute("aria-pressed", appState.debugVisible ? "true" : "false")
  }

  if (debugOverlay) {
    debugOverlay.hidden = !appState.debugVisible
    debugOverlay.textContent = buildDebugOverlayText()
  }
}

function renderEmptyState() {
  if (!elements.graphCanvas) {
    return
  }

  elements.graphCanvas.setAttribute("viewBox", "0 0 960 720")
  elements.graphCanvas.replaceChildren(
    createSvgElement("defs"),
    createSvgElement("rect", {
      x: 24,
      y: 24,
      width: 912,
      height: 672,
      rx: 28,
      class: "stage-frame",
    }),
    createSvgElement("g", { id: "graph-placeholder" }),
  )

  const placeholder = elements.graphCanvas.querySelector("#graph-placeholder")
  placeholder.append(
    createSvgElement("text", { x: 80, y: 112, class: "svg-kicker" }),
    createSvgElement("text", { x: 80, y: 160, class: "svg-title" }),
    createSvgElement("text", { x: 80, y: 204, class: "svg-copy" }),
  )
  placeholder.children[0].textContent = "SVG STAGE"
  placeholder.children[1].textContent = "Load a slice graph to begin."
  placeholder.children[2].textContent = "Task 3 adds deterministic scope layout for block nodes."
}

function renderGraph() {
  if (!elements.graphCanvas) {
    return
  }

  if (!appState.normalizedGraph || !appState.layout) {
    renderEmptyState()
    syncDebugUi()
    return
  }

  const svg = elements.graphCanvas
  svg.setAttribute("viewBox", `0 0 ${appState.layout.width} ${appState.layout.height}`)

  const defs = createSvgElement("defs")
  const scopeLayer = createSvgElement("g", { class: "scope-layer" })
  const edgeLayer = createSvgElement("g", { class: "edge-layer" })
  const blockLayer = createSvgElement("g", { class: "block-layer" })
  const debugLayer = createSvgElement("g", {
    class: `debug-layer${appState.debugVisible ? " is-visible" : ""}`,
  })

  const stageFrame = createSvgElement("rect", {
    x: STAGE_PAD,
    y: STAGE_PAD,
    width: Math.max(0, appState.layout.width - STAGE_PAD * 2),
    height: Math.max(0, appState.layout.height - STAGE_PAD * 2),
    rx: 28,
    class: "stage-frame",
  })
  stageFrame.addEventListener("click", () => {
    if (appState.selectedEdgeId !== null) {
      appState.selectedEdgeId = null
      renderGraph()
      updateDebugState()
    }
  })

  for (const route of appState.layout.edgeRoutes) {
    const isSelected = route.edgeId === appState.selectedEdgeId
    const path = createSvgElement("path", {
      d: buildCurvedPath(route.points),
      class: `graph-edge${isSelected ? " is-selected" : ""}`,
      "data-edge-id": route.edgeId,
    })
    path.addEventListener("click", (event) => {
      event.stopPropagation()
      selectEdge(route.edgeId)
    })
    edgeLayer.append(path)

    const arrowHead = createSvgElement("path", {
      d: buildArrowHead(route.points),
      class: `graph-edge-head${isSelected ? " is-selected" : ""}`,
      "data-edge-id": route.edgeId,
    })
    arrowHead.addEventListener("click", (event) => {
      event.stopPropagation()
      selectEdge(route.edgeId)
    })
    edgeLayer.append(arrowHead)

    if (route.label && isSelected) {
      const labelPosition = computeEdgeLabelPosition(route)
      const label = createSvgElement("text", {
        x: labelPosition.x,
        y: labelPosition.y,
        class: "edge-label",
        "text-anchor": "middle",
      })
      label.textContent = route.label
      edgeLayer.append(label)
    }
  }

  const sortedScopes = Array.from(appState.layout.scopes.entries()).sort(([, left], [, right]) => left.depth - right.depth || left.y - right.y || left.x - right.x)
  for (const [scopeName, rect] of sortedScopes) {
    scopeLayer.append(
      createSvgElement("rect", {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        rx: 20,
        class: "scope-rect",
      }),
    )

    const resizeHandle = createSvgElement("rect", {
      x: rect.x + rect.width - 14,
      y: rect.y + rect.height - 14,
      width: 14,
      height: 14,
      rx: 4,
      class: "scope-resize-handle",
      role: "button",
      tabindex: 0,
      "aria-label": `Resize scope ${scopeName}`,
    })
    resizeHandle.addEventListener("pointerdown", (event) => {
      event.preventDefault()
      event.stopPropagation()
      startScopeResize(event, scopeName)
    })
    resizeHandle.addEventListener("mousedown", (event) => {
      event.preventDefault()
      event.stopPropagation()
      startScopeResize(event, scopeName)
    })
    scopeLayer.append(resizeHandle)

    const label = createSvgElement("text", {
      x: rect.x + SCOPE_PAD,
      y: rect.y + 22,
      class: "scope-label",
    })
    label.textContent = scopeName
    scopeLayer.append(label)
  }

  const sortedBlocks = appState.normalizedGraph.blockNodes.slice().sort((left, right) => left.id - right.id)
  for (const blockNode of sortedBlocks) {
    const rect = appState.layout.blocks.get(blockNode.id)
    const isSelected = blockNode.id === appState.selectedBlockId
    const isTargetDriver = blockNode.id === appState.targetDriverBlockId
    const classNames = ["block-node"]
    if (isSelected) {
      classNames.push("is-selected")
    }
    if (isTargetDriver) {
      classNames.push("is-target-driver")
    }
    const group = createSvgElement("g", {
      class: classNames.join(" "),
      "data-block-id": blockNode.id,
      transform: `translate(${rect.x}, ${rect.y})`,
      tabindex: 0,
      role: "button",
      "aria-label": `${blockNode.block_type} block b${blockNode.id} in ${blockNode.scope}${typeof blockNode.time === "number" ? ` at time ${blockNode.time}` : ""}`,
      "aria-pressed": isSelected ? "true" : "false",
    })
    group.addEventListener("click", () => {
      if (suppressNextBlockClick) {
        return
      }
      selectBlock(blockNode.id)
    })
    group.addEventListener("pointerdown", (event) => {
      if (event.target instanceof SVGElement && event.target.closest('.scope-resize-handle')) {
        return
      }
      if (event.button !== 0) {
        return
      }
      startBlockDrag(event, blockNode.id)
    })
    group.addEventListener("mousedown", (event) => {
      if (event.button !== 0) {
        return
      }
      startBlockDrag(event, blockNode.id)
    })
    group.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault()
        selectBlock(blockNode.id)
      }
    })
    group.append(
      createSvgElement("rect", {
        width: rect.width,
        height: rect.height,
        rx: 16,
        class: `block-card block-type-${blockNode.block_type.toLowerCase()}`,
      }),
    )

    const typeLabel = createSvgElement("text", {
      x: 16,
      y: 24,
      class: "block-type-label",
    })
    typeLabel.textContent = blockNode.block_type
    group.append(typeLabel)

    const idLabel = createSvgElement("text", {
      x: 16,
      y: 42,
      class: "block-id-label",
    })
    idLabel.textContent = `b${blockNode.id}`
    group.append(idLabel)

    if (typeof blockNode.time === "number") {
      const timeLabel = createSvgElement("text", {
        x: rect.width - 16,
        y: 24,
        class: "block-time-label",
        "text-anchor": "end",
      })
      timeLabel.textContent = `t=${blockNode.time}`
      group.append(timeLabel)
    }

    blockLayer.append(group)
  }

  for (const route of appState.layout.edgeRoutes) {
    debugLayer.append(
      createSvgElement("path", {
        d: buildPolylinePath(route.points),
        class: "debug-edge-line",
      }),
      createSvgElement("circle", {
        cx: route.points[0].x,
        cy: route.points[0].y,
        r: 4,
        class: "debug-anchor debug-anchor-from",
      }),
      createSvgElement("circle", {
        cx: route.points[route.points.length - 1].x,
        cy: route.points[route.points.length - 1].y,
        r: 4,
        class: "debug-anchor debug-anchor-to",
      }),
    )
  }

  svg.replaceChildren(defs, stageFrame, scopeLayer, edgeLayer, blockLayer, debugLayer)
  syncDebugUi()
}

function updateDebugState() {
  window.__SLICE_VIEWER_STATE__ = {
    source: appState.source,
    rawGraph: appState.rawGraph,
    normalizedGraph: appState.normalizedGraph,
    baseLayout: appState.baseLayout,
    layout: appState.layout,
    selectedBlockId: appState.selectedBlockId,
    selectedEdgeId: appState.selectedEdgeId,
    targetSignal: appState.targetSignal,
    targetDriverBlockId: appState.targetDriverBlockId,
    blockOffsets: Array.from(appState.blockOffsets.entries()),
    scopeSizeAdjustments: Array.from(appState.scopeSizeAdjustments.entries()),
    debugVisible: appState.debugVisible,
  }
}

function parseAndLoad(jsonText, sourceLabel) {
  const trimmed = jsonText.trim()

  if (!trimmed) {
    setStatus("Paste slice JSON or choose a JSON file before loading.", "error")
    return
  }

  try {
    const raw = JSON.parse(trimmed)
    if (!is_slice_graph_shape(raw)) {
      throw new Error("Expected an object with blocks, nodes, and edges arrays")
    }

    const normalized = normalizeSliceGraph(raw)
    assertNormalizedGraphData(normalized)
    const baseLayout = computeGraphLayout(normalized)

    appState.rawGraph = raw
    appState.normalizedGraph = normalized
    appState.baseLayout = baseLayout
    appState.blockOffsets = new Map()
    appState.scopeSizeAdjustments = new Map()
    appState.selectedEdgeId = null
    appState.source = sourceLabel
    appState.selectedBlockId = null
    appState.targetSignal = typeof raw.target === "string" && raw.target ? raw.target : null
    appState.targetDriverBlockId = computeTargetDriverBlockId(normalized, appState.targetSignal)
    recomputeInteractiveLayout()
    resetSelectionPlaceholders()
    renderGraph()
    updateDebugState()
    setStatus(`${summarizeGraph(raw, normalized)} Source: ${sourceLabel}.`, "success")
  } catch (error) {
    appState.rawGraph = null
    appState.normalizedGraph = null
    appState.source = null
    appState.baseLayout = null
    appState.layout = null
    appState.selectedBlockId = null
    appState.selectedEdgeId = null
    appState.targetSignal = null
    appState.targetDriverBlockId = null
    appState.blockOffsets = new Map()
    appState.scopeSizeAdjustments = new Map()
    resetSelectionPlaceholders()
    renderGraph()
    updateDebugState()
    const message = error instanceof Error ? error.message : String(error)
    setStatus(`Could not parse JSON: ${message}`, "error")
  }
}

async function loadFile(file) {
  if (!file) {
    return
  }

  try {
    const text = await file.text()
    elements.jsonInput.value = text
    setStatus(`Loaded ${file.name} into the editor. Choose Load Graph to parse it.`, "info")
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    setStatus(`Could not read file: ${message}`, "error")
  }
}

async function loadDemo() {
  try {
    const text = await loadDemoSliceText()
    if (elements.jsonInput) {
      elements.jsonInput.value = text
    }
    parseAndLoad(text, DEMO_SLICE_LABEL)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    setStatus(`Could not load demo JSON: ${message}`, "error")
  }
}

elements.fileInput?.addEventListener("change", async (event) => {
  const input = event.currentTarget
  if (!(input instanceof HTMLInputElement)) {
    return
  }

  await loadFile(input.files?.[0] ?? null)
})

elements.loadGraphButton?.addEventListener("click", () => {
  parseAndLoad(elements.jsonInput?.value ?? "", "editor")
})

elements.loadDemoButton?.addEventListener("click", async () => {
  await loadDemo()
})

sidebarFields.sourceFile = ensureSidebarField("meta-source-file", "Source File")
sidebarFields.lineRange = ensureSidebarField("meta-line-range", "Line Range")
resetSelectionPlaceholders()
ensureDebugUi()
renderEmptyState()
updateDebugState()
setStatus("Waiting for slice JSON.", "info")
