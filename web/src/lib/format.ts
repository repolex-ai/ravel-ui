/** What a level means, in hours of conversation. Level 0 is one memory;
 *  level k+1 summarizes a window of 2^k hours (ravel's memory index). */
export function levelName(level: number): string {
  if (level === 0) return 'memory'
  return `${span(2 ** (level - 1))} summary`
}

export function span(hours: number): string {
  if (hours < 48) return `${hours}h`
  const d = hours / 24
  return `${d >= 10 ? Math.round(d) : d.toFixed(1).replace(/\.0$/, '')}d`
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

/** "Apr 3 18:00" — local time, because these are hours a person lived. */
export function when(iso: string | number | null): string {
  if (iso === null || iso === '') return '—'
  const d = typeof iso === 'number' ? new Date(iso * 1000) : new Date(iso)
  if (isNaN(d.getTime())) return String(iso)
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  return `${MONTHS[d.getMonth()]} ${d.getDate()} ${hh}:${mm}`
}

export function day(iso: string | null): string {
  if (!iso) return '—'
  const d = new Date(iso)
  if (isNaN(d.getTime())) return iso
  return `${MONTHS[d.getMonth()]} ${d.getDate()}${d.getFullYear() !== new Date().getFullYear() ? ` ${d.getFullYear()}` : ''}`
}

export function count(n: number | null): string {
  return n === null ? '?' : n.toLocaleString('en-US')
}

export function ago(iso: string | null): string {
  if (!iso) return 'never'
  const s = (Date.now() - new Date(iso).getTime()) / 1000
  if (s < 90) return 'just now'
  if (s < 5400) return `${Math.round(s / 60)}m ago`
  if (s < 172800) return `${Math.round(s / 3600)}h ago`
  return `${Math.round(s / 86400)}d ago`
}
