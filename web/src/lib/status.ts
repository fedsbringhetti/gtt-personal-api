/** Response of `GET /api/status` (see docs/user/api.md). Times are ISO 8601 strings. */
export interface Status {
  version: string
  time: string
  upstream: {
    ok: boolean | null
    last_success: string | null
    last_failure: string | null
    last_error: string | null
    consecutive_failures: number
  }
  stop_index: { synced_at: string; count: number } | null
}

export async function fetchStatus(): Promise<Status> {
  const response = await fetch('/api/status')
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}`)
  }
  return response.json()
}

/** One line for the placeholder page, e.g. "Server 0.1.0 · 7.058 fermate". */
export function describeStatus(status: Status): string {
  const stops = status.stop_index
    ? // Italian rules group only from five digits ("7058"): force "7.058" for a steady layout.
      `${status.stop_index.count.toLocaleString('it-IT', { useGrouping: 'always' })} fermate`
    : 'fermate non ancora scaricate'
  return `Server ${status.version} · ${stops}`
}
