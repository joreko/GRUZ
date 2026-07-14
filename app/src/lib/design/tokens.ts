// Дизайн-токены — единственный источник правды для всего UI
// Нигде в коде не должно быть хардкодных цветов, отступов, радиусов

export const color = {
  // Фон
  bg: {
    content: '#111111',
    base: '#0f0f0f',
    surface: '#161616',
    elevated: '#1e1e1e',
    overlay: '#252525',
    selected: '#111111',
  },
  // Граница
  border: {
    subtle: '#2a2a2a',
    default: '#3a3a3a',
    strong: '#4a4a4a',
  },
  // Текст
  text: {
    primary: '#f0f0f0',
    secondary: '#a0a0a0',
    muted: '#606060',
    tertiary: '#8a8a8a',
    inverse: '#0f0f0f',
  },
  // Акцент — фирменный красный (CTA, выделение, активные элементы)
  accent: {
    default: '#e63946',
    hover: '#ff4d5a',
    active: '#cc2f3b',
    subtle: '#e6394620',
    warm: '#ff6b3d',
  },
  // Статусы
  status: {
    success: '#22c55e',
    warning: '#f59e0b',
    error: '#ef4444',
    info: '#3b82f6',
    downloading: '#6366f1',
  },
  // Мысли оркестратора
  thought: {
    muted:   '#a1a1aa',
    success: '#39ff6e',
    error:   '#ff3333',
    warning: '#ffdd00',
    info:    '#00e5ff',
    pink:    '#ff80cc',
    dash:    '#3f3f46',
  },
  // Полноэкранные сцены (лайтбокс) — сплошной чёрный холст
  scrim: {
    base:   '#000000',
    soft:   '#0c0c0c',
    overlay: 'rgba(0, 0, 0, 0.72)',
  },
  // Контент поверх scrim (кнопки навигации, крестик)
  onScrim: '#ffffff',
  // Тени (токенизированы, чтобы не хардкодить rgba в компонентах)
  shadow: {
    card:      '0 4px 20px rgba(0, 0, 0, 0.2)',
    cardHover: '0 12px 32px rgba(0, 0, 0, 0.5)',
    panel:     '0 24px 64px rgba(0, 0, 0, 0.6)',
    menu:      '0 8px 32px rgba(0, 0, 0, 0.5)',
  },
} as const

export const space = {
  1: '4px',
  2: '8px',
  3: '12px',
  4: '16px',
  5: '20px',
  6: '24px',
  8: '32px',
  9: '36px',
  10: '40px',
  12: '48px',
} as const

export const radius = {
  sm: '4px',
  md: '8px',
  card: '10px',
  lg: '12px',
  panel: '14px',
  xl: '16px',
  '2xl': '48px',
  full: '9999px',
} as const

export const font = {
  size: {
    xs: '11px',
    sm: '13px',
    md: '15px',
    lg: '17px',
    xl: '20px',
    '2xl': '24px',
  },
  weight: {
    normal: '400',
    medium: '500',
    semibold: '600',
    bold: '700',
  },
  family: {
    sans: "'Inter', 'Segoe UI', system-ui, sans-serif",
    mono: "'JetBrains Mono', 'Fira Code', monospace",
  },
} as const

export const transition = {
  fast: '120ms ease',
  default: '200ms ease',
  slow: '300ms ease',
} as const

export const sidebar = {
  width: '220px',
} as const
