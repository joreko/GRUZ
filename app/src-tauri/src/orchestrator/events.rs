use serde::Serialize;

/// События которые оркестратор шлёт в WebView
pub enum OrchestratorEvent {
    QueueUpdated,
    Thought(Thought),
}

/// Мысль оркестратора — отображается в центральной панели TitleBar
#[derive(Serialize, Clone)]
pub struct Thought {
    pub text: String,
    /// hex-цвет: "#a1a1aa" серый, "#4ade80" зелёный, "#f87171" красный, "#fbbf24" жёлтый
    pub color: String,
    /// 0=CHATTER, 1=EVENT, 2=CRITICAL
    pub priority: u8,
}

