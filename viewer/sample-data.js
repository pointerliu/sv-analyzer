export const DEMO_SLICE_URL = new URL("./examples/demo-static-slice.json", import.meta.url)
export const DEMO_SLICE_LABEL = "checked-in demo"
export const DEMO_TARGET_SIGNAL = "TOP.tb.result"

export async function loadDemoSliceText() {
  const response = await fetch(DEMO_SLICE_URL)

  if (!response.ok) {
    throw new Error(`HTTP ${response.status} while loading ${DEMO_SLICE_URL.pathname}`)
  }

  const raw = JSON.parse(await response.text())
  if (raw.target !== DEMO_TARGET_SIGNAL) {
    throw new Error(`Demo fixture must define target ${DEMO_TARGET_SIGNAL}`)
  }

  return JSON.stringify(raw, null, 2)
}
