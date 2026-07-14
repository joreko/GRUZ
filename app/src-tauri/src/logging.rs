use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tauri::{AppHandle, Emitter};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt,
    layer::{Context, SubscriberExt},
    reload,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Максимальный размер одного лог-файла до ротации (5 МБ).
const MAX_BYTES: u64 = 5 * 1024 * 1024;
/// Сколько архивных файлов хранить (плюс текущий `gruz.log`).
const MAX_FILES: usize = 10;

/// Type-erased сеттер уровня логов: захватывает конкретный `reload::Handle`
/// (его тип дженерик по подписчику, хранить явно неудобно) в замыкание.
static LOG_SETTER: OnceLock<Arc<dyn Fn(&str) + Send + Sync>> = OnceLock::new();

/// Кольцевой буфер последних строк лога для живого просмотра в приложении.
const MAX_LINES: usize = 3000;
static LOG_RING: OnceLock<Arc<Mutex<VecDeque<String>>>> = OnceLock::new();
/// AppHandle, заполняется в setup() — нужен чтобы эмитить событие `log:line`.
static LOG_HANDLE: OnceLock<Arc<Mutex<Option<AppHandle>>>> = OnceLock::new();

/// Поменять уровень логирования на лету. `level` — директива фильтра
/// (например `gruz=debug`, `gruz=trace`, `gruz=warn`, `off`).
pub fn set_log_level(level: &str) {
    if let Some(f) = LOG_SETTER.get() {
        f(level);
    }
}

/// Сохранить AppHandle, чтобы live-слой мог эмитить события `log:line`.
pub fn set_app_handle(handle: &AppHandle) {
    if let Some(h) = LOG_HANDLE.get() {
        *h.lock().unwrap() = Some(handle.clone());
    }
}

/// История лога (последние `MAX_LINES` строк) для подгрузки при открытии страницы.
pub fn get_log_history() -> Vec<String> {
    LOG_RING
        .get()
        .map(|r| r.lock().unwrap().iter().cloned().collect())
        .unwrap_or_default()
}

/// Писатель, который ротирует файл по размеру и хранит последние `MAX_FILES`
/// архивов вида `gruz_<timestamp>.log`. Текущий файл всегда `gruz.log`.
struct SizeRollingWriter {
    dir: PathBuf,
    prefix: String,
    max_bytes: u64,
    max_files: usize,
    file: Option<File>,
    written: u64,
}

impl SizeRollingWriter {
    fn new(dir: &Path, prefix: &str, max_bytes: u64, max_files: usize) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let mut w = Self {
            dir: dir.to_path_buf(),
            prefix: prefix.to_string(),
            max_bytes,
            max_files,
            file: None,
            written: 0,
        };
        w.roll()?;
        w.cleanup();
        Ok(w)
    }

    fn current_path(&self) -> PathBuf {
        self.dir.join(format!("{}.log", self.prefix))
    }

    fn roll(&mut self) -> io::Result<()> {
        if let Some(mut f) = self.file.take() {
            let _ = f.flush();
            let ts = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
            let archive = self.dir.join(format!("{}_{}.log", self.prefix, ts));
            let _ = fs::rename(self.current_path(), &archive);
        }
        let f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.current_path())?;
        self.file = Some(f);
        self.written = 0;
        Ok(())
    }

    fn cleanup(&self) {
        if let Ok(entries) = fs::read_dir(&self.dir) {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    n.starts_with(&format!("{}_", self.prefix)) && n.ends_with(".log")
                })
                .collect();
            files.sort();
            if files.len() > self.max_files {
                for old in &files[..files.len() - self.max_files] {
                    let _ = fs::remove_file(old);
                }
            }
        }
    }
}

impl Write for SizeRollingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written + buf.len() as u64 > self.max_bytes {
            self.roll()?;
            self.cleanup();
        }
        match self.file.as_mut() {
            Some(f) => {
                let n = f.write(buf)?;
                self.written += n as u64;
                Ok(n)
            }
            None => Ok(0),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(f) = self.file.as_mut() {
            f.flush()?;
        }
        Ok(())
    }
}

/// Слой, дублирующий каждое событие в кольцевой буфер и шлющий Tauri-событие
/// `log:line` для живого просмотра лога в приложении.
#[derive(Clone)]
struct LiveLogLayer {
    ring: Arc<Mutex<VecDeque<String>>>,
    handle: Arc<Mutex<Option<AppHandle>>>,
}

struct LogVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl Visit for LogVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else {
            self.fields
                .push((field.name().to_string(), format!("{:?}", value)));
        }
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() != "message" {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() != "message" {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() != "message" {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        if field.name() != "message" {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }
}

impl<S> Layer<S> for LiveLogLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut visitor = LogVisitor {
            message: String::new(),
            fields: Vec::new(),
        };
        event.record(&mut visitor);
        let ts = chrono::Local::now().format("%H:%M:%S%.3f");
        // Схлопываем переносы строк/табуляции в один видимый маркер — иначе
        // многострочные сообщения (цепочки ошибок, stack trace) не парсятся
        // фронтом как одна строка и сваливаются в «сырой» вид.
        let msg = sanitize(&visitor.message);
        let mut line = format!("[{}] {} {}: {}", ts, meta.level(), meta.target(), msg);
        if !visitor.fields.is_empty() {
            let extra = visitor
                .fields
                .iter()
                .map(|(k, v)| format!("{}={}", k, sanitize(v)))
                .collect::<Vec<_>>()
                .join(" ");
            line.push_str(" | ");
            line.push_str(&extra);
        }
        // Цепочка span-ов (call stack) — даёт контекст, откуда пришла ошибка.
        if let Some(span) = ctx.lookup_current() {
            let mut chain: Vec<String> = Vec::new();
            let mut s = Some(span);
            while let Some(cur) = s {
                chain.push(cur.name().to_string());
                s = cur.parent();
            }
            chain.reverse();
            if !chain.is_empty() {
                line.push_str(&format!(" | span={}", chain.join(">")));
            }
        }
        if let Ok(mut ring) = self.ring.lock() {
            ring.push_back(line.clone());
            while ring.len() > MAX_LINES {
                ring.pop_front();
            }
        }
        if let Some(h) = self.handle.lock().ok().and_then(|g| g.clone()) {
            let _ = h.emit("log:line", &line);
        }
    }
}

/// Заменяет переносы строк и табуляцию на видимый маркер, чтобы событие
/// всегда оставалось одной строкой (удобно для парсинга и вывода).
fn sanitize(s: &str) -> String {
    s.replace('\r', "").replace('\n', " ⏎ ").replace('\t', " ")
}

/// Инициализировать логирование. Возвращает guard — должен жить до конца
/// работы приложения (его Drop сбрасывает буфер non_blocking).
pub fn init_logging() -> WorkerGuard {
    let log_dir = dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gruz")
        .join("logs");

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,gruz=debug"));
    let (reload_layer, handle) = reload::Layer::new(filter);

    let (nb_file, guard) = match SizeRollingWriter::new(&log_dir, "gruz", MAX_BYTES, MAX_FILES) {
        Ok(w) => tracing_appender::non_blocking(w),
        Err(e) => {
            eprintln!("лог-файл недоступен ({e}), пишем в stderr");
            tracing_appender::non_blocking(std::io::stderr())
        }
    };

    // Структурированный JSON в файл — машиночитаемый, удобно искать/парсить.
    let file_layer = fmt::layer()
        .json()
        .with_writer(nb_file)
        .with_current_span(true)
        .with_span_list(true);

    // Человекочитаемый цветной вывод в терминал (дублирует файл, всегда вкл).
    let console_layer = fmt::layer()
        .pretty()
        .with_target(false)
        .with_thread_ids(false);

    // Живой лог: кольцевой буфер + событие `log:line` для просмотра в приложении.
    let ring = Arc::new(Mutex::new(VecDeque::new()));
    LOG_RING.get_or_init(|| ring.clone());
    let handle_holder = Arc::new(Mutex::new(None));
    LOG_HANDLE.get_or_init(|| handle_holder.clone());
    let live_layer = LiveLogLayer {
        ring,
        handle: handle_holder,
    };

    let registry = tracing_subscriber::registry()
        .with(reload_layer)
        .with(file_layer)
        .with(console_layer)
        .with(live_layer);

    registry.init();

    // Panic — логируем через tracing (попадает в JSON-файл и live-лог).
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("PANIC: {info}");
        default_hook(info);
    }));

    // Захватываем handle в type-erased замыкание для смены уровня на лету.
    LOG_SETTER.get_or_init(|| {
        Arc::new(move |level: &str| {
            let new_filter =
                EnvFilter::try_from(level).unwrap_or_else(|_| EnvFilter::new("warn,gruz=debug"));
            let _ = handle.reload(new_filter);
        }) as Arc<dyn Fn(&str) + Send + Sync>
    });

    guard
}
