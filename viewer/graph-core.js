export const BLOCK_W = 180
export const BLOCK_H = 56
export const COL_GAP = 56
export const ROW_GAP = 24
export const SCOPE_PAD = 24
export const SCOPE_GAP = 40
export const SCOPE_TITLE_H = 28
export const STAGE_PAD = 24

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

export function classifyScopeColumn(blockNode) {
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
        column: rect.column,
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

export function computeLayout(graph) {
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
  return layout
}

export function computeEdgeRoutes(graph, layout) {
  const laneCounts = new Map()
  const edges = [...graph.edges].sort((left, right) => left.id - right.id)

  return edges.map((edge) => {
    const fromRect = layout.blocks.get(edge.fromNode.id)
    const toRect = layout.blocks.get(edge.toNode.id)
    if (!fromRect || !toRect) {
      return null
    }

    const from = {
      left: fromRect.x,
      right: fromRect.x + fromRect.width,
      top: fromRect.y,
      bottom: fromRect.y + fromRect.height,
      centerY: fromRect.y + fromRect.height / 2,
      centerX: fromRect.x + fromRect.width / 2,
      column: fromRect.column,
    }
    const to = {
      left: toRect.x,
      right: toRect.x + toRect.width,
      top: toRect.y,
      bottom: toRect.y + toRect.height,
      centerY: toRect.y + toRect.height / 2,
      centerX: toRect.x + toRect.width / 2,
      column: toRect.column,
    }

    const isForward = from.right <= to.left
    const laneKey = isForward
      ? `forward:${from.column}->${to.column}`
      : `return:${Math.max(from.right, to.right)}`
    const laneIndex = laneCounts.get(laneKey) ?? 0
    laneCounts.set(laneKey, laneIndex + 1)

    let points
    if (isForward) {
      const baseLaneX = from.right + (to.left - from.right) / 2
      const laneSpread = laneIndex * 18
      const laneX = baseLaneX + (laneIndex % 2 === 0 ? laneSpread : -laneSpread)
      points = [
        { x: from.right, y: from.centerY },
        { x: laneX, y: from.centerY },
        { x: laneX, y: to.centerY },
        { x: to.left, y: to.centerY },
      ]
    } else {
      const laneX = Math.max(from.right, to.right) + 28 + laneIndex * 18
      const laneY = Math.min(from.centerY, to.centerY) - 28 - laneIndex * 18
      points = [
        { x: from.right, y: from.centerY },
        { x: laneX, y: from.centerY },
        { x: laneX, y: laneY },
        { x: to.right, y: laneY },
        { x: to.right, y: to.centerY },
      ]
    }

    return {
      edgeId: edge.id,
      fromNode: edge.fromNode,
      toNode: edge.toNode,
      label: edge.label,
      points,
    }
  }).filter(Boolean)
}

export function edgeLabelPosition(route) {
  let bestSegment = null
  for (let index = 0; index < route.points.length - 1; index += 1) {
    const start = route.points[index]
    const end = route.points[index + 1]
    const length = Math.abs(end.x - start.x) + Math.abs(end.y - start.y)
    if (!bestSegment || length > bestSegment.length) {
      bestSegment = { start, end, length }
    }
  }

  if (!bestSegment) {
    return { x: 0, y: 0 }
  }

  const horizontal = bestSegment.start.y === bestSegment.end.y
  return {
    x: (bestSegment.start.x + bestSegment.end.x) / 2,
    y: horizontal
      ? bestSegment.start.y - 10
      : (bestSegment.start.y + bestSegment.end.y) / 2 - 6,
  }
}

export function buildPolylinePath(points) {
  return points.map((point, index) => `${index === 0 ? "M" : "L"} ${point.x} ${point.y}`).join(" ")
}

export function buildArrowHead(points, size = 10, wing = 4) {
  if (points.length < 2) {
    return ""
  }
  const tip = points[points.length - 1]
  const base = points[points.length - 2]
  const angle = Math.atan2(tip.y - base.y, tip.x - base.x)
  const left = {
    x: tip.x - size * Math.cos(angle) + wing * Math.sin(angle),
    y: tip.y - size * Math.sin(angle) - wing * Math.cos(angle),
  }
  const right = {
    x: tip.x - size * Math.cos(angle) - wing * Math.sin(angle),
    y: tip.y - size * Math.sin(angle) + wing * Math.cos(angle),
  }
  return `M ${left.x} ${left.y} L ${tip.x} ${tip.y} L ${right.x} ${right.y}`
}
