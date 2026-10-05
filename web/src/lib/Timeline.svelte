<script lang="ts">
  // One soul's memory tree, drawn as what it is: time spans stacked by level.
  //
  // Time runs left to right. The bottom row is single memories, one tick each,
  // placed in the hour they happened. Every row above is a level of summary:
  // level k+1 covers a window of 2^k hours, so each row's bars are twice as
  // long as the row below and never overlap one another. Nothing is laid out
  // by a simulation, so the same soul draws the same picture every time.
  //
  // Plain canvas 2D. lUX is about 35,000 nodes, which is fine for a canvas
  // that skips what is off screen, and not worth a WebGL renderer.
  import { onMount } from 'svelte'
  import type { Tree, NodeView, Line } from './api'
  import { levelName, when, span } from './format'

  interface Props {
    tree: Tree
    selected: string | null
    hits: Set<string>
    onselect: (id: string | null) => void
    fetchNode: (id: string) => Promise<NodeView>
    /** The soul's startup view, or null to hide it. */
    wake: Line[] | null
  }
  let { tree, selected, hits, onselect, fetchNode, wake }: Props = $props()

  let wrap: HTMLDivElement
  let canvas: HTMLCanvasElement
  let W = $state(800)
  let H = $state(500)

  const GUTTER = 74
  const AXIS = 30
  const PAD_R = 14
  const PAD_T = 10
  const HOUR = 3600

  // ---- derived geometry, rebuilt when the tree changes -------------------
  /** [lo, hi) index range of each level in the columns (they are sorted). */
  let ranges: [number, number][] = []
  /** Where inside its hour each memory tick sits: memories sharing an hour
   *  spread evenly across it, so a busy hour reads as a dense hour once you
   *  zoom in, instead of one tick drawn forty times. */
  let frac: Float32Array = new Float32Array(0)
  let index = new Map<string, number>()

  function prepare(t: Tree) {
    ranges = Array.from({ length: t.top + 1 }, () => [0, 0] as [number, number])
    let i = 0
    for (let l = 0; l <= t.top; l++) {
      const lo = i
      while (i < t.count && t.level[i] === l) i++
      ranges[l] = [lo, i]
    }
    frac = new Float32Array(t.count)
    const [m0, m1] = ranges[0] ?? [0, 0]
    for (let a = m0; a < m1; ) {
      let b = a
      while (b < m1 && t.start[b] === t.start[a]) b++
      for (let k = a; k < b; k++) frac[k] = (k - a + 0.5) / (b - a)
      a = b
    }
    index = new Map(t.ids.map((id, k) => [id, k]))
  }

  // ---- view: the visible time span ---------------------------------------
  let t0 = $state(0)
  let t1 = $state(1)

  function fit() {
    const pad = Math.max((tree.last - tree.first) * 0.015, HOUR)
    t0 = tree.first - pad
    t1 = tree.last + pad
  }

  let prepared: Tree | null = null
  $effect(() => {
    if (tree !== prepared) {
      prepare(tree)
      prepared = tree
      fit()
    }
  })

  const plotW = () => Math.max(10, W - GUTTER - PAD_R)
  // The rows fill the height. The memory row gets two shares: it is the
  // densest row and the one every summary is ultimately made of.
  const unit = () => Math.max(10, Math.min(150, (H - AXIS - PAD_T) / (tree.top + 2)))
  const rowH = (level: number) => (level === 0 ? 2 * unit() : unit())
  const x = (t: number) => GUTTER + ((t - t0) / (t1 - t0)) * plotW()
  const tAt = (px: number) => t0 + ((px - GUTTER) / plotW()) * (t1 - t0)
  /** Level drawn at the top, memories at the bottom. */
  const rowTop = (level: number) => PAD_T + (tree.top - level) * unit()
  const plotBottom = () => rowTop(0) + rowH(0)

  function nodeX(k: number): [number, number] {
    if (tree.level[k] === 0) {
      const xm = x(tree.start[k] + frac[k] * (tree.end[k] - tree.start[k]))
      return [xm, xm]
    }
    return [x(tree.start[k]), x(tree.end[k])]
  }

  // ---- relation to the selection -----------------------------------------
  // A window's descendants are everything inside its span on the levels
  // below; its ancestors are the windows above that contain it. Both follow
  // from time alone, which is how the index is built, so no edge list needs
  // to travel to draw the family.
  function related(k: number, s: number): boolean {
    if (k === s) return false
    const ls = tree.level[s]
    const lk = tree.level[k]
    if (lk < ls) return tree.start[k] >= tree.start[s] && tree.start[k] < tree.end[s]
    if (lk > ls) return tree.start[s] >= tree.start[k] && tree.start[s] < tree.end[k]
    return false
  }

  // ---- the startup view ---------------------------------------------------
  // What `ravel memory` prints when the soul wakes: fine detail near now,
  // coarser summaries further back. Coloured by its order in time, old in
  // ochre and recent in green, after the figure in the OptMem paper. A line
  // whose window has not been summarized yet is not in the tree; it is drawn
  // from its own span, outlined rather than filled, so a gap in what the soul
  // remembers is visible as a gap.
  const STOPS = [
    [0xbd, 0x76, 0x3a],
    [0xc4, 0x9b, 0x46],
    [0xb6, 0xc4, 0x55],
    [0x5f, 0xc0, 0x5a],
  ]
  function ramp(u: number): string {
    const f = Math.min(0.9999, Math.max(0, u)) * (STOPS.length - 1)
    const i = Math.floor(f)
    const t = f - i
    const c = STOPS[i].map((v, j) => Math.round(v + (STOPS[i + 1][j] - v) * t))
    return `rgb(${c[0]},${c[1]},${c[2]})`
  }
  interface Mark {
    k: number
    level: number
    a: number
    b: number
    color: string
    pending: boolean
  }
  const marks = $derived.by((): Mark[] => {
    if (!wake) return []
    // Its own id map, from the tree prop itself, so it can never be read
    // against a tree that has not been prepared yet.
    const at = new Map(tree.ids.map((id, k) => [id, k]))
    const sorted = [...wake].sort((p, q) => p.from.localeCompare(q.from))
    const n = Math.max(1, sorted.length - 1)
    return sorted.map((l, i) => {
      const k = at.get(l.id) ?? -1
      const a = Date.parse(l.from) / 1000
      const b = Date.parse(l.to) / 1000
      return {
        k,
        level: l.level,
        a,
        b: b > a ? b : a,
        color: ramp(i / n),
        pending: k < 0 && l.text.includes('not summarized'),
      }
    })
  })

  function drawWake(c: CanvasRenderingContext2D) {
    for (const m of marks) {
      if (m.level > tree.top) continue
      const y = rowTop(m.level)
      const rh = rowH(m.level)
      if (m.level === 0) {
        const xm = m.k >= 0 ? nodeX(m.k)[0] : x(m.a)
        c.fillStyle = m.color
        c.fillRect(Math.round(xm) - 1, y + 2, 3, rh - 4)
        continue
      }
      const xa = m.k >= 0 ? nodeX(m.k)[0] : x(m.a)
      const xb = m.k >= 0 ? nodeX(m.k)[1] : x(m.b)
      const w = Math.max(2, xb - xa - 1)
      if (m.pending) {
        c.strokeStyle = m.color
        c.lineWidth = 1.5
        c.setLineDash([3, 2])
        c.strokeRect(xa + 0.75, y + 3.75, Math.max(1, w - 1.5), rh - 7.5)
        c.setLineDash([])
      } else {
        c.fillStyle = m.color
        c.fillRect(xa, y + 3, w, rh - 6)
      }
    }
  }

  // ---- drawing ------------------------------------------------------------
  let raf = 0
  function request() {
    if (!raf) raf = requestAnimationFrame(draw)
  }

  $effect(() => {
    // Everything that changes the picture.
    void [W, H, t0, t1, selected, hits, hover, tree, marks]
    request()
  })

  function css(name: string) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  }

  function draw() {
    raf = 0
    if (!canvas) return
    const dpr = window.devicePixelRatio || 1
    if (canvas.width !== Math.round(W * dpr) || canvas.height !== Math.round(H * dpr)) {
      canvas.width = Math.round(W * dpr)
      canvas.height = Math.round(H * dpr)
    }
    const c = canvas.getContext('2d')!
    c.setTransform(dpr, 0, 0, dpr, 0, 0)
    c.clearRect(0, 0, W, H)

    const ink = css('--ink') || '#111'
    const faint = css('--ink-faint') || '#8a8a8a'
    const rule = css('--rule') || '#e3e3e3'
    const tint = css('--paper-tint') || '#f7f7f5'
    const hit = css('--hit') || '#2459c4'
    const sel = selected !== null ? (index.get(selected) ?? -1) : -1

    // Row bands and labels.
    c.font = '11px ui-monospace, SFMono-Regular, Menlo, monospace'
    c.textBaseline = 'middle'
    for (let l = 0; l <= tree.top; l++) {
      const y = rowTop(l)
      const rh = rowH(l)
      if (l % 2 === 0) {
        c.fillStyle = tint
        c.fillRect(GUTTER, y, plotW(), rh)
      }
      c.fillStyle = faint
      c.textAlign = 'right'
      c.fillText(l === 0 ? 'memory' : span(2 ** (l - 1)), GUTTER - 8, y + rh / 2)
    }

    // Axis.
    drawAxis(c, ink, faint, rule)

    c.save()
    c.beginPath()
    c.rect(GUTTER, 0, plotW(), plotBottom())
    c.clip()

    // Summaries first, memories on top; within each, plain then related then
    // hits then selection, so the things you asked about are never buried.
    for (let pass = 0; pass < 4; pass++) {
      // The startup view sits above the plain tree and its family, and
      // below search hits and the selection.
      if (pass === 2) drawWake(c)
      for (let l = tree.top; l >= 0; l--) {
        const [lo, hi] = ranges[l]
        const y = rowTop(l)
        const rh = rowH(l)
        for (let k = lo; k < hi; k++) {
          if (tree.end[k] < t0 || tree.start[k] > t1) continue
          const isSel = k === sel
          const isHit = hits.has(tree.ids[k])
          const isRel = sel >= 0 && !isSel && related(k, sel)
          const p = isSel ? 3 : isHit ? 2 : isRel ? 1 : 0
          if (p !== pass) continue
          const [xa, xb] = nodeX(k)
          if (l === 0) {
            c.strokeStyle = isSel ? ink : isHit ? hit : isRel ? ink : faint
            c.lineWidth = isSel ? 2 : 1
            c.beginPath()
            c.moveTo(Math.round(xa) + 0.5, y + 3)
            c.lineTo(Math.round(xa) + 0.5, y + rh - 3)
            c.stroke()
          } else {
            const w = Math.max(1, xb - xa - 1)
            c.fillStyle = isSel ? ink : isHit ? hit : isRel ? '#a9a9a3' : '#e1e1dc'
            c.fillRect(xa, y + 3, w, rh - 6)
            if (w > 4 && !isSel) {
              c.strokeStyle = isHit ? hit : '#b5b5b0'
              c.lineWidth = 1
              c.strokeRect(Math.round(xa) + 0.5, y + 3.5, Math.max(0, Math.round(w) - 1), rh - 7)
            }
          }
        }
      }
    }

    if (hover !== null && hover !== sel) {
      const [xa, xb] = nodeX(hover)
      const y = rowTop(tree.level[hover])
      const rh = rowH(tree.level[hover])
      c.strokeStyle = ink
      c.lineWidth = 1.5
      if (tree.level[hover] === 0) {
        c.strokeRect(xa - 3, y + 1, 6, rh - 2)
      } else {
        c.strokeRect(xa, y + 2, Math.max(2, xb - xa - 1), rh - 4)
      }
    }

    // Now, if it is in view: the right edge of the tree is "as of" here.
    const nowX = x(Date.now() / 1000)
    if (nowX > GUTTER && nowX < W - PAD_R) {
      c.strokeStyle = rule
      c.setLineDash([3, 3])
      c.beginPath()
      c.moveTo(nowX + 0.5, PAD_T)
      c.lineTo(nowX + 0.5, plotBottom())
      c.stroke()
      c.setLineDash([])
    }
    c.restore()
  }

  const STEPS = [
    HOUR, 3 * HOUR, 6 * HOUR, 12 * HOUR, 86400, 2 * 86400, 7 * 86400, 14 * 86400, 30 * 86400, 61 * 86400,
    91 * 86400, 182 * 86400, 365 * 86400,
  ]
  const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

  function drawAxis(c: CanvasRenderingContext2D, ink: string, faint: string, rule: string) {
    const y = plotBottom()
    c.strokeStyle = ink
    c.lineWidth = 1
    c.beginPath()
    c.moveTo(GUTTER, y + 0.5)
    c.lineTo(W - PAD_R, y + 0.5)
    c.stroke()
    const perPx = (t1 - t0) / plotW()
    const step = STEPS.find((s) => s / perPx >= 90) ?? STEPS[STEPS.length - 1]
    c.textAlign = 'center'
    c.textBaseline = 'top'
    c.fillStyle = faint
    // Ticks on local-time boundaries: midnights for days, month starts for
    // months, so a label never lands at 17:00 on a "day" tick.
    const ticks: { t: number; label: string }[] = []
    if (step >= 30 * 86400) {
      const months = Math.round(step / (30 * 86400))
      const d = new Date(t0 * 1000)
      d.setDate(1)
      d.setHours(0, 0, 0, 0)
      while (d.getTime() / 1000 <= t1) {
        if (d.getMonth() % months === 0) {
          const label = d.getMonth() === 0 ? String(d.getFullYear()) : MONTHS[d.getMonth()]
          ticks.push({ t: d.getTime() / 1000, label })
        }
        d.setMonth(d.getMonth() + 1)
      }
    } else if (step >= 86400) {
      const days = Math.round(step / 86400)
      const d = new Date(t0 * 1000)
      d.setHours(0, 0, 0, 0)
      while (d.getTime() / 1000 <= t1) {
        const n = Math.floor(d.getTime() / 86400000)
        if (n % days === 0) ticks.push({ t: d.getTime() / 1000, label: `${MONTHS[d.getMonth()]} ${d.getDate()}` })
        d.setDate(d.getDate() + 1)
      }
    } else {
      const hours = step / HOUR
      const d = new Date(t0 * 1000)
      d.setMinutes(0, 0, 0)
      while (d.getTime() / 1000 <= t1) {
        if (d.getHours() % hours === 0) {
          const label =
            d.getHours() === 0 ? `${MONTHS[d.getMonth()]} ${d.getDate()}` : `${String(d.getHours()).padStart(2, '0')}:00`
          ticks.push({ t: d.getTime() / 1000, label })
        }
        d.setHours(d.getHours() + 1)
      }
    }
    for (const tk of ticks) {
      const px = x(tk.t)
      if (px < GUTTER || px > W - PAD_R) continue
      c.strokeStyle = rule
      c.beginPath()
      c.moveTo(Math.round(px) + 0.5, PAD_T)
      c.lineTo(Math.round(px) + 0.5, y)
      c.stroke()
      c.fillText(tk.label, px, y + 7)
    }
  }

  // ---- picking ------------------------------------------------------------
  let hover = $state<number | null>(null)

  function pick(px: number, py: number): number | null {
    if (px < GUTTER || py > plotBottom() || py < PAD_T) return null
    const level = Math.max(0, tree.top - Math.floor((py - PAD_T) / unit()))
    if (level > tree.top) return null
    const [lo, hi] = ranges[level]
    if (level > 0) {
      // Windows on one level never overlap: binary search by start.
      const t = tAt(px)
      let a = lo
      let b = hi
      while (a < b) {
        const m = (a + b) >> 1
        if (tree.start[m] <= t) a = m + 1
        else b = m
      }
      const k = a - 1
      if (k >= lo && t < tree.end[k]) return k
      // A bar narrower than a pixel is still worth hitting: take the nearest
      // within a few pixels.
      return nearest(lo, hi, px, 4)
    }
    return nearest(lo, hi, px, 5)
  }

  function nearest(lo: number, hi: number, px: number, within: number): number | null {
    // Search a window of time around the cursor, not the whole level.
    const tA = tAt(px - within) - HOUR
    let a = lo
    let b = hi
    while (a < b) {
      const m = (a + b) >> 1
      if (tree.start[m] < tA) a = m + 1
      else b = m
    }
    let best: number | null = null
    let bestD = within
    for (let k = a; k < hi; k++) {
      const [xa, xb] = nodeX(k)
      if (xa > px + within) break
      const d = px < xa ? xa - px : px > xb ? px - xb : 0
      if (d <= bestD) {
        best = k
        bestD = d
      }
    }
    return best
  }

  // ---- tooltip ------------------------------------------------------------
  let tip = $state<{ k: number; x: number; y: number; text: string | null } | null>(null)
  let tipTimer: ReturnType<typeof setTimeout> | undefined

  function showTip(k: number | null, mx: number, my: number) {
    clearTimeout(tipTimer)
    if (k === null) {
      tip = null
      return
    }
    const text = tree.text[k]
    tip = { k, x: mx, y: my, text: text ?? (tree.level[k] === 0 ? null : '(not summarized yet)') }
    if (text === null && tree.level[k] === 0) {
      const id = tree.ids[k]
      tipTimer = setTimeout(async () => {
        try {
          const n = await fetchNode(id)
          if (tip && tip.k === k) tip = { ...tip, text: n.text ?? '(no text)' }
        } catch {
          if (tip && tip.k === k) tip = { ...tip, text: '(could not load)' }
        }
      }, 90)
    }
  }

  // ---- interaction ------------------------------------------------------------
  let drag = $state<{ x: number; t0: number; t1: number; moved: boolean } | null>(null)

  function local(e: MouseEvent) {
    const r = canvas.getBoundingClientRect()
    return [e.clientX - r.left, e.clientY - r.top]
  }

  function onmove(e: MouseEvent) {
    const [mx, my] = local(e)
    if (drag) {
      const dx = mx - drag.x
      if (Math.abs(dx) > 2) drag.moved = true
      const dt = (dx / plotW()) * (drag.t1 - drag.t0)
      t0 = drag.t0 - dt
      t1 = drag.t1 - dt
      tip = null
      return
    }
    const k = pick(mx, my)
    if (k !== hover) {
      hover = k
      showTip(k, mx, my)
    } else if (tip) {
      tip = { ...tip, x: mx, y: my }
    }
  }

  function ondown(e: MouseEvent) {
    const [mx] = local(e)
    drag = { x: mx, t0, t1, moved: false }
  }

  function onup(e: MouseEvent) {
    const d = drag
    drag = null
    if (!d || d.moved) return
    const [mx, my] = local(e)
    const k = pick(mx, my)
    onselect(k === null ? null : tree.ids[k])
  }

  function onleave() {
    drag = null
    hover = null
    showTip(null, 0, 0)
  }

  const MIN_SPAN = 2 * HOUR

  function onwheel(e: WheelEvent) {
    e.preventDefault()
    const [mx] = local(e)
    // A sideways swipe pans; anything vertical (wheel, pinch) zooms about the
    // cursor, so the moment under the pointer stays under the pointer.
    if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
      const dt = (e.deltaX / plotW()) * (t1 - t0)
      t0 += dt
      t1 += dt
      return
    }
    const anchor = tAt(Math.max(GUTTER, mx))
    const full = (tree.last - tree.first) * 1.2 + 2 * HOUR
    const z = Math.exp(e.deltaY * (e.ctrlKey ? 0.01 : 0.0025))
    const spanNow = t1 - t0
    const next = Math.min(full, Math.max(MIN_SPAN, spanNow * z))
    const k = next / spanNow
    t0 = anchor - (anchor - t0) * k
    t1 = anchor + (t1 - anchor) * k
  }

  /** Bring a node into view: centred if it fits, fitted if it is wider than
   *  the view. Animated briefly, so the eye can follow where it went. */
  export function reveal(id: string) {
    const k = index.get(id)
    if (k === undefined) return
    const a = tree.start[k]
    const b = Math.max(tree.end[k], a + HOUR)
    let n0 = t0
    let n1 = t1
    const spanNow = t1 - t0
    if (b - a > spanNow * 0.8) {
      const pad = (b - a) * 0.15
      n0 = a - pad
      n1 = b + pad
    } else if (a < t0 + spanNow * 0.05 || b > t1 - spanNow * 0.05) {
      const mid = (a + b) / 2
      n0 = mid - spanNow / 2
      n1 = mid + spanNow / 2
    } else return
    animate(n0, n1)
  }

  export function reset() {
    const pad = Math.max((tree.last - tree.first) * 0.015, HOUR)
    animate(tree.first - pad, tree.last + pad)
  }

  function animate(n0: number, n1: number) {
    const a0 = t0
    const a1 = t1
    const start = performance.now()
    const step = (now: number) => {
      const u = Math.min(1, (now - start) / 260)
      const e = 1 - (1 - u) ** 3
      t0 = a0 + (n0 - a0) * e
      t1 = a1 + (n1 - a1) * e
      if (u < 1) requestAnimationFrame(step)
    }
    requestAnimationFrame(step)
  }

  onMount(() => {
    const ro = new ResizeObserver(([e]) => {
      W = e.contentRect.width
      H = e.contentRect.height
    })
    ro.observe(wrap)
    canvas.addEventListener('wheel', onwheel, { passive: false })
    return () => {
      ro.disconnect()
      canvas.removeEventListener('wheel', onwheel)
    }
  })

  const visibleSpan = $derived(span(Math.max(1, Math.round((t1 - t0) / HOUR))))
</script>

<div class="wrap" bind:this={wrap}>
  <canvas
    bind:this={canvas}
    style="width:{W}px;height:{H}px"
    class:grab={!!drag}
    onmousemove={onmove}
    onmousedown={ondown}
    onmouseup={onup}
    onmouseleave={onleave}
    ondblclick={reset}
  ></canvas>
  <div class="hint">{visibleSpan} in view · scroll to zoom · drag to pan · double-click to fit</div>
  {#if tip}
    {@const k = tip.k}
    <div
      class="tip"
      class:flipx={tip.x > W * 0.6}
      class:flipy={tip.y > H * 0.55}
      style="--x:{tip.x}px;--y:{tip.y}px"
    >
      <div class="head">
        {levelName(tree.level[k])} · {when(tree.start[k])}{#if tree.level[k] > 0} → {when(tree.end[k])}{/if}
      </div>
      <div class="body">{tip.text ?? 'loading…'}</div>
    </div>
  {/if}
</div>

<style>
  .wrap { position: relative; width: 100%; height: 100%; overflow: hidden; }
  canvas { display: block; cursor: crosshair; }
  canvas.grab { cursor: grabbing; }
  .hint {
    position: absolute;
    right: 14px;
    top: 4px;
    font-size: 0.68rem;
    color: var(--ink-faint);
    pointer-events: none;
  }
  .tip {
    position: absolute;
    left: calc(var(--x) + 14px);
    top: calc(var(--y) + 14px);
    max-width: 28rem;
    background: var(--paper);
    border: 1px solid var(--ink);
    padding: 0.45rem 0.6rem;
    font-size: 0.76rem;
    pointer-events: none;
    box-shadow: 3px 3px 0 rgba(0, 0, 0, 0.08);
    z-index: 5;
  }
  .tip.flipx { left: auto; right: calc(100% - var(--x) + 14px); }
  .tip.flipy { top: auto; bottom: calc(100% - var(--y) + 14px); }
  .head { color: var(--ink-soft); font-size: 0.7rem; margin-bottom: 0.25rem; }
  .body {
    display: -webkit-box;
    -webkit-line-clamp: 9;
    line-clamp: 9;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
