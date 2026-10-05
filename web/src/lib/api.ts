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

export interface Health {
  raveld: {
    ok: boolean
    version: string
    memory: string
    souls: string[]
    uptime_secs: number
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
export const search = (id: string, q: string) =>
  json<{ query: string; hits: Hit[]; truncated: boolean }>(
    `/api/souls/${id}/search?q=${encodeURIComponent(q)}`,
  )
