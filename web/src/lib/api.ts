// Everything the page asks for, and the shapes it comes back in. One place,
// so a field the server renames breaks here, loudly, and not in a corner of
// a component.

export interface Soul {
  id: string
  name: string
  path: string
  last_sync: string | null
  last_error: string | null
  /** null when the store could not be read — not the same as zero. */
  memories: number | null
  summaries: number | null
  first: string | null
  last: string | null
  index_error: string | null
  progress: Progress | null
}

export interface Progress {
  turns: number
  turns_read: number
  memories: number
  summary_windows: number
  pending_windows: number
  done: boolean
}

export interface Tree {
  soul: string
  count: number
  top: number
  first: number
  last: number
  ids: string[]
  level: number[]
  start: number[]
  end: number[]
  text: (string | null)[]
  unreadable: number
}

export interface Line {
  id: string
  level: number
  from: string
  to: string
  text: string
  /** Startup view only: a window with no current summary yet. */
  pending?: boolean
  /** Startup view only: why raveld chose this line. 'age' is kept whole by
   *  the age rule, 'present' is leftover budget spent near now, 'opened' is
   *  shown because the window above has no current summary. */
  rule?: 'age' | 'present' | 'opened'
}

export interface Turn {
  turn: string
  ts: string | null
  role: string | null
  text: string | null
}

export interface NodeView {
  id: string
  level: number | null
  from: string | null
  to: string | null
  text: string | null
  model: string | null
  open:
    | { kind: 'window'; node: string; children: Line[] }
    | { kind: 'memory'; node: string; memory: { ts: string; turns: string[] }; turns: Turn[] }
    | { error: string }
}

export interface Hit {
  id: string
  level: number
  from: string
  text: string
}

export interface Wake {
  id: string
  memories: number
  summaries: number
  /** Total characters of text in the view. */
  chars: number
  lines: Line[]
}

export interface Health {
  raveld: {
    ok: boolean
    version: string
    souls: string[]
    uptime_secs: number
    memory_state?: 'off' | 'on' | 'paused'
    memory_paused_reason?: string | null
  } | null
  raveld_url: string
  error?: string
}

async function json<T>(url: string): Promise<T> {
  const r = await fetch(url)
  if (!r.ok) throw new Error(await r.text())
  return r.json()
}

export const getHealth = () => json<Health>('/api/health')
export const getSouls = () => json<Soul[]>('/api/souls')
export const getTree = (id: string) => json<Tree>(`/api/souls/${id}/tree`)
export const getNode = (id: string, node: string) => json<NodeView>(`/api/souls/${id}/node/${node}`)
export const getWake = (id: string) => json<Wake>(`/api/souls/${id}/wake`)
export const search = (id: string, q: string) =>
  json<{ query: string; hits: Hit[]; truncated: boolean }>(
    `/api/souls/${id}/search?q=${encodeURIComponent(q)}`,
  )
