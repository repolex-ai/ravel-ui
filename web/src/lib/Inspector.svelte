<script lang="ts">
  // The fixed right-hand panel: what the selected node says, where it sits in
  // the tree, and what it was made from. For a summary, that is the windows
  // or memories it summarizes. For a memory, it is the conversation turns it
  // was read from — the step that makes a memory checkable rather than
  // something to take on trust.
  import type { Tree, NodeView, Wake } from './api'
  import { levelName, when } from './format'

  interface Props {
    tree: Tree | null
    wake: Wake | null
    selected: string | null
    node: NodeView | null
    loading: boolean
    error: string | null
    onselect: (id: string) => void
  }
  let { tree, wake, selected, node, loading, error, onselect }: Props = $props()

  /** Where the startup view's size goes, by level. Characters, because that
   *  is what the soul reads; tokens are an estimate at four characters each,
   *  and labelled as one. */
  const wakeByLevel = $derived.by(() => {
    if (!wake) return []
    const by = new Map<number, { lines: number; chars: number; pending: number }>()
    for (const l of wake.lines) {
      const e = by.get(l.level) ?? { lines: 0, chars: 0, pending: 0 }
      e.lines++
      e.chars += [...l.text].length
      if (l.text.includes('not summarized')) e.pending++
      by.set(l.level, e)
    }
    return [...by.entries()].sort((a, b) => b[0] - a[0]).map(([level, e]) => ({ level, ...e }))
  })
  const wakePending = $derived(wakeByLevel.reduce((n, r) => n + r.pending, 0))

  /** The windows above the selection, nearest first, found by time: the
   *  window on each level whose span contains the selection's start. */
  const ancestors = $derived.by(() => {
    if (!tree || !selected) return []
    const k = tree.ids.indexOf(selected)
    if (k < 0) return []
    const out: { id: string; level: number; text: string | null }[] = []
    const t = tree.start[k]
    for (let i = 0; i < tree.count; i++) {
      if (tree.level[i] > tree.level[k] && tree.start[i] <= t && t < tree.end[i]) {
        out.push({ id: tree.ids[i], level: tree.level[i], text: tree.text[i] })
      }
    }
    return out.sort((a, b) => a.level - b.level)
  })

  let open = $state<Record<string, boolean>>({})
</script>

<aside>
  {#if !tree}
    <p class="empty">pick a soul on the left</p>
  {:else if !selected}
    <div class="intro">
      <h2 class="label">{tree.count.toLocaleString('en-US')} nodes</h2>
      <p>
        Bottom row: single memories, one tick each, in the hour they happened. Each row above summarizes
        windows twice as long as the row below.
      </p>
      <p>Click any tick or bar to read it. A memory opens to the conversation turns it was made from.</p>
      {#if wake}
        <h3 class="label">wakes up with</h3>
        <p class="wakehead">
          {wake.lines.length} lines · {wake.chars.toLocaleString('en-US')} characters ·
          <span title="estimated at four characters per token">≈{Math.round(wake.chars / 4).toLocaleString('en-US')} tokens</span>
        </p>
        <table class="wake">
          <tbody>
            {#each wakeByLevel as r (r.level)}
              <tr>
                <td>{levelName(r.level)}</td>
                <td class="num">{r.lines}</td>
                <td class="barcell">
                  <span class="bar" style="width:{(100 * r.chars) / Math.max(1, wake.chars)}%"></span>
                </td>
                <td class="num">{Math.round((100 * r.chars) / Math.max(1, wake.chars))}%</td>
              </tr>
            {/each}
          </tbody>
        </table>
        {#if wakePending}
          <p class="warn">
            {wakePending} line{wakePending === 1 ? '' : 's'} not summarized yet — drawn outlined on the timeline.
          </p>
        {/if}
        <p>This is what <code>ravel memory</code> prints when the soul starts: fine detail near now, coarser summaries further back. On the timeline it is coloured from ochre (oldest) to green (now).</p>
      {/if}
      {#if tree.unreadable}
        <p class="warn">{tree.unreadable} nodes could not be read and are not drawn.</p>
      {/if}
    </div>
  {:else}
    <header>
      <h2 class="label">{node?.level != null ? levelName(node.level) : 'selected'}</h2>
      <code class="id">{selected}</code>
      {#if node}
        <p class="dates">
          {#if node.level === 0}{when(node.from)}{:else}{when(node.from)} → {when(node.to)}{/if}
          {#if node.model}<span class="model"> · {node.model}</span>{/if}
        </p>
      {/if}
    </header>

    {#if error}
      <p class="warn">{error}</p>
    {:else if loading && !node}
      <p class="empty">loading…</p>
    {:else if node}
      <section class="text">
        {#if node.text}
          <p>{node.text}</p>
        {:else}
          <p class="empty">Not summarized yet. Its parts are below.</p>
        {/if}
      </section>

      {#if ancestors.length}
        <section>
          <h3 class="label">inside</h3>
          <ol class="list">
            {#each ancestors as a (a.id)}
              <li>
                <button onclick={() => onselect(a.id)}>
                  <span class="sub">{levelName(a.level)}</span>
                  <span class="snip">{a.text ?? '(not summarized yet)'}</span>
                </button>
              </li>
            {/each}
          </ol>
        </section>
      {/if}

      {#if 'error' in node.open}
        <p class="warn">{node.open.error}</p>
      {:else if node.open.kind === 'window'}
        <section>
          <h3 class="label">summarizes {node.open.children.length}</h3>
          <ol class="list">
            {#each node.open.children as c (c.id)}
              <li>
                <button onclick={() => onselect(c.id)}>
                  <span class="sub">{levelName(c.level)} · {when(c.from)}</span>
                  <span class="snip">{c.text}</span>
                </button>
              </li>
            {/each}
          </ol>
        </section>
      {:else}
        <section>
          <h3 class="label">made from {node.open.turns.length} turn{node.open.turns.length === 1 ? '' : 's'}</h3>
          <ol class="turns">
            {#each node.open.turns as t (t.turn)}
              <li class:user={t.role === 'user'}>
                <div class="sub">{t.role ?? '?'} · {when(t.ts)}</div>
                {#if t.text}
                  <div class="turn" class:clamped={!open[t.turn]}>{t.text}</div>
                  {#if t.text.length > 600}
                    <button class="more" onclick={() => (open = { ...open, [t.turn]: !open[t.turn] })}>
                      {open[t.turn] ? 'less' : 'more'}
                    </button>
                  {/if}
                {:else}
                  <div class="empty">(no text — a tool call or result)</div>
                {/if}
              </li>
            {/each}
          </ol>
        </section>
      {/if}
    {/if}
  {/if}
</aside>

<style>
  aside {
    border-left: 1px solid var(--rule);
    overflow-y: auto;
    min-height: 0;
    padding: 0.8rem 0.9rem 2rem;
  }
  .label {
    font-family: var(--mono);
    font-size: 0.72rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-soft);
    margin: 0 0 0.4rem;
  }
  h3.label { margin-top: 1.1rem; }
  .empty { color: var(--ink-faint); font-size: 0.8rem; }
  .warn { color: var(--warn); font-size: 0.8rem; }
  .intro p { font-size: 0.82rem; color: var(--ink-soft); }
  .intro .wakehead { color: var(--ink); margin: 0 0 0.4rem; }
  .wake { width: 100%; border-collapse: collapse; font-size: 0.74rem; }
  .wake td { padding: 0.12rem 0.3rem 0.12rem 0; border-bottom: 1px solid var(--rule); white-space: nowrap; }
  .wake .num { text-align: right; color: var(--ink-soft); }
  .barcell { width: 45%; }
  .bar { display: block; height: 0.5rem; background: linear-gradient(90deg, #bd763a, #5fc05a); }
  .id { font-size: 0.72rem; color: var(--ink-faint); }
  .dates { margin: 0.2rem 0 0; font-size: 0.8rem; }
  .model { color: var(--ink-faint); }
  .text p {
    font-family: var(--display);
    font-size: 1.02rem;
    line-height: 1.5;
    margin: 0.7rem 0 0;
  }
  .list, .turns { list-style: none; margin: 0; padding: 0; }
  .list button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    text-align: left;
    border: 0;
    border-bottom: 1px solid var(--rule);
    padding: 0.35rem 0.1rem;
    background: none;
  }
  .list button:hover { background: var(--paper-tint); color: var(--ink); }
  .sub { font-size: 0.7rem; color: var(--ink-faint); }
  .snip {
    font-size: 0.78rem;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .turns li { border-left: 2px solid var(--rule); padding: 0.2rem 0 0.5rem 0.6rem; margin-bottom: 0.4rem; }
  .turns li.user { border-left-color: var(--ink); }
  .turn { white-space: pre-wrap; font-size: 0.78rem; overflow-wrap: anywhere; }
  .turn.clamped { max-height: 9.5rem; overflow: hidden; mask-image: linear-gradient(#000 70%, transparent); }
  .more { border: 0; padding: 0; font-size: 0.72rem; text-decoration: underline; background: none; }
  .more:hover { background: none; color: var(--ink); }
</style>
