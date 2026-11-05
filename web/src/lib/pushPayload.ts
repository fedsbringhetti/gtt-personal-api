/**
 * What the service worker reads from a push. Pure, so it is tested without a worker. The server
 * sends `{title, body, url, tag?}` (`url` is a path inside the app). A payload that cannot be
 * read still gives a generic notification: a push must always show one (`userVisibleOnly`).
 */
export interface PushPayload {
  title: string
  body: string
  url: string
  tag?: string
}

/** `read` is the payload's JSON reader (`event.data?.json()`); it may throw. */
export function readPayload(read: () => unknown): PushPayload {
  try {
    const data = read() as Partial<PushPayload> | null | undefined
    return {
      title: typeof data?.title === 'string' ? data.title : 'InTime',
      body: typeof data?.body === 'string' ? data.body : '',
      // Only a path inside the app: `//evil.com` is a link out of it.
      url:
        typeof data?.url === 'string' && data.url.startsWith('/') && !data.url.startsWith('//')
          ? data.url
          : '/',
      tag: typeof data?.tag === 'string' ? data.tag : undefined,
    }
  } catch {
    return { title: 'InTime', body: '', url: '/' }
  }
}
