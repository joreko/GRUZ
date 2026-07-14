// Ленивая загрузка изображений через IntersectionObserver.
// URL хранится в data-src, реально ставится в src только когда картинка
// попадает в зону видимости (+ rootMargin буфера). Для Tauri WebView это
// экономит сетевые запросы к i.ytimg при больших библиотеках.

export interface LazyImageOptions {
  rootMargin?: string
}

export function lazyImage(node: HTMLImageElement, options: LazyImageOptions = {}) {
  const margin = options.rootMargin ?? '400px'
  let observer: IntersectionObserver | null = null

  const reveal = () => {
    const src = node.dataset.src
    if (src && node.src !== src) {
      node.src = src
      // decode() не блокирует поток, но даёт плавный fade-in по onload
      node.decode?.().catch(() => {})
    }
  }

  if (typeof IntersectionObserver !== 'undefined') {
    observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            reveal()
            observer?.disconnect()
            observer = null
            break
          }
        }
      },
      { rootMargin: margin }
    )
    observer.observe(node)
  } else {
    reveal()
  }

  return {
    destroy() {
      observer?.disconnect()
    }
  }
}
