export const tip = $state({ text: '', x: 0, y: 0, visible: false, placement: 'top' as 'top' | 'right' | 'bottom' })

export function tipShow(e: MouseEvent, text: string, placement: 'top' | 'right' | 'bottom' = 'top') {
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
  tip.text = text
  tip.placement = placement
  if (placement === 'right') {
    tip.x = r.right
    tip.y = r.top + r.height / 2
  } else if (placement === 'bottom') {
    tip.x = r.left + r.width / 2
    tip.y = r.bottom
  } else {
    tip.x = r.left + r.width / 2
    tip.y = r.top
  }
  tip.visible = true
}

export function tipHide() {
  tip.visible = false
}
