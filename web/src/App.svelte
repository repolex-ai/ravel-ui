<script lang="ts">
  import { onMount } from 'svelte'
  import LeftRail from './lib/LeftRail.svelte'
  import Timeline from './lib/Timeline.svelte'
  import Inspector from './lib/Inspector.svelte'
  import { getSouls, getHealth, getTree, getNode, search as runSearch } from './lib/api'
  import type { Soul, Tree, NodeView, Health, Hit } from './lib/api'
  import { count, day } from './lib/format'

  let souls = $state<Soul[]>([])
  let soulsError = $state<string | null>(null)
  let health = $state<Health | null>(null)
  let current = $state<Soul | null>(null)
  let tree = $state<Tree | null>(null)
  let treeError = $state<string | null>(null)
  let loadingTree = $state(false)
  let selected = $state<string | null>(null)
  let node = $state<NodeView | null>(null)
  let nodeError = $state<string | null>(null)
  let loadingNode = $state(false)
  let query = $state('')
  let hits = $state<Hit[]>([])
  let truncated = $state(false)
  let searching = $state(false)
  let timeline = $state<ReturnType<typeof Timeline> | null>(null)

  const hitSet = $derived(new Set(hits.map((h) => h.id)))

  // One cache per soul: hovering a tick and then clicking it asks once.
  let cache = new Map<string, Promise<NodeView>>()
  function fetchNode(id: string): Promise<NodeView> {
    const soul = current?.id
    if (!soul) return Promise.reject(new Error('no soul'))
    let p = cache.get(id)
    if (!p) {
      p = getNode(soul, id)
      p.catch(() => cache.delete(id))
      cache.set(id, p)
    }
    return p
  }

  // The address bar carries the soul and the selection, so a view of a
  // memory is something you can send someone: #<soul>/<node>.
  function writeHash() {
    const h = current ? `#${current.id}${selected ? '/' + selected : ''}` : ''
    if (location.hash !== h) history.replaceState(null, '', `${location.pathname}${location.search}${h}`)
  }
  function readHash(): [string | null, string | null] {
    const [s, n] = location.hash.replace(/^#/, '').split('/')
    return [s || null, n || null]
  }

  async function pick(s: Soul, keep: string | null = null) {
    if (current?.id === s.id && tree) return
    current = s
    tree = null
    treeError = null
    selected = null
    node = null
    hits = []
    query = ''
    cache = new Map()
    loadingTree = true
    writeHash()
    try {
      const t = await getTree(s.id)
      if (current?.id !== s.id) return
      tree = t
      if (keep && t.ids.includes(keep)) {
        select(keep)
        queueMicrotask(() => timeline?.reveal(keep))
      }
    } catch (e) {
      if (current?.id === s.id) treeError = (e as Error).message
    } finally {
      if (current?.id === s.id) loadingTree = false
    }
  }

  async function select(id: string | null) {
    selected = id
    node = null
    nodeError = null
    writeHash()
    if (!id) return
    loadingNode = true
    try {
      const n = await fetchNode(id)
      if (selected === id) node = n
    } catch (e) {
      if (selected === id) nodeError = (e as Error).message
    } finally {
      if (selected === id) loadingNode = false
    }
  }

  function selectAndReveal(id: string) {
    select(id)
    timeline?.reveal(id)
  }

  let searchSeq = 0
  async function onsearch(q: string) {
    query = q.trim()
    const soul = current?.id
    if (!soul || query.length < 2) {
      hits = []
      truncated = false
      return
    }
    const seq = ++searchSeq
    searching = true
    try {
      const r = await runSearch(soul, query)
      if (seq !== searchSeq) return
      hits = r.hits
      truncated = r.truncated
    } catch {
      if (seq === searchSeq) hits = []
    } finally {
      if (seq === searchSeq) searching = false
    }
  }

  function onkey(e: KeyboardEvent) {
    if ((e.target as HTMLElement)?.tagName === 'INPUT') return
    if (e.key === 'Escape') select(null)
  }

  onMount(() => {
    const poll = async () => {
      try {
        health = await getHealth()
      } catch (e) {
        health = { raveld: null, raveld_url: '', error: (e as Error).message }
      }
    }
    poll()
    const iv = setInterval(poll, 15000)
    ;(async () => {
      try {
        souls = await getSouls()
      } catch (e) {
        soulsError = (e as Error).message
        return
      }
      const [s, n] = readHash()
      const want = souls.find((x) => x.id === s) ?? souls.find((x) => (x.memories ?? 0) > 0)
      if (want) pick(want, s === want.id ? n : null)
    })()
    return () => clearInterval(iv)
  })
</script>

<svelte:window onkeydown={onkey} />

<div class="page">
  <header class="top">
    <span class="brand">ravel</span>
    {#if current}
      <span class="soulname">{current.name}</span>
      <span class="meta">{current.path}</span>
      {#if tree}
        <span class="meta right">
          {count(current.memories)} memories · {count(current.summaries)} summaries · {day(current.first)} → {day(
            current.last,
          )}
        </span>
      {/if}
    {/if}
  </header>

  <LeftRail
    {souls}
    {health}
    {current}
    {query}
    {hits}
    {truncated}
    {searching}
    {selected}
    onpick={(s) => pick(s)}
    {onsearch}
    onhit={(h) => selectAndReveal(h.id)}
  />

  <main>
    {#if soulsError}
      <p class="state warn">Could not list souls: {soulsError}</p>
    {:else if treeError}
      <p class="state warn">{treeError}</p>
    {:else if loadingTree}
      <p class="state">reading {current?.name}'s memory tree…</p>
    {:else if tree}
      <Timeline bind:this={timeline} {tree} {selected} hits={hitSet} onselect={select} {fetchNode} />
    {:else}
      <p class="state">pick a soul</p>
    {/if}
  </main>

  <Inspector
    {tree}
    {selected}
    {node}
    loading={loadingNode}
    error={nodeError}
    onselect={selectAndReveal}
  />
</div>

<style>
  .page {
    display: grid;
    grid-template-columns: 15rem 1fr 26rem;
    grid-template-rows: auto 1fr;
    height: 100vh;
  }
  .top {
    grid-column: 1 / -1;
    display: flex;
    align-items: baseline;
    gap: 0.9rem;
    padding: 0.45rem 0.9rem;
    border-bottom: 1px solid var(--rule-strong);
    white-space: nowrap;
    overflow: hidden;
  }
  .brand { font-family: var(--display); font-size: 1.05rem; }
  .soulname { font-family: var(--display); font-size: 1.05rem; }
  .meta { font-size: 0.75rem; color: var(--ink-faint); overflow: hidden; text-overflow: ellipsis; }
  .right { margin-left: auto; color: var(--ink-soft); }
  main { position: relative; min-width: 0; min-height: 0; }
  .state { padding: 2rem; color: var(--ink-soft); }
  .warn { color: var(--warn); white-space: pre-wrap; }
</style>
