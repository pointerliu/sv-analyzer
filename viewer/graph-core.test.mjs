import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'

import {
  buildArrowHead,
  buildPolylinePath,
  computeEdgeRoutes,
  computeLayout,
} from './graph-core.js'

const dynamicGraph = JSON.parse(fs.readFileSync(new URL('./examples/demo-dynamic-slice.json', import.meta.url), 'utf8'))

function normalize(raw) {
  const blocksById = new Map(raw.blocks.map((block) => [block.id, block]))
  const nodesById = new Map()
  const scopeGroupsByName = new Map()
  const blockNodes = []

  for (const node of raw.nodes) {
    if (node.kind !== 'block') {
      continue
    }
    const blockNode = { ...node, meta: blocksById.get(node.block_id), scope: blocksById.get(node.block_id).scope, block_type: blocksById.get(node.block_id).block_type }
    blockNodes.push(blockNode)
    nodesById.set(node.id, blockNode)
    const group = scopeGroupsByName.get(blockNode.scope) ?? []
    group.push(blockNode)
    scopeGroupsByName.set(blockNode.scope, group)
  }

  return {
    blockNodes,
    scopeGroups: Array.from(scopeGroupsByName, ([scope, nodes]) => ({ scope, nodes })),
    edges: raw.edges.map((edge, index) => ({ ...edge, id: index, label: edge.signal?.name ?? null, fromNode: nodesById.get(edge.from), toNode: nodesById.get(edge.to) })).filter((edge) => edge.fromNode && edge.toNode),
  }
}

function pointInsideRect(point, rect) {
  return point.x > rect.x && point.x < rect.x + rect.width && point.y > rect.y && point.y < rect.y + rect.height
}

function segmentIntersectsRect(a, b, rect) {
  if (a.x === b.x) {
    if (a.x <= rect.x || a.x >= rect.x + rect.width) {
      return false
    }
    const top = Math.min(a.y, b.y)
    const bottom = Math.max(a.y, b.y)
    return bottom > rect.y && top < rect.y + rect.height
  }
  if (a.y === b.y) {
    if (a.y <= rect.y || a.y >= rect.y + rect.height) {
      return false
    }
    const left = Math.min(a.x, b.x)
    const right = Math.max(a.x, b.x)
    return right > rect.x && left < rect.x + rect.width
  }
  return pointInsideRect(a, rect) || pointInsideRect(b, rect)
}

test('edge routes stay out of unrelated block rectangles', () => {
  const graph = {
    edges: [
      {
        id: 0,
        label: 's1',
        fromNode: { id: 1, kind: 'block' },
        toNode: { id: 2, kind: 'block' },
      },
    ],
  }
  const layout = {
    blocks: new Map([
      [1, { x: 520, y: 120, width: 180, height: 56, column: 'right' }],
      [2, { x: 24, y: 120, width: 180, height: 56, column: 'left' }],
      [3, { x: 272, y: 120, width: 180, height: 56, column: 'center' }],
    ]),
  }
  const routes = computeEdgeRoutes(graph, layout)

  const intersecting = []
  for (const route of routes) {
    for (let index = 0; index < route.points.length - 1; index += 1) {
      const a = route.points[index]
      const b = route.points[index + 1]
      for (const [blockId, rect] of layout.blocks) {
        if (blockId === route.fromNode.id || blockId === route.toNode.id) {
          continue
        }
        if (segmentIntersectsRect(a, b, rect)) {
          intersecting.push({ edgeId: route.edgeId, blockId })
        }
      }
    }
  }

  assert.deepEqual(intersecting, [])
})

test('arrow head follows the final segment direction', () => {
  const points = [{ x: 100, y: 40 }, { x: 60, y: 40 }]
  const head = buildArrowHead(points)
  const shaft = buildPolylinePath(points)

  assert.match(shaft, /^M 100 40 L 60 40$/)
  assert.match(head, /^M /)
  assert.ok(head.includes('L 60 40 L'), 'arrow head should terminate at the line tip')
})
