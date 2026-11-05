/**
 * Web Push on this device: what the Notifiche screen shows and the steps that turn
 * notifications on. The decisions are pure functions (tested without a browser); the
 * functions that talk to the browser are thin and follow the plan's decision AP: on open,
 * an existing subscription is sent to the server again, so the server never keeps a stale one.
 */
import {
  ApiError,
  deletePushSubscription,
  getPushKey,
  savePushSubscription,
  type PushSubscriptionBody,
} from './api'

/**
 * What the screen shows:
 * - `unsupported`: the browser cannot do Web Push;
 * - `ios-home-screen`: an iPhone or iPad where InTime is opened in Safari, not from the Home Screen;
 * - `server-disabled`: the server has no VAPID key;
 * - `denied`: the user (or the browser) blocked notifications;
 * - `off`: nothing subscribed yet; the tap on "Attiva notifiche" starts it;
 * - `on`: permission granted and this device is subscribed.
 */
export type PushState = 'unsupported' | 'ios-home-screen' | 'server-disabled' | 'denied' | 'off' | 'on'

export interface PushInputs {
  ios: boolean
  /** Opened from the Home Screen icon (or any installed window). */
  standalone: boolean
  /** Service worker, Push API and Notification API all exist. */
  supported: boolean
  /** The server has a VAPID key. */
  serverEnabled: boolean
  permission: NotificationPermission
  /** The browser holds a subscription made with the server's key. */
  subscribed: boolean
}

/** The state from its inputs. Order matters: each state hides the ones after it. */
export function pushState(input: PushInputs): PushState {
  // Safari on iOS has no Push API at all outside an installed app, so this comes first:
  // "unsupported" would be the wrong thing to tell someone who only has to install.
  if (input.ios && !input.standalone) return 'ios-home-screen'
  if (!input.supported) return 'unsupported'
  if (!input.serverEnabled) return 'server-disabled'
  if (input.permission === 'denied') return 'denied'
  if (input.permission === 'granted' && input.subscribed) return 'on'
  return 'off'
}

/** iPhone, iPad and iPod; an iPad in desktop mode says `Macintosh` but has a touch screen. */
export function isIos(userAgent: string, maxTouchPoints: number): boolean {
  return /iPhone|iPad|iPod/.test(userAgent) || (/Macintosh/.test(userAgent) && maxTouchPoints > 1)
}

/** The server's key is base64url without padding; `subscribe` wants the raw bytes. */
export function urlBase64ToUint8Array(value: string): Uint8Array<ArrayBuffer> {
  const base64 = value.replace(/-/g, '+').replace(/_/g, '/')
  const binary = atob(base64.padEnd(Math.ceil(base64.length / 4) * 4, '='))
  const bytes = new Uint8Array(new ArrayBuffer(binary.length))
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
  return bytes
}

/**
 * A subscription made with another key is useless to the server. A browser that does not
 * report the key (`null`/`undefined`) is given the benefit of the doubt.
 */
function keyChanged(subscription: PushSubscription, key: Uint8Array): boolean {
  const current = subscription.options?.applicationServerKey
  return current != null && !sameKey(current, key)
}

/** Whether a subscription was made with this key (`PushSubscription.options.applicationServerKey`). */
export function sameKey(current: ArrayBuffer | null | undefined, expected: Uint8Array): boolean {
  if (!current || current.byteLength !== expected.length) return false
  const bytes = new Uint8Array(current)
  return bytes.every((byte, i) => byte === expected[i])
}

/**
 * The body for the server, from `PushSubscription.toJSON()`. Fails when the browser left out
 * the endpoint or a key, rather than sending something the server will refuse.
 */
export function subscriptionBody(json: PushSubscriptionJSON): PushSubscriptionBody {
  const { endpoint, keys } = json
  if (!endpoint || !keys?.p256dh || !keys.auth) throw new Error('Incomplete push subscription')
  return {
    endpoint,
    expirationTime: json.expirationTime ?? null,
    keys: { p256dh: keys.p256dh, auth: keys.auth },
  }
}

// ---- The browser side: thin, not unit tested. ----

function standalone(): boolean {
  return (
    matchMedia('(display-mode: standalone)').matches ||
    (navigator as Navigator & { standalone?: boolean }).standalone === true
  )
}

function supported(): boolean {
  return 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window
}

/**
 * The service worker's registration. It exists only in builds, and a few moments after the
 * first visit, so waiting is bounded: `ready` never settles when there is no worker at all.
 */
async function registration(): Promise<ServiceWorkerRegistration | null> {
  const existing = await navigator.serviceWorker.getRegistration()
  // A registration whose worker is still installing cannot subscribe yet: wait for `ready`.
  if (existing?.active) return existing
  return Promise.race([
    navigator.serviceWorker.ready,
    new Promise<null>((resolve) => setTimeout(() => resolve(null), 3000)),
  ])
}

export interface PushStatus {
  state: PushState
  /** The server's public key; only when the server has one. */
  key: Uint8Array<ArrayBuffer> | null
}

/** Forgets a subscription made with an old key, then subscribes again with the current one. */
async function replaceSubscription(
  worker: ServiceWorkerRegistration,
  old: PushSubscription,
  key: Uint8Array<ArrayBuffer>,
): Promise<PushSubscription> {
  await old.unsubscribe()
  await deletePushSubscription(old.endpoint).catch(() => {})
  return worker.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: key })
}

/**
 * Reads the state when the screen opens. With permission already granted and a subscription
 * in the browser, it is sent to the server again (upsert by endpoint); if it was made with
 * another key (the server's key changed) it is replaced. Never asks for permission.
 * Throws an `ApiError` when the server cannot be reached.
 */
export async function loadPush(): Promise<PushStatus> {
  const ios = isIos(navigator.userAgent, navigator.maxTouchPoints)
  const base = { ios, standalone: standalone(), supported: supported() }
  const none = { serverEnabled: false, permission: 'default' as const, subscribed: false }
  // These two need neither the server nor the browser's push machinery.
  if (pushState({ ...base, ...none }) === 'ios-home-screen') return { state: 'ios-home-screen', key: null }
  if (!base.supported) return { state: 'unsupported', key: null }

  let key: Uint8Array<ArrayBuffer>
  try {
    key = urlBase64ToUint8Array(await getPushKey())
  } catch (error) {
    if (error instanceof ApiError && error.code === 'push_disabled') {
      return { state: 'server-disabled', key: null }
    }
    throw error
  }

  const permission = Notification.permission
  let subscribed = false
  if (permission === 'granted') {
    const worker = await registration()
    if (!worker) return { state: 'unsupported', key }
    let subscription = await worker.pushManager.getSubscription()
    if (subscription && keyChanged(subscription, key)) {
      try {
        subscription = await replaceSubscription(worker, subscription, key)
      } catch {
        return { state: 'off', key }
      }
    }
    if (subscription) {
      await savePushSubscription(subscriptionBody(subscription.toJSON()))
      subscribed = true
    }
  }
  return { state: pushState({ ...base, serverEnabled: true, permission, subscribed }), key }
}

/**
 * Turns notifications on. Call it from a tap and nothing else: the permission prompt must
 * follow a user gesture (iOS refuses it otherwise), so `requestPermission` is the first
 * thing that runs, before any `await`.
 */
export async function enablePush(key: Uint8Array<ArrayBuffer>): Promise<PushState> {
  const permission = await Notification.requestPermission()
  if (permission === 'denied') return 'denied'
  if (permission !== 'granted') return 'off' // The prompt was dismissed: ask again next time.
  const worker = await registration()
  if (!worker) return 'unsupported'
  let subscription = await worker.pushManager.getSubscription()
  if (subscription && keyChanged(subscription, key)) subscription = await replaceSubscription(worker, subscription, key)
  subscription ??= await worker.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: key })
  await savePushSubscription(subscriptionBody(subscription.toJSON()))
  return 'on'
}
