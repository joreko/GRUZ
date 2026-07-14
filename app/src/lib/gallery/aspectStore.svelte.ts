// Реальные пропорции превью (id -> [width, height]), измеряются при загрузке
// картинки в мозаике и драйвят высоту сетки (Pinterest-раскладка).
// Module-level $state: читается напрямую в VirtualGrid.layout, без цепочки props/derived.
export const aspectStore = $state<Record<string, [number, number]>>({})
