import type { IconName } from './components/Icon.svelte'
import type { Tab } from './router'

/** The bottom bar, in order. `path` is where each entry leads. */
export const tabs: { tab: Tab; label: string; icon: IconName; path: string }[] = [
  { tab: 'home', label: 'Home', icon: 'home', path: '/' },
  { tab: 'search', label: 'Cerca', icon: 'search', path: '/cerca' },
  { tab: 'near', label: 'Vicino', icon: 'pin', path: '/vicino' },
  { tab: 'favorites', label: 'Preferiti', icon: 'star', path: '/preferiti' },
  { tab: 'notifications', label: 'Notifiche', icon: 'bell', path: '/notifiche' },
]

export function tabInfo(tab: Tab) {
  return tabs.find((entry) => entry.tab === tab) ?? tabs[0]
}
