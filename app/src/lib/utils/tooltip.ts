import { tip, tipShow, tipHide } from '$lib/stores/tooltip.svelte'

export { tip, tipShow, tipHide }

/** Svelte action: use:tooltip={'текст'} или use:tooltip={{ text: 'текст', placement: 'right' }} */
export function tooltip(node: HTMLElement, options: string | { text: string; placement?: 'top' | 'right' | 'bottom' }) {
  function getOpts() {
    return typeof options === 'string'
      ? { text: options, placement: 'top' as const }
      : { text: options.text, placement: options.placement ?? 'top' as const }
  }
  const enter = (e: MouseEvent) => { const o = getOpts(); tipShow(e, o.text, o.placement) }
  node.addEventListener('mouseenter', enter)
  node.addEventListener('mouseleave', tipHide)
  return {
    update(newOpts: string | { text: string; placement?: 'top' | 'right' | 'bottom' }) { options = newOpts },
    destroy() {
      node.removeEventListener('mouseenter', enter)
      node.removeEventListener('mouseleave', tipHide)
      tipHide()
    }
  }
}
