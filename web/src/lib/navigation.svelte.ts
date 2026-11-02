/**
 * The current screen, kept in step with the address bar. The `.svelte.ts` extension lets
 * this module use runes: `$state` makes `navigation` reactive, so components reading it
 * update when it changes.
 */
import { isInternalClick, matchRoute, tabOf, type Route, type Tab } from './router'

const options = { dev: import.meta.env.DEV }

/**
 * Each history entry made by the app records its position, so a back link knows whether
 * there is a screen of the app to go back to (the first screen is position 0).
 */
interface EntryState {
  position: number
}

function position(): number {
  return (history.state as EntryState | null)?.position ?? 0
}

export const navigation = $state<{ route: Route; tab: Tab; canGoBack: boolean }>({
  route: matchRoute(location.pathname, location.search, options),
  tab: 'home',
  canGoBack: position() > 0,
})
navigation.tab = tabOf(navigation.route, 'home')

function update() {
  navigation.route = matchRoute(location.pathname, location.search, options)
  navigation.tab = tabOf(navigation.route, navigation.tab)
  navigation.canGoBack = position() > 0
}

/** Shows another screen; `replace` changes the address without a new history entry. */
export function navigate(path: string, { replace = false } = {}) {
  if (replace) {
    history.replaceState({ position: position() } satisfies EntryState, '', path)
  } else {
    history.pushState({ position: position() + 1 } satisfies EntryState, '', path)
    window.scrollTo(0, 0)
  }
  update()
}

/** The back links: the previous screen of the app, or `fallback` when the app was opened here. */
export function goBack(fallback: string) {
  if (navigation.canGoBack) {
    history.back()
  } else {
    navigate(fallback, { replace: true })
  }
}

/**
 * Starts following the address bar: the browser's back and forward buttons, and clicks on
 * any `<a href>` of the app, which change screen without reloading the page. Returns the
 * function that stops it.
 */
export function startNavigation(): () => void {
  const onPopState = () => update()

  const onClick = (event: MouseEvent) => {
    const link = (event.target as Element | null)?.closest('a')
    if (!link || !link.hasAttribute('href')) return
    const click = {
      button: event.button,
      modifier: event.metaKey || event.ctrlKey || event.shiftKey || event.altKey,
      defaultPrevented: event.defaultPrevented,
      target: link.target,
      download: link.hasAttribute('download'),
      url: new URL(link.href),
      origin: location.origin,
    }
    if (isInternalClick(click)) {
      event.preventDefault()
      navigate(click.url.pathname + click.url.search)
    }
  }

  window.addEventListener('popstate', onPopState)
  document.addEventListener('click', onClick)
  return () => {
    window.removeEventListener('popstate', onPopState)
    document.removeEventListener('click', onClick)
  }
}
