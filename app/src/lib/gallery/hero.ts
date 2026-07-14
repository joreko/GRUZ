// FLIP-анимация карточка → лайтбокс и обратно.
// Чистые функции на DOMRect + transform. Уважает prefers-reduced-motion.

export function prefersReducedMotion(): boolean {
  return typeof matchMedia !== 'undefined' &&
    matchMedia('(prefers-reduced-motion: reduce)').matches
}

export function captureRect(el: HTMLElement | null): DOMRect | null {
  return el ? el.getBoundingClientRect() : null
}

/**
 * Открытие: морфим target (обёртку медиа лайтбокса) из originRect
 * в её текущее положение (hero-transition). Реальное изображение/постер
 * показывается сразу (opacity:1), никакого пустого «белого/чёрного ящика».
 * Назначение: target уже должен стоять в финальной раскладке — мы лишь
 * накладываем transform поверх.
 */
export function flipOpen(
  originRect: DOMRect | null,
  target: HTMLElement | null,
  done?: () => void,
) {
  if (!target || !originRect || prefersReducedMotion()) {
    done?.()
    return
  }
  // Ждём следующий кадр, чтобы панель осела и dest был финальным.
  requestAnimationFrame(() => {
    const dest = target!.getBoundingClientRect()
    if (dest.width === 0 || dest.height === 0) {
      done?.()
      return
    }
    const dx = originRect.left - dest.left
    const dy = originRect.top - dest.top
    const sx = originRect.width / dest.width
    const sy = originRect.height / dest.height

    target!.style.transformOrigin = 'top left'
    target!.style.transition = 'none'
    target!.style.transform = `translate(${dx}px, ${dy}px) scale(${sx}, ${sy})`
    target!.style.opacity = '1'

    // Форсируем reflow, чтобы transition сработал от начального состояния
    void target!.getBoundingClientRect()

    requestAnimationFrame(() => {
      target!.style.transition =
        'transform 0.28s cubic-bezier(0.22, 1, 0.36, 1)'
      target!.style.transform = 'none'
      const onEnd = () => {
        target!.removeEventListener('transitionend', onEnd)
        reset(target!)
        done?.()
      }
      target!.addEventListener('transitionend', onEnd)
      setTimeout(onEnd, 340)
    })
  })
}

/** Закрытие: анимируем target обратно в originRect. */
export function flipClose(
  target: HTMLElement | null,
  originRect: DOMRect | null,
  done?: () => void,
) {
  if (!target || !originRect || prefersReducedMotion()) {
    done?.()
    return
  }
  const dest = target.getBoundingClientRect()
  if (dest.width === 0 || dest.height === 0) {
    done?.()
    return
  }
  const dx = originRect.left - dest.left
  const dy = originRect.top - dest.top
  const sx = originRect.width / dest.width
  const sy = originRect.height / dest.height

  target.style.transformOrigin = 'top left'
  target.style.transition =
    'transform 0.24s cubic-bezier(0.4, 0, 0.2, 1)'
  target.style.transform = `translate(${dx}px, ${dy}px) scale(${sx}, ${sy})`
  target.style.opacity = '1'

  const onEnd = () => {
    target.removeEventListener('transitionend', onEnd)
    reset(target)
    done?.()
  }
  target.addEventListener('transitionend', onEnd)
  setTimeout(onEnd, 300)
}

function reset(el: HTMLElement) {
  el.style.transform = ''
  el.style.transition = ''
  el.style.transformOrigin = ''
  el.style.opacity = ''
}
