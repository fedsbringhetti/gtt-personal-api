import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const api = vi.hoisted(() => ({
  getPushKey: vi.fn(),
  savePushSubscription: vi.fn(),
  deletePushSubscription: vi.fn(),
}))
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), ...api }))

import { ApiError } from './api'
import {
  enablePush,
  isIos,
  loadPush,
  pushState,
  sameKey,
  subscriptionBody,
  urlBase64ToUint8Array,
  type PushInputs,
} from './push'

const ready: PushInputs = {
  ios: false,
  standalone: false,
  supported: true,
  serverEnabled: true,
  permission: 'default',
  subscribed: false,
}

describe('pushState', () => {
  it('is off until the user turns notifications on', () => {
    expect(pushState(ready)).toBe('off')
  })

  it('is on with permission and a subscription', () => {
    expect(pushState({ ...ready, permission: 'granted', subscribed: true })).toBe('on')
  })

  it('is off when permission is granted but nothing is subscribed', () => {
    expect(pushState({ ...ready, permission: 'granted' })).toBe('off')
  })

  it('is denied when the browser blocks notifications', () => {
    expect(pushState({ ...ready, permission: 'denied' })).toBe('denied')
  })

  it('tells an iPhone outside the Home Screen to install, not that it is unsupported', () => {
    expect(pushState({ ...ready, ios: true, supported: false })).toBe('ios-home-screen')
    expect(pushState({ ...ready, ios: true, standalone: true, supported: false })).toBe('unsupported')
    expect(pushState({ ...ready, ios: true, standalone: true })).toBe('off')
  })

  it('is unsupported without the browser features', () => {
    expect(pushState({ ...ready, supported: false })).toBe('unsupported')
  })

  it('is server-disabled when the server has no key', () => {
    expect(pushState({ ...ready, serverEnabled: false })).toBe('server-disabled')
  })
})

describe('isIos', () => {
  it('recognizes iPhone and iPad', () => {
    expect(isIos('Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)', 5)).toBe(true)
    expect(isIos('Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X)', 5)).toBe(true)
  })

  it('recognizes an iPad that presents itself as a Mac, but not a Mac', () => {
    const mac = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)'
    expect(isIos(mac, 5)).toBe(true)
    expect(isIos(mac, 0)).toBe(false)
  })

  it('does not take Android for iOS', () => {
    expect(isIos('Mozilla/5.0 (Linux; Android 15; Pixel 9)', 5)).toBe(false)
  })
})

describe('urlBase64ToUint8Array', () => {
  it('decodes base64url without padding', () => {
    // "-_" are "+/" in base64url: 0xfb 0xff.
    expect([...urlBase64ToUint8Array('-_8')]).toEqual([0xfb, 0xff])
    expect([...urlBase64ToUint8Array('AQID')]).toEqual([1, 2, 3])
    expect([...urlBase64ToUint8Array('AQI')]).toEqual([1, 2])
  })

  it('decodes a 65 byte key', () => {
    const key = new Uint8Array(65).map((_, i) => i * 3)
    const text = btoa(String.fromCharCode(...key))
      .replace(/\+/g, '-')
      .replace(/\//g, '_')
      .replace(/=+$/, '')
    expect([...urlBase64ToUint8Array(text)]).toEqual([...key])
  })
})

describe('sameKey', () => {
  const expected = new Uint8Array([1, 2, 3])

  it('compares the bytes', () => {
    expect(sameKey(new Uint8Array([1, 2, 3]).buffer, expected)).toBe(true)
    expect(sameKey(new Uint8Array([1, 2, 4]).buffer, expected)).toBe(false)
    expect(sameKey(new Uint8Array([1, 2]).buffer, expected)).toBe(false)
  })

  it('is false when the subscription has no key', () => {
    expect(sameKey(null, expected)).toBe(false)
    expect(sameKey(undefined, expected)).toBe(false)
  })
})

describe('subscriptionBody', () => {
  it('keeps what the server takes', () => {
    expect(
      subscriptionBody({ endpoint: 'https://e', expirationTime: null, keys: { p256dh: 'p', auth: 'a' } }),
    ).toEqual({ endpoint: 'https://e', expirationTime: null, keys: { p256dh: 'p', auth: 'a' } })
  })

  it('refuses an incomplete subscription', () => {
    expect(() => subscriptionBody({ endpoint: 'https://e' })).toThrow()
    expect(() => subscriptionBody({ endpoint: 'https://e', keys: { p256dh: 'p' } })).toThrow()
    expect(() => subscriptionBody({ keys: { p256dh: 'p', auth: 'a' } })).toThrow()
  })
})

// ---- loadPush and enablePush, with the browser stubbed. ----

const KEY_TEXT = 'AQID' // bytes 1, 2, 3
const KEY = new Uint8Array([1, 2, 3])
const buffer = (...bytes: number[]) => new Uint8Array(bytes).buffer

function fakeSubscription(endpoint: string, applicationServerKey: ArrayBuffer | null | undefined) {
  return {
    endpoint,
    options: { applicationServerKey },
    toJSON: () => ({ endpoint, expirationTime: null, keys: { p256dh: 'p', auth: 'a' } }),
    unsubscribe: vi.fn().mockResolvedValue(true),
  }
}

type Sub = ReturnType<typeof fakeSubscription>

function setup(opts: { permission?: NotificationPermission; current?: Sub | null; active?: boolean }) {
  const pushManager = {
    getSubscription: vi.fn().mockResolvedValue(opts.current ?? null),
    subscribe: vi.fn().mockResolvedValue(fakeSubscription('https://new', buffer(1, 2, 3))),
  }
  const reg = { active: opts.active === false ? null : {}, pushManager }
  const serviceWorker = {
    getRegistration: vi.fn().mockResolvedValue(reg),
    ready: Promise.resolve({ active: {}, pushManager }),
  }
  vi.stubGlobal('navigator', { userAgent: 'Chrome', maxTouchPoints: 0, serviceWorker })
  const notification = {
    permission: opts.permission ?? 'granted',
    requestPermission: vi.fn().mockResolvedValue(opts.permission ?? 'granted'),
  }
  vi.stubGlobal('Notification', notification)
  vi.stubGlobal('matchMedia', () => ({ matches: false }))
  vi.stubGlobal('PushManager', class {})
  // `'PushManager' in window` looks at the global object.
  vi.stubGlobal('window', globalThis)
  return { pushManager, serviceWorker, notification }
}

beforeEach(() => {
  api.getPushKey.mockReset().mockResolvedValue(KEY_TEXT)
  api.savePushSubscription.mockReset().mockResolvedValue(undefined)
  api.deletePushSubscription.mockReset().mockResolvedValue(undefined)
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('loadPush', () => {
  it('sends an existing subscription to the server again', async () => {
    const sub = fakeSubscription('https://e', buffer(1, 2, 3))
    const { pushManager } = setup({ current: sub })
    const status = await loadPush()
    expect(status.state).toBe('on')
    expect(api.savePushSubscription).toHaveBeenCalledWith({
      endpoint: 'https://e',
      expirationTime: null,
      keys: { p256dh: 'p', auth: 'a' },
    })
    expect(sub.unsubscribe).not.toHaveBeenCalled()
    expect(pushManager.subscribe).not.toHaveBeenCalled()
  })

  it('keeps a subscription whose key the browser does not report', async () => {
    for (const reported of [null, undefined]) {
      const sub = fakeSubscription('https://e', reported)
      const { pushManager } = setup({ current: sub })
      api.savePushSubscription.mockClear()
      expect((await loadPush()).state).toBe('on')
      expect(sub.unsubscribe).not.toHaveBeenCalled()
      expect(pushManager.subscribe).not.toHaveBeenCalled()
      expect(api.savePushSubscription).toHaveBeenCalledTimes(1)
    }
  })

  it('replaces a subscription made with another key', async () => {
    const sub = fakeSubscription('https://old', buffer(9, 9, 9))
    const { pushManager } = setup({ current: sub })
    const status = await loadPush()
    expect(status.state).toBe('on')
    expect(sub.unsubscribe).toHaveBeenCalled()
    expect(api.deletePushSubscription).toHaveBeenCalledWith('https://old')
    expect(pushManager.subscribe).toHaveBeenCalledWith({ userVisibleOnly: true, applicationServerKey: KEY })
    expect(api.savePushSubscription).toHaveBeenCalledWith(expect.objectContaining({ endpoint: 'https://new' }))
  })

  it('is off, not an error, when subscribing again fails', async () => {
    const sub = fakeSubscription('https://old', buffer(9, 9, 9))
    const { pushManager } = setup({ current: sub })
    pushManager.subscribe.mockRejectedValue(new Error('push service down'))
    const status = await loadPush()
    expect(status.state).toBe('off')
    expect(status.key).toEqual(KEY)
    expect(api.savePushSubscription).not.toHaveBeenCalled()
  })

  it('does not ask for permission nor subscribe when permission is not granted', async () => {
    const { pushManager, notification } = setup({ permission: 'default' })
    expect((await loadPush()).state).toBe('off')
    expect(notification.requestPermission).not.toHaveBeenCalled()
    expect(pushManager.subscribe).not.toHaveBeenCalled()
  })

  it('reports a server without keys', async () => {
    setup({})
    api.getPushKey.mockRejectedValue(new ApiError(503, 'push_disabled', 'off'))
    expect((await loadPush()).state).toBe('server-disabled')
  })

  it('waits for the worker when the registration has none active', async () => {
    vi.useFakeTimers()
    const { pushManager, serviceWorker } = setup({ active: false })
    const status = await loadPush()
    // `ready` resolved with a worker that has the push manager; the stub has no subscription.
    expect(status.state).toBe('off')
    expect(serviceWorker.getRegistration).toHaveBeenCalled()
    expect(pushManager.getSubscription).toHaveBeenCalled()
  })

  it('gives up on a worker that never comes', async () => {
    vi.useFakeTimers()
    const { serviceWorker } = setup({ active: false })
    serviceWorker.ready = new Promise(() => {}) as never
    const promise = loadPush()
    await vi.advanceTimersByTimeAsync(3100)
    expect((await promise).state).toBe('unsupported')
  })
})

describe('enablePush', () => {
  it('subscribes and sends the subscription', async () => {
    const { pushManager } = setup({ permission: 'default' })
    ;(Notification.requestPermission as ReturnType<typeof vi.fn>).mockResolvedValue('granted')
    expect(await enablePush(KEY as Uint8Array<ArrayBuffer>)).toBe('on')
    expect(pushManager.subscribe).toHaveBeenCalled()
    expect(api.savePushSubscription).toHaveBeenCalledWith(expect.objectContaining({ endpoint: 'https://new' }))
  })

  it('replaces a subscription made with a stale key before sending', async () => {
    const sub = fakeSubscription('https://old', buffer(9, 9, 9))
    const { pushManager } = setup({ current: sub })
    expect(await enablePush(KEY as Uint8Array<ArrayBuffer>)).toBe('on')
    expect(sub.unsubscribe).toHaveBeenCalled()
    expect(api.deletePushSubscription).toHaveBeenCalledWith('https://old')
    expect(pushManager.subscribe).toHaveBeenCalledTimes(1)
    expect(api.savePushSubscription).toHaveBeenCalledWith(expect.objectContaining({ endpoint: 'https://new' }))
  })

  it('reuses a subscription made with the current key', async () => {
    const sub = fakeSubscription('https://e', buffer(1, 2, 3))
    const { pushManager } = setup({ current: sub })
    expect(await enablePush(KEY as Uint8Array<ArrayBuffer>)).toBe('on')
    expect(pushManager.subscribe).not.toHaveBeenCalled()
    expect(api.savePushSubscription).toHaveBeenCalledWith(expect.objectContaining({ endpoint: 'https://e' }))
  })

  it('is denied when the user blocks the prompt', async () => {
    const { pushManager } = setup({ permission: 'denied' })
    expect(await enablePush(KEY as Uint8Array<ArrayBuffer>)).toBe('denied')
    expect(pushManager.subscribe).not.toHaveBeenCalled()
    expect(api.savePushSubscription).not.toHaveBeenCalled()
  })

  it('stays off when the prompt is dismissed', async () => {
    const { pushManager } = setup({ permission: 'default' })
    expect(await enablePush(KEY as Uint8Array<ArrayBuffer>)).toBe('off')
    expect(pushManager.subscribe).not.toHaveBeenCalled()
  })
})
