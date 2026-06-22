// architecture-guard.ts
// Принуждает архитектурные правила ГРУЗ ДО записи в файл.
// Нарушение => запись отклоняется с понятным сообщением.
//
// Правила (из AGENTS.md):
//   1. invoke()  — только в app/src/lib/bridge/commands.ts
//   2. listen()  — только в app/src/lib/bridge/events.ts
//   3. Хардкод hex-цветов (#rgb / #rrggbb) запрещён вне app/src/lib/design/
//   4. Слой commands/ (Rust) не лезет напрямую в queue/downloader/db —
//      только через orchestrator (мягкое предупреждение, не блок).

/** Нормализуем путь к виду с прямыми слешами и без учёта регистра диска. */
function norm(p) {
  return String(p || "").replace(/\\./g, "/")
}

/** Достаём текст, который агент собирается записать, из аргументов разных тулзов. */
function extractWrittenText(tool, args) {
  if (!args) return ""
  if (tool === "write") return String(args.content ?? "")
  if (tool === "edit") return String(args.newString ?? "")
  if (tool === "apply_patch") {
    const patch = String(args.patch ?? args.input ?? "")
    return patch
      .split("\n")
      .filter((l) => l.startsWith("+") && !l.startsWith("+++"))
      .join("\n")
  }
  return ""
}

function getFilePath(args) {
  if (!args) return ""
  return norm(args.filePath ?? args.path ?? args.file ?? "")
}

const HEX_COLOR = /#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/

export const ArchitectureGuard = async ({ client }) => {
  function deny(rule, msg) {
    throw new Error(
      `[architecture-guard] Нарушение архитектуры ГРУЗ (${rule}):\n${msg}\n` +
        `Правила описаны в AGENTS.md. Исправь подход, а не правило.`,
    )
  }

  return {
    "tool.execute.before": async (input, output) => {
      const tool = input.tool
      if (tool !== "write" && tool !== "edit" && tool !== "apply_patch") return

      const file = getFilePath(output.args)
      const text = extractWrittenText(tool, output.args)
      if (!file || !text) return

      // installer/ — отдельное приложение, своя архитектура, не подчиняется bridge-правилам
      if (file.includes(String.fromCharCode(47) + String.fromCharCode(105) + String.fromCharCode(110) + String.fromCharCode(115) + String.fromCharCode(116) + String.fromCharCode(97) + String.fromCharCode(108) + String.fromCharCode(108) + String.fromCharCode(101) + String.fromCharCode(114) + String.fromCharCode(47))) return

      // Исключения: установщик — отдельное приложение, плагины — служебные
      if (file.includes("/installer/") || file.includes("/.opencode/")) return

      const isBridge = file.includes("/lib/bridge/")
      const isDesign = file.includes("/lib/design/")
      const isFrontendTs = /\.(ts|svelte)$/.test(file)
      const isRust = file.endsWith(".rs")

      // --- Правило 1: invoke() только в bridge/commands.ts ---
      if (isFrontendTs && /\binvoke\s*[<(]/.test(text)) {
        const allowed = file.endsWith("/lib/bridge/commands.ts")
        if (!allowed) {
          deny(
            "invoke-outside-bridge",
            `Вызов invoke() обнаружен в ${file}.\n` +
              `invoke() разрешён ТОЛЬКО в app/src/lib/bridge/commands.ts.\n` +
              `Добавь метод в объект commands и импортируй его отсюда.`,
          )
        }
      }

      // --- Правило 2: listen() только в bridge/events.ts ---
      if (isFrontendTs && /\blisten\s*[<(]/.test(text)) {
        const allowed = file.endsWith("/lib/bridge/events.ts")
        if (!allowed) {
          deny(
            "listen-outside-bridge",
            `Вызов listen() обнаружен в ${file}.\n` +
              `listen() разрешён ТОЛЬКО в app/src/lib/bridge/events.ts.\n` +
              `Подпишись на событие через events.ts и пробрось данные в store.`,
          )
        }
      }

      // --- Правило 3: хардкод hex-цветов вне design/ ---
      if (isFrontendTs && !isDesign && !isBridge) {
        const match = text.match(HEX_COLOR)
        if (match) {
          deny(
            "hardcoded-color",
            `Хардкод цвета "${match[0]}" в ${file}.\n` +
              `Цвета живут только в app/src/lib/design/tokens.ts.\n` +
              `Используй токен дизайн-системы вместо литерала.`,
          )
        }
      }

      // --- Правило 4 (мягкое): commands/ Rust не зовёт queue/downloader/db напрямую ---
      if (isRust && file.includes("/commands/")) {
        const badImport = /use\s+crate::(queue|downloader|db)\b/.test(text)
        if (badImport) {
          await client.app.log({
            body: {
              service: "architecture-guard",
              level: "warn",
              message: `commands/ импортирует напрямую queue/downloader/db в ${file}. Делегируй через orchestrator.`,
            },
          })
        }
      }
    },
  }
}
