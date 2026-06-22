<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  const appWindow = getCurrentWindow()

  // --- Типы ---
  type Screen = 'loading' | 'license' | 'install' | 'installed' | 'progress' | 'done' | 'uninstalling' | 'uninstalled' | 'error' | 'running' | 'changelog'
  type ComponentState = 'pending' | 'active' | 'done' | 'error'

  interface ProgressEvent { step: string; pct: number; done: boolean; error: string | null }
  interface InstallState { installed: boolean; version: string | null; install_dir: string; same_version: boolean }
  interface Component { id: string; label: string; state: ComponentState }

  // --- Состояние ---
  let screen = $state<Screen>('loading')
  let installState = $state<InstallState | null>(null)
  let licenseAccepted = $state(false)
  let pct = $state(0)
  let step = $state('')
  let errorMsg = $state('')
  let mode = $state<'install' | 'upgrade' | 'uninstall'>('install')

  let customInstallDir = $state('')
  let desktopShortcut = $state(true)
  let addToPath = $state(false)
  let autostart = $state(false)
  let launchAfter = $state(true)

  let payloadSizeMb = $state<number | null>(null)
  let changelogText = $state('')

  let components = $state<Component[]>([
    { id: 'preflight', label: 'Preflight',    state: 'pending' },
    { id: 'gruz',      label: 'Груз',          state: 'pending' },
    { id: 'ytdlp',     label: 'yt-dlp',        state: 'pending' },
    { id: 'ffmpeg',    label: 'ffmpeg',         state: 'pending' },
    { id: 'link',      label: 'Ярлыки',        state: 'pending' },
    { id: 'reg',       label: 'Реестр',        state: 'pending' },
  ])

  // B1: реактивный $state для uninstall компонентов
  let uninstallComps = $state<Component[]>([
    { id: 'link',  label: 'Ярлыки',  state: 'pending' },
    { id: 'reg',   label: 'Реестр',  state: 'pending' },
    { id: 'files', label: 'Файлы',   state: 'pending' },
  ])

  const stepMap: Record<string, string> = {
    'Проверяю систему':  'preflight',
    'Копирую Груз':      'gruz',
    'Копирую yt-dlp':    'ytdlp',
    'Копирую ffmpeg':    'ffmpeg',
    'Создаю ярлыки':     'link',
    'Регистрирую':       'reg',
    'Удаляю ярлыки':     'link',
    'Удаляю из реестра': 'reg',
    'Удаляю файлы':      'files',
  }

  function updateComponents(stepText: string, allDone: boolean, comps: Component[]): Component[] {
    const activeId = Object.entries(stepMap).find(([k]) => stepText.startsWith(k))?.[1] ?? null
    const activeIdx = comps.findIndex(c => c.id === activeId)
    return comps.map((c, i) => {
      if (allDone) return { ...c, state: 'done' }
      if (c.id === activeId) return { ...c, state: 'active' }
      if (activeIdx >= 0 && i < activeIdx) return { ...c, state: 'done' }
      return c
    })
  }

  // --- Инициализация ---
  onMount(async () => {
    const state = await invoke<InstallState>('check_install')
    installState = state
    customInstallDir = state.install_dir
    screen = state.installed ? 'installed' : 'license'

    // B6: размер payload
    invoke<number>('get_payload_size_mb').then(mb => { payloadSizeMb = mb }).catch(() => {})

    await listen<ProgressEvent>('install_progress', ({ payload }) => {
      pct = payload.pct
      step = payload.step
      const isUninstall = mode === 'uninstall'

      if (isUninstall) {
        // B1: обновляем реактивный uninstallComps
        uninstallComps = updateComponents(payload.step, payload.done, uninstallComps)
      } else {
        components = updateComponents(payload.step, payload.done, components)
      }

      if (payload.error) {
        // B5: детектируем запущенный груз
        if (payload.error.includes('GRUZ_RUNNING')) {
          screen = 'running'
        } else {
          screen = 'error'
          errorMsg = payload.error
          if (isUninstall) {
            uninstallComps = uninstallComps.map(c => c.state === 'active' ? { ...c, state: 'error' } : c)
          } else {
            components = components.map(c => c.state === 'active' ? { ...c, state: 'error' } : c)
          }
        }
      } else if (payload.done) {
        if (mode === 'uninstall') {
          screen = 'uninstalled'
        } else if (mode === 'upgrade') {
          // B7: после upgrade показываем changelog если есть
          invoke<string>('get_changelog').then(log => {
            changelogText = log
            screen = log.trim() ? 'changelog' : 'done'
          }).catch(() => { screen = 'done' })
        } else {
          screen = 'done'
        }
      }
    })
  })

  // B8: автозапуск после install если launchAfter включён
  $effect(() => {
    if (screen === 'done' && mode === 'install' && launchAfter) {
      invoke('launch_gruz').finally(() => getCurrentWindow().close())
    }
  })

  // --- Действия ---
  function goInstall() {
    mode = 'install'; screen = 'license'
  }
  function goUpgrade() {
    mode = 'upgrade'; screen = 'license'
  }
  async function goUninstall() {
    mode = 'uninstall'; pct = 0; step = 'Начинаю удаление…'
    uninstallComps = uninstallComps.map(c => ({ ...c, state: 'pending' }))
    screen = 'uninstalling'
    await invoke('uninstall')
  }

  // B4: передаём все опции в install
  async function startInstall() {
    pct = 0; step = 'Начинаю…'
    components = components.map(c => ({ ...c, state: 'pending' }))
    screen = 'progress'
    await invoke('install', {
      opts: {
        mode,
        install_dir: customInstallDir || null,
        desktop_shortcut: desktopShortcut,
        add_to_path: addToPath,
        autostart,
        launch_after: launchAfter,
      }
    })
  }

  // B5: закрыть груз и продолжить
  async function killAndInstall() {
    screen = 'progress'
    await invoke('kill_gruz')
    await startInstall()
  }

  // B2: выбор папки через диалог
  async function pickDir() {
    try {
      const dir = await invoke<string>('pick_install_dir')
      if (dir) customInstallDir = dir
    } catch(e) {}
  }

  async function launchGruz() {
    try { await invoke('launch_gruz') } catch(e) { screen = 'error'; errorMsg = String(e) }
  }
  function close() { getCurrentWindow().close() }

  // Частицы при успехе
  let particles = $state<{x:number;y:number;size:number;delay:number}[]>([])
  $effect(() => {
    if (screen === 'done' || screen === 'uninstalled' || screen === 'changelog') {
      particles = Array.from({length: 20}, () => ({
        x: Math.random() * 100,
        y: 40 + Math.random() * 60,
        size: 2 + Math.random() * 5,
        delay: Math.random() * 1.5,
      }))
    }
  })

  // Компоненты для правой колонки
  let activeComps = $derived(screen === 'uninstalling' ? uninstallComps : components)
</script>

<div class="shell" onmousedown={(e) => { if ((e.target as HTMLElement).closest('button, input, a, [role=button]') === null) appWindow.startDragging() }}>

  {#if screen === 'done' || screen === 'uninstalled' || screen === 'changelog'}
    {#each particles as p}
      <div class="particle" style="left:{p.x}%;top:{p.y}%;width:{p.size}px;height:{p.size}px;animation-delay:{p.delay}s"></div>
    {/each}
  {/if}

  <button class="close-btn" onclick={close} aria-label="Закрыть">
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M1 1l10 10M11 1L1 11"/></svg>
  </button>

  <div class="left">
    <div class="brand">
      <img src="/logo.svg" alt="Груз" class="logo" draggable="false"/>
      <div class="brand-text">
        <span class="brand-name">ГРУЗ</span>
        <span class="brand-sub">загрузчик видео</span>
      </div>
    </div>

    <div class="spacer"></div>

    <!-- Loading -->
    {#if screen === 'loading'}
      <div class="status-text">Проверяю систему…</div>

    <!-- Лицензия + опции -->
    {:else if screen === 'license'}
      <div class="license-block">
        <div class="license-title">Лицензионное соглашение</div>
        <div class="license-scroll">
          <p>Груз — бесплатная программа с открытым исходным кодом (Commons Clause + MIT).</p>
          <p>Используя Груз, вы соглашаетесь:</p>
          <ul>
            <li>Не использовать приложение для нарушения авторских прав.</li>
            <li>Скачивать только контент, на который имеете право.</li>
            <li>Разработчик не несёт ответственности за использование приложения.</li>
          </ul>
          <p>Исходный код доступен на <span class="link" role="button" tabindex="0" onclick={() => invoke('open_url', {url:'https://github.com/joreko/GRUZ'})} onkeydown={() => {}}>github.com/joreko/GRUZ</span>.</p>
        </div>
        <label class="license-check">
          <input type="checkbox" bind:checked={licenseAccepted}/>
          <span>Я принимаю условия соглашения</span>
        </label>

        <!-- B2: Папка установки -->
        <div class="field-label">Папка установки</div>
        <div class="dir-row">
          <input class="dir-input" type="text" bind:value={customInstallDir} spellcheck="false" aria-label="Папка установки"/>
          <button class="btn-pick" onclick={pickDir} title="Выбрать папку">…</button>
        </div>

        <!-- B3: Опции -->
        <div class="opts">
          <label class="opt-check">
            <input type="checkbox" bind:checked={desktopShortcut}/>
            <span>Ярлык на рабочем столе</span>
          </label>
          <label class="opt-check">
            <input type="checkbox" bind:checked={addToPath}/>
            <span>Добавить в PATH</span>
          </label>
          <label class="opt-check">
            <input type="checkbox" bind:checked={autostart}/>
            <span>Запускать при старте Windows</span>
          </label>
          <label class="opt-check">
            <input type="checkbox" bind:checked={launchAfter}/>
            <span>Запустить после установки</span>
          </label>
        </div>
      </div>
      <div class="actions">
        <button class="btn-primary" disabled={!licenseAccepted} onclick={startInstall}>
          {mode === 'upgrade' ? 'Обновить' : 'Установить'}
        </button>
        <button class="btn-ghost" onclick={close}>Отмена</button>
      </div>

    <!-- Уже установлено -->
    {:else if screen === 'installed'}
      <div class="installed-block">
        <div class="installed-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M9 12l2 2 4-4"/><circle cx="12" cy="12" r="9"/></svg>
        </div>
        <div class="installed-text">
          <span class="installed-title">Груз установлен</span>
          <span class="installed-ver">Версия {installState?.version ?? '—'}{installState?.same_version ? ' · актуальная' : ' · доступно обновление'}</span>
        </div>
      </div>
      <div class="installed-dir">{installState?.install_dir}</div>
      <div class="actions">
        {#if !installState?.same_version}
          <button class="btn-primary" onclick={goUpgrade}>Обновить</button>
        {:else}
          <button class="btn-primary" onclick={launchGruz}>Запустить</button>
        {/if}
        <button class="btn-ghost" onclick={goInstall}>Переустановить</button>
        <button class="btn-danger" onclick={goUninstall}>Удалить</button>
      </div>

    <!-- Прогресс установки -->
    {:else if screen === 'progress'}
      <div class="step-text">{step}</div>
      <div class="track"><div class="fill" style="width:{pct}%"></div><div class="shimmer"></div></div>
      <!-- B6: размер payload рядом с путём -->
      <div class="info-row">
        <span class="dir">{customInstallDir || installState?.install_dir}{payloadSizeMb != null ? `  ·  ${payloadSizeMb} МБ` : ''}</span>
        <span class="pct-label">{pct}%</span>
      </div>
      <div class="actions"><button class="btn-primary" disabled>Установка…</button></div>

    <!-- Прогресс удаления -->
    {:else if screen === 'uninstalling'}
      <div class="step-text">{step}</div>
      <div class="track"><div class="fill fill-warn" style="width:{pct}%"></div><div class="shimmer"></div></div>
      <div class="info-row"><span class="pct-label">{pct}%</span></div>
      <div class="actions"><button class="btn-primary" disabled>Удаление…</button></div>

    <!-- B5: Груз запущен -->
    {:else if screen === 'running'}
      <div class="done-block">
        <div class="done-icon done-warn">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M12 9v4M12 17h.01"/><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/></svg>
        </div>
        <div class="done-title">Груз сейчас запущен.</div>
        <div class="done-sub">Нужно закрыть его чтобы продолжить установку.</div>
      </div>
      <div class="actions">
        <button class="btn-primary" onclick={killAndInstall}>Закрыть Груз и продолжить</button>
        <button class="btn-ghost" onclick={close}>Отмена</button>
      </div>

    <!-- Готово -->
    {:else if screen === 'done'}
      <div class="done-block">
        <div class="done-icon done-success">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M5 13l4 4L19 7"/></svg>
        </div>
        <div class="done-title">Груз установлен.</div>
        <div class="done-sub">{launchAfter && mode === 'install' ? 'Запускаю…' : 'Всё готово к работе.'}</div>
      </div>
      {#if !(launchAfter && mode === 'install')}
        <div class="actions">
          <button class="btn-primary" onclick={launchGruz}>Запустить Груз</button>
          <button class="btn-ghost" onclick={close}>Закрыть</button>
        </div>
      {/if}

    <!-- B7: Changelog после upgrade -->
    {:else if screen === 'changelog'}
      <div class="changelog-header">
        <div class="done-icon done-success" style="margin-bottom:0">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M5 13l4 4L19 7"/></svg>
        </div>
        <div class="done-title">Что нового</div>
      </div>
      <div class="changelog-scroll">{changelogText}</div>
      <div class="actions">
        <button class="btn-primary" onclick={launchGruz}>Запустить Груз</button>
        <button class="btn-ghost" onclick={close}>Закрыть</button>
      </div>

    <!-- Удалено -->
    {:else if screen === 'uninstalled'}
      <div class="done-block">
        <div class="done-icon done-muted">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6"/></svg>
        </div>
        <div class="done-title">Груз удалён.</div>
        <div class="done-sub">Надеемся увидеть тебя снова.</div>
      </div>
      <div class="actions"><button class="btn-ghost" onclick={close}>Закрыть</button></div>

    <!-- Ошибка -->
    {:else if screen === 'error'}
      <div class="done-block">
        <div class="done-icon done-error">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M12 8v4M12 16h.01"/><circle cx="12" cy="12" r="9"/></svg>
        </div>
        <div class="done-title">Что-то пошло не так.</div>
        <div class="done-sub error-msg">{errorMsg}</div>
      </div>
      <div class="actions">
        <button class="btn-primary" onclick={startInstall}>Попробовать снова</button>
        <button class="btn-ghost" onclick={close}>Закрыть</button>
      </div>
    {/if}
  </div>

  <!-- Правая колонка -->
  <div class="right">
    <span class="comp-title">Компоненты</span>
    <div class="comps">
      {#each activeComps as c}
        <div class="badge badge-{c.state}">
          {#if c.state === 'done'}
            <svg class="bi" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M2 5l2 2 4-4"/></svg>
          {:else if c.state === 'active'}
            <div class="bdot"></div>
          {:else if c.state === 'error'}
            <svg class="bi" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M2 2l6 6M8 2l-6 6"/></svg>
          {/if}
          {c.label}
        </div>
      {/each}
    </div>

    <div class="spacer"></div>

    <div class="meta">
      <span>v{installState?.version ?? '0.0.1'}</span>
      <span>·</span>
      <span class="meta-link" role="button" tabindex="0"
        onclick={() => invoke('open_url', {url:'https://github.com/joreko/GRUZ'})}
        onkeydown={() => {}}>GitHub</span>
      <span>·</span>
      <span class="meta-link" role="button" tabindex="0"
        onclick={() => invoke('open_url', {url:'https://t.me/+rVTNJ_uXV0s4NTky'})}
        onkeydown={() => {}}>Telegram</span>
    </div>
  </div>

</div>

<style>
  :global(*,*::before,*::after){box-sizing:border-box;margin:0;padding:0}
  :global(html){background:transparent;border-radius:24px;overflow:hidden}
  :global(body){background:transparent;border-radius:24px;overflow:hidden;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;-webkit-font-smoothing:antialiased;user-select:none}

  .shell{position:relative;width:860px;height:480px;background:linear-gradient(135deg,#1a1a1a 0%,#0f0f0f 100%);border-radius:24px;border:1px solid rgba(0,0,0,1);box-shadow:inset 0 1px 0 rgba(120,120,120,.5),inset 0 -1px 0 rgba(80,80,80,.15),inset 1px 0 0 rgba(120,120,120,.2),inset -1px 0 0 rgba(80,80,80,.1),0 32px 80px rgba(0,0,0,.8);display:flex;overflow:hidden}

  .close-btn{position:absolute;top:16px;right:16px;width:28px;height:28px;background:rgba(255,255,255,.05);border:1px solid rgba(255,255,255,.08);border-radius:8px;cursor:pointer;display:flex;align-items:center;justify-content:center;color:rgba(255,255,255,.3);z-index:10;transition:background .15s,color .15s}
  .close-btn:hover{background:rgba(255,255,255,.1);color:rgba(255,255,255,.7)}
  .close-btn svg{width:12px;height:12px}

  .left{flex:1;padding:44px 44px 36px;display:flex;flex-direction:column;min-width:0}
  .brand{display:flex;align-items:center;gap:14px}
  .logo{width:52px;height:52px;flex-shrink:0}
  .brand-text{display:flex;flex-direction:column;gap:2px}
  .brand-name{font-size:30px;font-weight:800;letter-spacing:.08em;color:#fff;line-height:1}
  .brand-sub{font-size:11px;color:rgba(255,255,255,.35);letter-spacing:.04em}
  .spacer{flex:1}

  /* Лицензия */
  .license-block{display:flex;flex-direction:column;gap:10px}
  .license-title{font-size:13px;font-weight:600;color:rgba(255,255,255,.7)}
  .license-scroll{background:rgba(255,255,255,.04);border:1px solid rgba(255,255,255,.07);border-radius:10px;padding:14px 16px;max-height:110px;overflow-y:auto;display:flex;flex-direction:column;gap:8px}
  .license-scroll p,.license-scroll li{font-size:11px;color:rgba(255,255,255,.4);line-height:1.6}
  .license-scroll ul{padding-left:16px}
  .license-scroll .link{color:rgba(255,255,255,.6);cursor:pointer;text-decoration:underline}
  .license-check{display:flex;align-items:center;gap:8px;cursor:pointer}
  .license-check input{accent-color:#ff3d3d;width:14px;height:14px;cursor:pointer}
  .license-check span{font-size:12px;color:rgba(255,255,255,.6)}

  /* B2: Папка установки */
  .field-label{font-size:10px;font-weight:600;text-transform:uppercase;letter-spacing:.08em;color:rgba(255,255,255,.25);margin-bottom:-4px}
  .dir-row{display:flex;gap:6px;align-items:center}
  .dir-input{flex:1;height:30px;background:rgba(255,255,255,.04);border:1px solid rgba(255,255,255,.1);border-radius:8px;color:rgba(255,255,255,.6);font-size:11px;font-family:monospace;padding:0 10px;outline:none;transition:border-color .15s}
  .dir-input:focus{border-color:rgba(255,61,61,.4)}
  .btn-pick{width:30px;height:30px;flex-shrink:0;background:rgba(255,255,255,.06);border:1px solid rgba(255,255,255,.1);border-radius:8px;color:rgba(255,255,255,.5);font-size:14px;cursor:pointer;display:flex;align-items:center;justify-content:center;transition:background .15s,color .15s}
  .btn-pick:hover{background:rgba(255,255,255,.1);color:rgba(255,255,255,.9)}

  /* B3: Опции */
  .opts{display:flex;flex-direction:column;gap:5px}
  .opt-check{display:flex;align-items:center;gap:7px;cursor:pointer}
  .opt-check input{accent-color:#ff3d3d;width:12px;height:12px;cursor:pointer;flex-shrink:0}
  .opt-check span{font-size:11px;color:rgba(255,255,255,.45)}
  .opt-check:hover span{color:rgba(255,255,255,.7)}

  /* Уже установлено */
  .installed-block{display:flex;align-items:center;gap:14px;margin-bottom:10px}
  .installed-icon svg{width:36px;height:36px;color:#3dff7a}
  .installed-text{display:flex;flex-direction:column;gap:3px}
  .installed-title{font-size:16px;font-weight:700;color:#fff}
  .installed-ver{font-size:11px;color:rgba(255,255,255,.4)}
  .installed-dir{font-size:10px;color:rgba(255,255,255,.2);font-family:monospace;margin-bottom:4px}

  /* Прогресс */
  .step-text{font-size:13px;color:rgba(255,255,255,.5);min-height:18px;margin-bottom:10px}
  .track{position:relative;height:3px;background:rgba(255,255,255,.08);border-radius:2px;overflow:hidden}
  .fill{height:100%;background:linear-gradient(90deg,#ff3d3d,#ff6b3d);border-radius:2px;transition:width .4s cubic-bezier(.4,0,.2,1)}
  .fill-warn{background:linear-gradient(90deg,#ff8c00,#ffb300)}
  .shimmer{position:absolute;inset:0;background:linear-gradient(90deg,transparent 0%,rgba(255,255,255,.2) 50%,transparent 100%);animation:shimmer 1.4s ease-in-out infinite}
  @keyframes shimmer{0%{transform:translateX(-100%)}100%{transform:translateX(400%)}}
  .info-row{display:flex;align-items:center;justify-content:space-between;margin-top:8px}
  .dir{font-size:10px;color:rgba(255,255,255,.2);font-family:monospace;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:280px}
  .pct-label{font-size:10px;color:rgba(255,255,255,.3);font-variant-numeric:tabular-nums;flex-shrink:0}

  /* Done */
  .done-block{display:flex;flex-direction:column;gap:6px;margin-bottom:8px}
  .done-icon{width:44px;height:44px;border-radius:14px;display:flex;align-items:center;justify-content:center;margin-bottom:6px}
  .done-icon svg{width:24px;height:24px}
  .done-success{background:rgba(61,255,122,.1);color:#3dff7a}
  .done-muted{background:rgba(255,255,255,.06);color:rgba(255,255,255,.4)}
  .done-error{background:rgba(255,68,68,.1);color:#ff6b6b}
  .done-warn{background:rgba(255,165,0,.1);color:#ffa53d}
  .done-title{font-size:20px;font-weight:700;color:#fff}
  .done-sub{font-size:12px;color:rgba(255,255,255,.4)}
  .error-msg{color:#ff6b6b;font-family:monospace;font-size:11px;word-break:break-all}
  .status-text{font-size:13px;color:rgba(255,255,255,.3)}

  /* B7: Changelog */
  .changelog-header{display:flex;align-items:center;gap:14px;margin-bottom:10px}
  .changelog-scroll{flex:1;background:rgba(255,255,255,.04);border:1px solid rgba(255,255,255,.07);border-radius:10px;padding:14px 16px;overflow-y:auto;font-size:11px;color:rgba(255,255,255,.45);line-height:1.7;white-space:pre-wrap;font-family:monospace;min-height:0;max-height:160px}

  /* Кнопки */
  .actions{margin-top:16px;display:flex;gap:8px;flex-wrap:wrap}
  .btn-primary{height:38px;padding:0 24px;background:linear-gradient(135deg,#ff3d3d,#ff6b3d);border:none;border-radius:10px;color:#fff;font-size:13px;font-weight:600;cursor:pointer;transition:opacity .15s,transform .1s;box-shadow:0 4px 14px rgba(255,61,61,.3)}
  .btn-primary:hover:not(:disabled){opacity:.9;transform:translateY(-1px)}
  .btn-primary:active:not(:disabled){transform:translateY(0)}
  .btn-primary:disabled{opacity:.35;cursor:not-allowed}
  .btn-ghost{height:38px;padding:0 18px;background:rgba(255,255,255,.05);border:1px solid rgba(255,255,255,.1);border-radius:10px;color:rgba(255,255,255,.5);font-size:12px;cursor:pointer;transition:background .15s,color .15s}
  .btn-ghost:hover{background:rgba(255,255,255,.08);color:rgba(255,255,255,.8)}
  .btn-danger{height:38px;padding:0 18px;background:rgba(255,68,68,.08);border:1px solid rgba(255,68,68,.2);border-radius:10px;color:rgba(255,100,100,.8);font-size:12px;cursor:pointer;transition:background .15s,color .15s}
  .btn-danger:hover{background:rgba(255,68,68,.15);color:rgba(255,120,120,1)}

  /* Правая колонка */
  .right{width:188px;flex-shrink:0;padding:44px 28px 36px 28px;display:flex;flex-direction:column;gap:10px;border-left:1px solid rgba(255,255,255,.05)}
  .comp-title{font-size:9px;font-weight:700;text-transform:uppercase;letter-spacing:.12em;color:rgba(255,255,255,.18);margin-bottom:2px}
  .comps{display:flex;flex-direction:column;gap:5px}
  .badge{display:inline-flex;align-items:center;gap:6px;padding:5px 9px;border-radius:7px;font-size:11px;font-weight:500;border:1px solid transparent;transition:background .25s,border-color .25s,color .25s}
  .badge-pending{background:rgba(255,255,255,.03);border-color:rgba(255,255,255,.06);color:rgba(255,255,255,.2)}
  .badge-active{background:rgba(255,165,61,.1);border-color:rgba(255,165,61,.25);color:rgba(255,165,61,.9)}
  .badge-done{background:rgba(61,255,122,.07);border-color:rgba(61,255,122,.18);color:rgba(61,255,122,.75)}
  .badge-error{background:rgba(255,68,68,.08);border-color:rgba(255,68,68,.2);color:rgba(255,100,100,.9)}
  .bi{width:9px;height:9px;flex-shrink:0}
  .bdot{width:5px;height:5px;border-radius:50%;background:currentColor;flex-shrink:0;animation:pulse 1s ease-in-out infinite}
  @keyframes pulse{0%,100%{opacity:1}50%{opacity:.35}}

  /* Мета */
  .meta{display:flex;align-items:center;gap:6px;flex-wrap:wrap}
  .meta span{font-size:10px;color:rgba(255,255,255,.18)}
  .meta-link{cursor:pointer;transition:color .15s}
  .meta-link:hover{color:rgba(255,255,255,.5) !important}

  /* Частицы */
  .particle{position:absolute;border-radius:50%;background:linear-gradient(135deg,#ff3d3d,#ff6b3d);opacity:0;animation:float-up 2.5s ease-out forwards;pointer-events:none}
  @keyframes float-up{0%{opacity:.7;transform:translateY(0) scale(1)}100%{opacity:0;transform:translateY(-120px) scale(0)}}
</style>
