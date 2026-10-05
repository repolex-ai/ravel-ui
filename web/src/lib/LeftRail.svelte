<script lang="ts">
  import type { Soul, Hit, Health } from './api'
  import { count, day, ago, levelName } from './format'

  interface Props {
    souls: Soul[]
    health: Health | null
    current: Soul | null
    query: string
    hits: Hit[]
    truncated: boolean
    searching: boolean
    selected: string | null
    onpick: (s: Soul) => void
    onsearch: (q: string) => void
    onhit: (h: Hit) => void
  }
  let { souls, health, current, query, hits, truncated, searching, selected, onpick, onsearch, onhit }: Props =
    $props()

  // Three groups, because they are three different answers. A soul with an
  // index can be drawn. A soul without one is real but not processed yet. The
  // DEMOlishous souls were props for one demo in April 2026 and have no
  // history of their own; listing them with the others would read as four
  // colleagues who never said anything.
  const isDemo = (s: Soul) => s.path.includes('/DEMOlishous/')
  const groups = $derived([
    {
      key: 'indexed',
      title: 'Indexed',
      souls: souls.filter((s) => !isDemo(s) && (s.memories ?? 0) > 0).sort((a, b) => (b.memories ?? 0) - (a.memories ?? 0)),
    },
    {
      key: 'pending',
      title: 'Not indexed yet',
      souls: souls.filter((s) => !isDemo(s) && (s.memories ?? 0) === 0),
    },
    { key: 'demo', title: 'Demo props', souls: souls.filter(isDemo) },
  ])

  let folded = $state<Record<string, boolean>>(load())
  function load(): Record<string, boolean> {
    try {
      return JSON.parse(localStorage.getItem('ravel-ui.folded') ?? '') ?? { demo: true }
    } catch {
      return { demo: true }
    }
  }
  function fold(k: string) {
    folded = { ...folded, [k]: !folded[k] }
    try {
      localStorage.setItem('ravel-ui.folded', JSON.stringify(folded))
    } catch {}
  }

  let draft = $state('')
  $effect(() => {
    draft = query
  })
  let timer: ReturnType<typeof setTimeout> | undefined
  function typed(v: string) {
    draft = v
    clearTimeout(timer)
    timer = setTimeout(() => onsearch(v), 250)
  }
</script>

<aside>
  <section class="souls">
    <h2 class="label">souls {souls.length}</h2>
    {#each groups as g (g.key)}
      {#if g.souls.length}
        <button class="group" onclick={() => fold(g.key)} aria-expanded={!folded[g.key]}>
          <span>{folded[g.key] ? '▸' : '▾'} {g.title}</span><span>{g.souls.length}</span>
        </button>
        {#if !folded[g.key]}
          <ul>
            {#each g.souls as s (s.id)}
              <li>
                <button
                  class="soul"
                  class:on={current?.id === s.id}
                  disabled={!s.memories}
                  title={s.index_error ?? s.path}
                  onclick={() => onpick(s)}
                >
                  <span class="name">{s.name}</span>
                  {#if s.memories}
                    <span class="sub">{count(s.memories)} memories · {count(s.summaries)} summaries</span>
                    <span class="sub">{day(s.first)} → {day(s.last)}</span>
                  {:else if s.index_error}
                    <span class="sub warn">store unreadable</span>
                  {:else}
                    <span class="sub">synced {ago(s.last_sync)}</span>
                  {/if}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    {/each}
  </section>

  <section class="find">
    <h2 class="label">find in {current?.name ?? '—'}</h2>
    <input
      type="search"
      placeholder="words in a memory…"
      value={draft}
      disabled={!current}
      oninput={(e) => typed((e.target as HTMLInputElement).value)}
    />
    {#if query.length >= 2}
      <p class="note">
        {#if searching}searching…{:else}{hits.length}{truncated ? '+' : ''} match{hits.length === 1 ? '' : 'es'}{/if}
      </p>
      <ol class="hits">
        {#each hits as h (h.id)}
          <li>
            <button class:on={selected === h.id} onclick={() => onhit(h)}>
              <span class="sub">{day(h.from)} · {levelName(h.level)}</span>
              <span class="snip">{h.text}</span>
            </button>
          </li>
        {/each}
      </ol>
    {/if}
  </section>

  <footer>
    {#if health?.raveld}
      <div><span class="dot live"></span>raveld {health.raveld.version} · {health.raveld_url.replace('http://', '')}</div>
    {:else}
      <div><span class="dot dead"></span>raveld not answering</div>
      <div class="sub">{health?.error ?? 'checking…'}</div>
    {/if}
  </footer>
</aside>

<style>
  aside {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--rule);
    overflow: hidden;
  }
  section { padding: 0.8rem 0.75rem; border-bottom: 1px solid var(--rule); }
  .souls { overflow-y: auto; flex: 0 1 auto; max-height: 55%; }
  .find { flex: 1 1 auto; overflow-y: auto; min-height: 8rem; }
  .label {
    font-family: var(--mono);
    font-size: 0.72rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-soft);
    margin-bottom: 0.5rem;
  }
  ul, ol { list-style: none; margin: 0; padding: 0; }
  .group {
    display: flex;
    justify-content: space-between;
    width: 100%;
    border: 0;
    border-bottom: 1px solid var(--rule);
    padding: 0.35rem 0.1rem;
    margin-top: 0.3rem;
    font-size: 0.68rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink-soft);
    background: none;
  }
  .group:hover { background: none; color: var(--ink); }
  .soul {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    border: 0;
    padding: 0.4rem 0.45rem;
    text-align: left;
    background: none;
  }
  .soul .name { font-family: var(--display); font-size: 1rem; }
  .soul:hover:not(:disabled) { background: var(--paper-tint); color: var(--ink); }
  .soul.on, .soul.on:hover { background: var(--ink); color: var(--paper); }
  .soul.on .sub { color: #bbb; }
  .soul:disabled { opacity: 0.55; cursor: default; }
  .sub { font-size: 0.72rem; color: var(--ink-faint); line-height: 1.35; }
  .warn { color: var(--warn); }
  input {
    width: 100%;
    font: inherit;
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink);
  }
  .note { font-size: 0.72rem; color: var(--ink-soft); margin: 0.4rem 0; }
  .hits button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    text-align: left;
    border: 0;
    border-bottom: 1px solid var(--rule);
    padding: 0.35rem 0.2rem;
    background: none;
  }
  .hits button:hover { background: var(--paper-tint); color: var(--ink); }
  .hits button.on { background: var(--paper-tint); box-shadow: inset 3px 0 0 var(--hit); }
  .snip {
    font-size: 0.76rem;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  footer {
    margin-top: auto;
    padding: 0.55rem 0.75rem;
    font-size: 0.72rem;
    color: var(--ink-soft);
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    border-top: 1px solid var(--rule);
  }
  .dot { display: inline-block; width: 0.5rem; height: 0.5rem; border-radius: 50%; margin-right: 0.3rem; }
  .dot.live { background: var(--live); }
  .dot.dead { background: var(--dead); }
</style>
