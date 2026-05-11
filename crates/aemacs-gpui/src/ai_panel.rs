use std::{
    fmt::Write as _,
    path::Path,
    sync::{Arc, OnceLock},
};

// --- Modal Bridge (ACO-023) ---
use aemacs_ai::rag::KnowledgeBase;
use aemacs_ai::{
    AIBackend, Conversation, PersonaRegistry,
    connectors::openai_compatible::OpenAICompatibleBackend,
    loader::load_file,
    mcp::{ToolHost, ToolRegistry, validate_path},
};
use aemacs_core::{Editor, command::Command, mode::Mode};
use async_trait::async_trait;
use gpui::{
    App, AsyncApp, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, WeakEntity, Window,
    div, prelude::*, px, relative, rgb,
};
use regex::Regex;

use crate::{
    ai_utils::{AgentEvent, spawn_agent_task},
    editor_view::render_editor_view,
    input_handler::resolve_key_command,
};

/// Represents a request from the AI Mesh to the host UI environment.
/// This is used to prompt the user for approvals or additional information.
pub(crate) enum HostRequest {
    /// Requests explicit user approval for a tool action.
    Approval {
        /// Description of the action requiring approval.
        description: String,
        /// Channel to send the user's decision back to the agent.
        responder: futures::channel::oneshot::Sender<bool>,
    },
    /// Prompts the user for a textual response to a question.
    UserPrompt {
        /// The question being asked by the agent.
        question: String,
        /// Channel to send the user's response back to the agent.
        responder: futures::channel::oneshot::Sender<String>,
    },
}

/// Implements the `ToolHost` trait for the graphical interface.
/// It bridges the asynchronous agent execution with the synchronous GPUI windowing system.
pub(crate) struct GuiHost {
    /// Channel for sending requests to the UI thread.
    pub request_tx: async_channel::Sender<HostRequest>,
    /// The system event bus for emitting telemetry signals.
    pub event_tx: tokio::sync::broadcast::Sender<aemacs_core::bus::SystemEvent>,
    /// The unique identifier of the agent being hosted.
    pub agent_id: String,
}

#[async_trait]
impl ToolHost for GuiHost {
    /// Displays a warning prompt to the user requesting approval for a specific action.
    async fn ask_approval(&self, description: &str) -> bool {
        let (tx, rx) = futures::channel::oneshot::channel();
        let _ = self
            .request_tx
            .send(HostRequest::Approval { description: description.to_string(), responder: tx })
            .await;
        rx.await.unwrap_or(false)
    }

    /// Displays an informational prompt to the user requesting textual input.
    async fn ask_user(&self, question: &str) -> String {
        let (tx, rx) = futures::channel::oneshot::channel();
        let _ = self
            .request_tx
            .send(HostRequest::UserPrompt { question: question.to_string(), responder: tx })
            .await;
        rx.await.unwrap_or_default()
    }

    /// Retrieves the ID of the agent associated with this host.
    fn get_agent_id(&self) -> String {
        self.agent_id.clone()
    }

    /// Emits a progress signal to the event bus, typically used to update the "Currently Executing" UI.
    fn report_progress(&self, tool_name: String, is_running: bool) {
        let signal = aemacs_core::signals::ToolProgressSignal { tool_name, is_running };
        if let Ok(payload) = serde_json::to_string(&signal) {
            let _ = self.event_tx.send(aemacs_core::bus::SystemEvent::Signal {
                source: "Specialist".to_string(),
                event_type: "ToolProgress".to_string(),
                payload,
            });
        }
    }

    /// Emits a generic signal from the agent to the system event bus.
    async fn emit_signal(&self, event_type: String, payload: String) -> anyhow::Result<()> {
        let event = aemacs_core::bus::SystemEvent::Signal {
            source: format!("Agent:{}", self.agent_id),
            event_type,
            payload,
        };
        let _ = self.event_tx.send(event);
        Ok(())
    }
}

/// The `AiPanel` is the primary interactive interface for the Æmacs Mesh.
/// It handles model selection, agent switching, message history, and the integrated chat interface.
pub(crate) struct AiPanel {
    /// The editor entity used for the user's chat input.
    pub input_editor: Entity<Editor>,
    /// State for the chat input's list view.
    pub input_list_state: gpui::ListState,
    /// Manages focus for the AI panel.
    pub focus_handle: FocusHandle,
    /// Controls scrolling for the message history area.
    pub message_scroll_handle: gpui::ScrollHandle,
    /// The chronological list of messages displayed in the panel.
    messages: Vec<ChatMessage>,
    /// The primary AI backend for the panel.
    backend: OpenAICompatibleBackend,
    /// Channel for sending requests to the UI.
    host_tx: async_channel::Sender<HostRequest>,
    /// The system event bus for signal emission.
    event_tx: tokio::sync::broadcast::Sender<aemacs_core::bus::SystemEvent>,
    /// The active project roadmap tasks.
    tasks: Vec<aemacs_core::task::Task>,
    /// The currently selected hardware tier.
    selected_tier: aemacs_ai::models::ModelTier,
    /// The currently selected context window size.
    selected_context: u32,
    /// Reference to the RAG memory system.
    pub kb: Arc<KnowledgeBase>,
    /// Reference to the global tool registry.
    pub registry: Arc<ToolRegistry>,
    /// Reference to the agent persona registry.
    pub persona_registry: Arc<PersonaRegistry>,
    /// The name of the currently active agent persona.
    pub active_persona_name: Option<String>,
    /// The underlying conversation state machine.
    pub conversation: Conversation,
    /// Whether the panel automatically reacts to environment changes.
    pub proactive_mode: bool,
    /// The current physical width of the panel.
    pub width: f32,
    /// Whether the panel is expanded to fill the entire window.
    pub is_maximized: bool,
    /// Description of the agent's current background action.
    pub current_action: Option<String>,
    /// The internal cognitive state of the agent mesh.
    pub status: CognitiveStatus,
    /// Animation offset for the cognition pulse visual effect.
    pub shimmer_offset: f32,
    /// Handle to the parent window.
    pub window_handle: gpui::AnyWindowHandle,
    /// List of models available for the current hardware tier.
    pub available_models: Vec<&'static aemacs_ai::models::ModelDefinition>,
    /// List of all agent personas available in the registry.
    pub available_personas: Vec<String>,
    /// Tracks the previous line index of the input cursor.
    last_cursor_line: usize,
}

/// Defines the possible states of agent activity, determining the appearance of the cognition pulse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CognitiveStatus {
    /// The agent is inactive and waiting for input.
    Idle,
    /// The logic hemisphere is performing analysis or tool execution.
    Thinking,
    /// The voice hemisphere is actively generating output text.
    Streaming,
    /// An error occurred during the last operation.
    Errored(String),
}

pub(crate) struct AiPanelInit {
    pub window_handle: gpui::AnyWindowHandle,
    pub host_tx: async_channel::Sender<HostRequest>,
    pub kb: Arc<KnowledgeBase>,
    pub registry: Arc<ToolRegistry>,
    pub persona_registry: Arc<PersonaRegistry>,
    pub bus: aemacs_core::bus::EventBus,
    pub available_models: Vec<&'static aemacs_ai::models::ModelDefinition>,
}

/// Represents a single message in the chat UI, with support for markdown rendering.
struct ChatMessage {
    /// The role of the sender (e.g., "AI", "User", "System").
    role: String,
    /// The raw content of the message.
    content: String,
    /// Pre-parsed markdown blocks for efficient rendering.
    parsed_blocks: Vec<MarkdownBlock>,
}

/// Defines the supported block types for the internal markdown renderer.
#[derive(Clone, Debug)]
enum MarkdownBlock {
    /// A standard paragraph of text.
    Paragraph(String),
    /// a syntax-highlighted code block.
    Code { language: String, content: String },
    /// A markdown header at a specific level.
    Header { level: usize, content: String },
}

impl ChatMessage {
    fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        let parsed_blocks = parse_markdown_blocks(&content);
        Self { role: role.into(), content, parsed_blocks }
    }

    fn update_content(&mut self, new_content: String) {
        self.content = new_content;
        self.parsed_blocks = parse_markdown_blocks(&self.content);
    }
}

fn parse_markdown_blocks(text: &str) -> Vec<MarkdownBlock> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let parser = Parser::new(text);
    let mut blocks = Vec::new();
    let mut current_text = String::new();
    let mut in_code_block = false;
    let mut current_language = String::new();
    let mut current_header_level = 0;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(lang))) => {
                if !current_text.is_empty() {
                    blocks.push(MarkdownBlock::Paragraph(current_text.trim().to_string()));
                    current_text.clear();
                }
                in_code_block = true;
                current_language = lang.to_string();
            },
            Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Indented)) => {
                if !current_text.is_empty() {
                    blocks.push(MarkdownBlock::Paragraph(current_text.trim().to_string()));
                    current_text.clear();
                }
                in_code_block = true;
                current_language = String::new();
            },
            Event::End(TagEnd::CodeBlock) => {
                blocks.push(MarkdownBlock::Code {
                    language: current_language.clone(),
                    content: current_text.trim_end().to_string(),
                });
                current_text.clear();
                in_code_block = false;
            },
            Event::Start(Tag::Heading { level, .. }) => {
                if !current_text.is_empty() {
                    blocks.push(MarkdownBlock::Paragraph(current_text.trim().to_string()));
                    current_text.clear();
                }
                current_header_level = match level {
                    pulldown_cmark::HeadingLevel::H1 => 1,
                    pulldown_cmark::HeadingLevel::H2 => 2,
                    pulldown_cmark::HeadingLevel::H3 => 3,
                    pulldown_cmark::HeadingLevel::H4 => 4,
                    pulldown_cmark::HeadingLevel::H5 => 5,
                    pulldown_cmark::HeadingLevel::H6 => 6,
                };
            },
            Event::End(TagEnd::Heading(_)) => {
                blocks.push(MarkdownBlock::Header {
                    level: current_header_level,
                    content: current_text.trim().to_string(),
                });
                current_text.clear();
                current_header_level = 0;
            },
            Event::Text(t) => {
                current_text.push_str(&t);
            },
            Event::Code(t) => {
                current_text.push('`');
                current_text.push_str(&t);
                current_text.push('`');
            },
            Event::SoftBreak | Event::HardBreak => {
                current_text.push('\n');
            },
            _ => {},
        }
    }

    if !current_text.is_empty() {
        if in_code_block {
            blocks.push(MarkdownBlock::Code {
                language: current_language,
                content: current_text.trim_end().to_string(),
            });
        } else {
            blocks.push(MarkdownBlock::Paragraph(current_text.trim().to_string()));
        }
    }

    blocks
}

fn statute_regex() -> Result<&'static Regex, regex::Error> {
    static STATUTE_REGEX: OnceLock<Result<Regex, regex::Error>> = OnceLock::new();
    match STATUTE_REGEX.get_or_init(|| Regex::new(r"@([^@\s]+)")) {
        Ok(regex) => Ok(regex),
        Err(error) => Err(error.clone()),
    }
}

fn material_regex() -> Result<&'static Regex, regex::Error> {
    static MATERIAL_REGEX: OnceLock<Result<Regex, regex::Error>> = OnceLock::new();
    match MATERIAL_REGEX.get_or_init(|| Regex::new(r"@@([^@\s]+)")) {
        Ok(regex) => Ok(regex),
        Err(error) => Err(error.clone()),
    }
}

impl AiPanel {
    pub(crate) fn new(cx: &mut App, init: AiPanelInit) -> Entity<Self> {
        let backend = Self::create_backend();
        let initial_tier = aemacs_ai::models::ModelTier::Low;
        let logic_model = Self::select_initial_model(&init.available_models, initial_tier);
        let initial_model_name = logic_model.name;
        let initial_context = logic_model.max_context;
        let panel = Self::build_panel_entity(
            cx,
            init,
            backend.clone(),
            initial_tier,
            initial_model_name,
            initial_context,
        );

        Self::spawn_persona_loader(cx, &panel);
        Self::spawn_triage_listener(cx, &panel);
        Self::spawn_backend_health_check(cx, &panel, &backend);
        panel
    }

    fn create_backend() -> OpenAICompatibleBackend {
        OpenAICompatibleBackend::new("http://localhost:11434/v1", None).unwrap_or_else(|error| {
            log::error!("Failed to initialize AI backend: {error}");
            std::process::exit(1);
        })
    }

    fn select_initial_model(
        available_models: &[&'static aemacs_ai::models::ModelDefinition],
        initial_tier: aemacs_ai::models::ModelTier,
    ) -> &'static aemacs_ai::models::ModelDefinition {
        available_models
            .iter()
            .find(|m| m.tier == initial_tier && m.role == aemacs_ai::models::ModelRole::Logic)
            .or_else(|| available_models.first())
            .copied()
            .unwrap_or_else(|| {
                log::error!("No models available in registry");
                std::process::exit(1);
            })
    }

    fn build_panel_entity(
        cx: &mut App,
        init: AiPanelInit,
        backend: OpenAICompatibleBackend,
        initial_tier: aemacs_ai::models::ModelTier,
        initial_model_name: &'static str,
        initial_context: u32,
    ) -> Entity<Self> {
        let AiPanelInit {
            window_handle,
            host_tx,
            kb,
            registry,
            persona_registry,
            bus,
            available_models,
        } = init;

        cx.new(|cx| {
            let input_editor = cx.new(|_cx| {
                let mut editor = Editor::new();
                let _ = editor.run(Command::EnterMode(Mode::Insert));
                editor
            });

            Self {
                input_editor,
                input_list_state: gpui::ListState::new(0, gpui::ListAlignment::Top, px(10.0)),
                focus_handle: cx.focus_handle(),
                message_scroll_handle: gpui::ScrollHandle::new(),
                messages: vec![ChatMessage::new(
                    "System",
                    "Bicameral AI System Online. Select your hardware tier to ignite the Forge.",
                )],
                backend,
                host_tx,
                event_tx: bus.tx.clone(),
                tasks: Vec::new(),
                selected_tier: initial_tier,
                selected_context: initial_context,
                kb,
                registry,
                persona_registry,
                active_persona_name: None,
                conversation: Conversation::new(initial_model_name),
                proactive_mode: true,
                width: 400.0,
                is_maximized: false,
                current_action: None,
                status: CognitiveStatus::Idle,
                shimmer_offset: 0.0,
                window_handle,
                available_models,
                available_personas: Vec::new(),
                last_cursor_line: 0,
            }
        })
    }

    fn spawn_persona_loader(cx: &mut App, panel: &Entity<Self>) {
        let cx = &mut *cx;
        let persona_registry = panel.read(cx).persona_registry.clone();
        let panel_weak = panel.downgrade();
        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                let personas = persona_registry.list_personas().await;
                let _ = cx.update(|app: &mut App| {
                    if let Some(panel) = panel_weak.upgrade() {
                        panel.update(app, |this, cx| {
                            this.available_personas = personas;
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn spawn_triage_listener(cx: &mut App, panel: &Entity<Self>) {
        let cx = &mut *cx;
        let mut rx = panel.read(cx).event_tx.subscribe();
        let panel_weak = panel.downgrade();
        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Ok(event) = rx.recv().await {
                    if let aemacs_core::bus::SystemEvent::Signal { source, event_type, payload } =
                        event
                    {
                        let _ = cx.update(|app: &mut App| {
                            if let Some(panel) = panel_weak.upgrade() {
                                panel.update(app, |this, cx| {
                                    this.handle_signal_event(&source, &event_type, &payload, cx);
                                });
                            }
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn handle_signal_event(
        &mut self,
        source: &str,
        event_type: &str,
        payload: &str,
        cx: &mut Context<'_, Self>,
    ) {
        if event_type == "ToolProgress" {
            if let Ok(progress) =
                serde_json::from_str::<aemacs_core::signals::ToolProgressSignal>(payload)
            {
                self.current_action =
                    progress.is_running.then(|| format!("Executing {}...", progress.tool_name));
                cx.notify();
            }
            return;
        }

        if !self.proactive_mode || self.is_ai_busy() || event_type != "BufferModified" {
            return;
        }

        self.handle_buffer_modified(source, payload, cx);
    }

    fn is_ai_busy(&self) -> bool {
        matches!(self.messages.last(), Some(last_msg) if last_msg.role == "AI" && last_msg.content.is_empty())
    }

    fn handle_buffer_modified(&mut self, source: &str, payload: &str, cx: &mut Context<'_, Self>) {
        let observation = format!(
            "Observation (from {source}): The following file was just modified:\n{payload}"
        );

        self.messages
            .push(ChatMessage::new("System", "👀 I noticed you changed a file. Let me look..."));
        self.conversation.add_message(aemacs_ai::Message::user(observation));

        if self.active_persona_name.is_none() {
            self.spawn_default_marjin_response(cx);
        } else {
            self.trigger_ai_response(cx);
        }
    }

    fn spawn_default_marjin_response(&self, cx: &mut Context<'_, Self>) {
        let cx = &mut *cx;
        let persona_registry = self.persona_registry.clone();
        cx.spawn(|panel: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                if let Some(persona) = persona_registry.get_persona("marjin").await {
                    let _ = panel.update(&mut cx, |this, cx: &mut Context<'_, Self>| {
                        this.active_persona_name = Some("marjin".to_string());
                        this.conversation.set_persona(persona);
                        this.trigger_ai_response(cx);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn spawn_backend_health_check(
        cx: &mut App,
        panel: &Entity<Self>,
        backend: &OpenAICompatibleBackend,
    ) {
        let cx = &mut *cx;
        let backend = backend.clone();
        let panel_weak = panel.downgrade();
        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                let check_result = aemacs_core::runtime::Tokio::spawn(&cx, async move {
                    backend.health_check().await
                });

                if let Ok(task) = check_result
                    && let Ok(Err(_)) = task.await
                {
                    let _ = cx.update(|app: &mut App| {
                        if let Some(panel) = panel_weak.upgrade() {
                            let () = panel.update(app, |this, cx| {
                                this.messages.push(ChatMessage::new(
                                    "System",
                                    "❌ [BACKEND OFFLINE] Ollama is not responding at http://localhost:11434/v1. Specialists are currently disabled.",
                                ));
                                cx.notify();
                            });
                        }
                    });
                }
            }
        })
        .detach();
    }

    /// Updates the local task list (ACO-034)
    pub(crate) fn update_tasks(
        &mut self,
        tasks: Vec<aemacs_core::task::Task>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks = tasks;
        cx.notify();
    }

    /// Handles a programmatic persona switch from the dispatcher (ACO-010).
    pub(crate) fn handoff_persona(
        &mut self,
        name: &str,
        message: Option<String>,
        cx: &mut Context<'_, Self>,
    ) {
        let name_lower = name.to_lowercase();
        if let Some(persona) =
            futures::executor::block_on(self.persona_registry.get_persona(&name_lower))
        {
            self.active_persona_name = Some(name_lower);
            self.conversation.set_persona(persona);

            self.messages.push(ChatMessage::new(
                "System",
                format!("Programmatic handoff to: {}", name.to_uppercase()),
            ));

            if let Some(msg) = message {
                self.messages.push(ChatMessage::new("User", msg.clone()));
                self.conversation.add_message(aemacs_ai::Message::user(msg));
                // Automatically trigger the new agent if a message was provided
                self.trigger_ai_response(cx);
            }
            cx.notify();
        }
    }

    fn handle_statute_tags(&mut self, text: &mut String, cx: &mut Context<'_, Self>) -> bool {
        let statute_regex = match statute_regex() {
            Ok(regex) => regex,
            Err(error) => {
                self.messages.push(ChatMessage::new(
                    "System",
                    format!("⚠️ Failed to initialize statute parser: {error}"),
                ));
                cx.notify();
                return false;
            },
        };

        let statute_matches: Vec<_> = statute_regex
            .find_iter(text)
            .filter(|m| {
                let start = m.start();
                start == 0 || text.as_bytes()[start - 1] != b'@'
            })
            .collect();

        if statute_matches.len() > 1 {
            self.messages.push(ChatMessage::new(
                "System",
                "❌ Multiple statutes detected! You can only load one profile (@) at a time. Use @@ for attaching multiple source files.",
            ));
            cx.notify();
            return false;
        }

        if let Some((path_str, start, end)) = statute_matches
            .first()
            .map(|mat| (mat.as_str().trim_start_matches('@').to_string(), mat.start(), mat.end()))
        {
            self.load_profile_tag(&path_str, start, end, text);
        }

        true
    }

    fn load_profile_tag(&mut self, path_str: &str, start: usize, end: usize, text: &mut String) {
        match validate_path(path_str) {
            Ok(path) => match load_file(&path) {
                Ok(aemacs_ai::ContentPart::Text { text: profile_content }) => {
                    self.conversation.set_profile(profile_content);
                    self.messages
                        .push(ChatMessage::new("System", format!("📖 Profile loaded: {path_str}")));
                    text.replace_range(start..end, "");
                    *text = text.trim().to_string();
                },
                Ok(_) => {
                    self.messages.push(ChatMessage::new(
                        "System",
                        format!("⚠️ Profile at '{path_str}' is not a text file."),
                    ));
                },
                Err(error) => {
                    self.messages.push(ChatMessage::new(
                        "System",
                        format!("⚠️ Failed to load profile: {path_str} ({error})"),
                    ));
                },
            },
            Err(error) => {
                self.messages.push(ChatMessage::new(
                    "System",
                    format!("⚠️ Invalid profile path: {path_str} ({error})"),
                ));
            },
        }
    }

    fn handle_material_tags(&mut self, text: &mut String, cx: &mut Context<'_, Self>) -> bool {
        let material_regex = match material_regex() {
            Ok(regex) => regex,
            Err(error) => {
                self.messages.push(ChatMessage::new(
                    "System",
                    format!("⚠️ Failed to initialize material parser: {error}"),
                ));
                cx.notify();
                return false;
            },
        };

        let material_matches: Vec<(usize, usize, String)> = material_regex
            .find_iter(text)
            .map(|m| (m.start(), m.end(), m.as_str().to_string()))
            .collect();
        let mut attachments = Vec::new();

        for (start, end, mat_str) in material_matches.iter().rev() {
            let path_str = mat_str.trim_start_matches("@@");
            match validate_path(path_str) {
                Ok(path) => match load_file(&path) {
                    Ok(aemacs_ai::ContentPart::Text { text: file_content }) => {
                        attachments.push((path_str.to_string(), file_content));
                        text.replace_range(*start..*end, "");
                    },
                    Ok(_) => {
                        self.messages.push(ChatMessage::new(
                            "System",
                            format!("⚠️ Material at '{path_str}' is not a text file."),
                        ));
                    },
                    Err(error) => {
                        self.messages.push(ChatMessage::new(
                            "System",
                            format!("⚠️ Failed to load material: {path_str} ({error})"),
                        ));
                    },
                },
                Err(error) => {
                    self.messages.push(ChatMessage::new(
                        "System",
                        format!("⚠️ Invalid material path: {path_str} ({error})"),
                    ));
                },
            }
        }

        if !attachments.is_empty() {
            Self::append_attachments(text, attachments);
        }

        true
    }

    fn append_attachments(text: &mut String, attachments: Vec<(String, String)>) {
        *text = text.trim().to_string();
        text.push_str("\n\n---\n### ATTACHED CONTEXT\n");
        for (path, content) in attachments {
            let ext = Path::new(&path).extension().and_then(|s| s.to_str()).unwrap_or("");
            let _ = writeln!(text, "#### File: {path}");
            let _ = writeln!(text, "```{ext}");
            let _ = writeln!(text, "{content}");
            let _ = writeln!(text, "```");
        }
    }

    fn handle_slash_command(
        &mut self,
        text: &mut String,
        switched: &mut bool,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        if !text.starts_with('/') {
            return true;
        }

        let (cmd, remainder) = text.split_once(' ').unwrap_or((text.as_str(), ""));
        let agent_name = cmd.trim_start_matches('/').to_lowercase();

        if futures::executor::block_on(self.persona_registry.get_persona(&agent_name)).is_some() {
            self.active_persona_name = Some(agent_name);
            *text = remainder.to_string();
            *switched = true;
            return true;
        }

        self.messages.push(ChatMessage::new(
            "System",
            format!("⚠️ Unknown agent: /{agent_name}. Type a valid specialist name."),
        ));
        self.clear_input(cx);
        cx.notify();
        false
    }

    fn push_outgoing_message(&mut self, text: &str, switched: bool) -> bool {
        if !text.trim().is_empty() {
            self.messages.push(ChatMessage::new("User", text.to_string()));
            return true;
        }

        if switched {
            let persona_display =
                self.active_persona_name.as_deref().unwrap_or("UNKNOWN").to_uppercase();
            self.messages
                .push(ChatMessage::new("System", format!("Agent switched to: {persona_display}")));
            return true;
        }

        false
    }

    fn clear_input(&self, cx: &mut Context<'_, Self>) {
        self.input_editor.update(cx, |editor, _| {
            editor.clear();
            editor.mode = Mode::Normal;
        });
        cx.notify();
    }

    fn sync_active_persona(&mut self) {
        if let Some(persona_name) = &self.active_persona_name {
            if let Some(persona) =
                futures::executor::block_on(self.persona_registry.get_persona(persona_name))
            {
                self.conversation.set_persona(persona);
            }
        } else {
            self.conversation.clear_persona();
        }
    }

    fn trigger_ai_response(&mut self, cx: &mut Context<'_, Self>) {
        // Prepare AI Message Placeholder
        self.messages.push(ChatMessage::new("AI", ""));
        self.status = CognitiveStatus::Thinking;

        // ACO-040: Start recursive animation loop
        self.animate_pulse(cx);

        let registry = self.registry.clone();
        let host = GuiHost {
            request_tx: self.host_tx.clone(),
            event_tx: self.event_tx.clone(),
            agent_id: self.active_persona_name.clone().unwrap_or_else(|| "global".to_string()),
        };
        let backend = self.backend.clone();
        let conversation = self.conversation.clone();

        // Spawn Agent Task
        spawn_agent_task(
            cx,
            backend,
            registry,
            self.kb.clone(),
            host,
            conversation,
            |panel, cx, event| {
                match event {
                    AgentEvent::StreamChunk(chunk) => {
                        panel.status = CognitiveStatus::Streaming;
                        if let Some(last_msg) = panel.messages.last_mut()
                            && last_msg.role == "AI"
                        {
                            last_msg.update_content(last_msg.content.clone() + &chunk);
                        }
                        panel.message_scroll_handle.set_offset(gpui::point(px(0.0), px(999_999.0)));
                    },
                    AgentEvent::ToolStarted(name) => {
                        panel.current_action = Some(format!("Executing {name}..."));
                    },
                    AgentEvent::ToolFinished(_name, _success) => {
                        panel.current_action = None;
                    },
                    AgentEvent::Result(updated_conv) => {
                        panel.status = CognitiveStatus::Idle;
                        panel.conversation = *updated_conv;

                        // ACO-025: Synchronize UI messages with the updated conversation
                        // This ensures tool outputs are visible in the message area.
                        let conv_messages = panel.conversation.messages();
                        if conv_messages.len() > panel.messages.len() {
                            for m in conv_messages.iter().skip(panel.messages.len()) {
                                let role = match m.role {
                                    aemacs_ai::Role::System => "System",
                                    aemacs_ai::Role::User => "User",
                                    aemacs_ai::Role::Assistant => "AI",
                                    aemacs_ai::Role::Tool => "Tool",
                                };
                                let content = match &m.content {
                                    aemacs_ai::Content::Text(t) => t.clone(),
                                    aemacs_ai::Content::Parts(_) => String::new(),
                                };
                                panel.messages.push(ChatMessage::new(role, content));
                            }
                        }
                    },
                    AgentEvent::Error(e) => {
                        panel.status = CognitiveStatus::Errored(e.clone());
                        if let Some(last_msg) = panel.messages.last_mut()
                            && last_msg.role == "AI"
                        {
                            last_msg.role = "System".to_string();
                            last_msg.update_content(format!("❌ AI Error: {e}"));
                        }
                        log::error!("AI Task Failed: {e}");
                    },
                }
                cx.notify();
            },
        )
        .detach();
    }

    /// Schedules the next frame of the cognition pulse animation.
    fn animate_pulse(&self, cx: &mut Context<'_, Self>) {
        if matches!(self.status, CognitiveStatus::Idle) {
            return;
        }

        let window_handle = self.window_handle;
        let handle = cx.entity().downgrade();
        cx.notify();

        let _ = cx.update_window(window_handle, move |_, window, _cx| {
            window.on_next_frame(move |_, cx| {
                if let Some(panel) = handle.upgrade() {
                    let () = panel.update(cx, |this, cx| {
                        // Adjust speed based on status
                        let delta = match this.status {
                            CognitiveStatus::Thinking => 0.015,
                            _ => 0.025,
                        };
                        this.shimmer_offset = (this.shimmer_offset + delta) % 1.0;
                        cx.notify();
                        this.animate_pulse(cx);
                    });
                }
            });
        });
    }

    fn render_agent_selector(&self, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let cx = &mut *cx;
        let active_persona = self.active_persona_name.clone();

        div()
            .flex()
            .flex_col()
            .gap_y(px(4.0))
            .child(div().text_size(px(10.0)).text_color(rgb(0x005c_6370)).child("ACTIVE AGENT"))
            .child(div().flex().flex_wrap().gap(px(4.0)).children(
                self.available_personas.iter().enumerate().map(|(i, name)| {
                    let is_selected = active_persona.as_ref() == Some(name);
                    let name_clone = name.clone();
                    div()
                        .id(i)
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded_md()
                        .border_1()
                        .border_color(if is_selected { rgb(0x00bd_93f9) } else { rgb(0x003e_4451) })
                        .bg(if is_selected { rgb(0x0028_2c34) } else { rgb(0x0021_252b) })
                        .text_color(if is_selected { rgb(0x00ff_ffff) } else { rgb(0x00ab_b2bf) })
                        .text_size(px(11.0))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            let name_lower = name_clone.clone();
                            let persona_registry = this.persona_registry.clone();

                            cx.spawn(|panel: WeakEntity<Self>, cx: &mut AsyncApp| {
                                let mut cx = cx.clone();
                                async move {
                                    if let Some(persona) =
                                        persona_registry.get_persona(&name_lower).await
                                    {
                                        let _ = panel.update(
                                            &mut cx,
                                            |this, cx: &mut Context<'_, Self>| {
                                                this.active_persona_name = Some(name_lower);
                                                this.conversation.set_persona(persona);
                                                cx.notify();
                                            },
                                        );
                                    }
                                }
                            })
                            .detach();
                        }))
                        .child(name.to_uppercase())
                }),
            ))
    }

    fn render_tier_selector(&self, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let cx = &mut *cx;
        let selected_tier = self.selected_tier;

        div()
            .flex()
            .flex_col()
            .gap_y(px(4.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0x005c_6370))
                    .child("HARDWARE TIER (BICAMERAL LOCK)"),
            )
            .child(
                div().flex().flex_wrap().gap(px(4.0)).children(
                    [
                        aemacs_ai::models::ModelTier::Low,
                        aemacs_ai::models::ModelTier::Medium,
                        aemacs_ai::models::ModelTier::High,
                    ]
                    .iter()
                    .map(|&tier| {
                        let is_selected = tier == selected_tier;
                        let label = match tier {
                            aemacs_ai::models::ModelTier::Low => "LOW",
                            aemacs_ai::models::ModelTier::Medium => "MEDIUM",
                            aemacs_ai::models::ModelTier::High => "HIGH",
                        };

                        div()
                            .id(label)
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded_md()
                            .border_1()
                            .border_color(if is_selected {
                                rgb(0x00bd_93f9)
                            } else {
                                rgb(0x003e_4451)
                            })
                            .bg(if is_selected { rgb(0x0028_2c34) } else { rgb(0x0021_252b) })
                            .text_color(if is_selected {
                                rgb(0x00ff_ffff)
                            } else {
                                rgb(0x00ab_b2bf)
                            })
                            .text_size(px(11.0))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.selected_tier = tier;
                                // Update conversation with the LOGIC model of the new tier
                                if let Some(m) = this.available_models.iter().find(|m| {
                                    m.tier == tier && m.role == aemacs_ai::models::ModelRole::Logic
                                }) {
                                    this.conversation.set_model(m.name);
                                    this.selected_context = m.max_context;
                                    this.conversation.set_context_window(this.selected_context);
                                }
                                cx.notify();
                            }))
                            .child(label)
                    }),
                ),
            )
            .child(self.render_bicameral_info())
    }

    fn render_bicameral_info(&self) -> impl IntoElement {
        let tier = self.selected_tier;
        let logic_model = self
            .available_models
            .iter()
            .find(|m| m.tier == tier && m.role == aemacs_ai::models::ModelRole::Logic);
        let voice_model = self
            .available_models
            .iter()
            .find(|m| m.tier == tier && m.role == aemacs_ai::models::ModelRole::Roleplay);

        div()
            .mt(px(8.0))
            .p(px(8.0))
            .bg(rgb(0x0018_1a1f))
            .rounded_md()
            .flex()
            .flex_col()
            .gap_y(px(6.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .text_color(rgb(0x005c_6370))
                                    .child("LOGIC HEMISPHERE"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(0x0061_afef))
                                    .child(logic_model.map_or("Unknown", |m| m.label)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .text_color(rgb(0x005c_6370))
                                    .child("VOICE HEMISPHERE"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(0x0098_c379))
                                    .child(voice_model.map_or("Unknown", |m| m.label)),
                            ),
                    ),
            )
            .child(div().h_px().bg(rgb(0x003e_4451)).w_full())
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0x00ab_b2bf))
                    .child(logic_model.map_or("", |m| m.model_description)),
            )
            .child(
                div()
                    .text_size(px(9.0))
                    .italic()
                    .text_color(rgb(0x005c_6370))
                    .child(logic_model.map_or("", |m| m.license_constraints)),
            )
    }

    fn render_context_selector(&self, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let cx = &mut *cx;
        let logic_model = self.available_models.iter().find(|m| {
            m.tier == self.selected_tier && m.role == aemacs_ai::models::ModelRole::Logic
        });
        let current_context = self.selected_context;

        div()
            .flex()
            .flex_col()
            .gap_y(px(4.0))
            .child(
                div().text_size(px(10.0)).text_color(rgb(0x005c_6370)).child("MAX CONTEXT DEPTH"),
            )
            .child(
                div().flex().flex_wrap().gap(px(4.0)).children(
                    aemacs_ai::models::CONTEXT_OPTIONS
                        .iter()
                        .filter(|&&opt| logic_model.is_some_and(|m| opt <= m.max_context))
                        .map(|&opt| {
                            let is_selected = opt == current_context;
                            div()
                                .id(("context", opt))
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded_md()
                                .border_1()
                                .border_color(if is_selected {
                                    rgb(0x00bd_93f9)
                                } else {
                                    rgb(0x003e_4451)
                                })
                                .bg(if is_selected { rgb(0x0028_2c34) } else { rgb(0x0021_252b) })
                                .text_color(if is_selected {
                                    rgb(0x00ff_ffff)
                                } else {
                                    rgb(0x00ab_b2bf)
                                })
                                .text_size(px(11.0))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    this.selected_context = opt;
                                    this.conversation.set_context_window(opt);
                                    cx.notify();
                                }))
                                .child(if opt >= 1024 {
                                    format!("{}k", opt / 1024)
                                } else {
                                    opt.to_string()
                                })
                        }),
                ),
            )
    }

    fn render_vram_estimate(&self) -> impl IntoElement {
        let tier = self.selected_tier;
        let logic_model = self
            .available_models
            .iter()
            .find(|m| m.tier == tier && m.role == aemacs_ai::models::ModelRole::Logic);
        let voice_model = self
            .available_models
            .iter()
            .find(|m| m.tier == tier && m.role == aemacs_ai::models::ModelRole::Roleplay);

        let context_k = f64::from(self.selected_context) / 1024.0;
        let logic_est = logic_model.map_or(0.0, |m| {
            context_k.mul_add(f64::from(m.kv_rate_gb_per_1k), f64::from(m.base_vram_gb))
        });
        let voice_est = voice_model.map_or(0.0, |m| {
            context_k.mul_add(f64::from(m.kv_rate_gb_per_1k), f64::from(m.base_vram_gb))
        });

        let total_est = if logic_est > voice_est { logic_est } else { voice_est }; // Max because we unload Logic before loading Voice

        div()
            .text_size(px(10.0))
            .text_color(rgb(0x00bd_93f9))
            .italic()
            .child(format!("Sequential VRAM Peak: {total_est:.2} GB"))
    }

    fn render_cognition_pulse(&self, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let status = &self.status;
        let is_active = !matches!(status, CognitiveStatus::Idle);

        let (color, shimmer_width, intensity) = match status {
            CognitiveStatus::Idle => {
                (gpui::rgba(0x1c31_5eff), Default::default(), Default::default())
            }, // Dormant cobalt
            CognitiveStatus::Thinking => (gpui::rgba(0xffbf_00ff), 0.4, 0.8), // Radiating amber
            CognitiveStatus::Streaming => (gpui::rgba(0x00ff_7fff), 0.8, 1.0), // Vibrant emerald
            CognitiveStatus::Errored(_) => (gpui::rgba(0xdc14_3cff), 0.2, 1.0), // Jagged crimson
        };

        // Simulated Sine-wave pulsing for opacity (intensity)
        let pulse_opacity =
            (self.shimmer_offset * std::f32::consts::PI * 2.0).sin().mul_add(0.3, 0.7);
        let final_opacity = intensity * pulse_opacity;

        div()
            .h(px(8.0)) // The Height of Sanctity
            .w_full()
            .when(is_active, |this| {
                this.bg(gpui::rgba(0x181a_1fff)).child(
                    div().size_full().bg(color).opacity(final_opacity).relative().child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(relative(self.shimmer_offset))
                            .w(relative(shimmer_width))
                            .bg(gpui::rgba(0xffff_ffff))
                            .opacity(0.4), // The "Heat" core
                    ),
                )
            })
            .when(!is_active, |this| this.bg(gpui::transparent_black()))
    }

    fn sync_input_list_state(&mut self, cx: &Context<'_, Self>) {
        let editor = self.input_editor.read(cx);
        let line_count = editor.line_count();
        let (line, _col) = editor.cursor_position();
        let cursor_line = line.saturating_sub(1);
        let current_count = self.input_list_state.item_count();

        if current_count == line_count {
            if cursor_line == self.last_cursor_line {
                self.input_list_state.splice(cursor_line..cursor_line + 1, 1);
            } else {
                let min_line = std::cmp::min(cursor_line, self.last_cursor_line);
                let max_line = std::cmp::max(cursor_line, self.last_cursor_line);
                if min_line != max_line {
                    self.input_list_state.splice(min_line..min_line + 1, 1);
                    self.input_list_state.splice(max_line..max_line + 1, 1);
                }
            }
        } else {
            self.input_list_state.splice(0..current_count, line_count);
        }

        self.last_cursor_line = cursor_line;
    }

    fn render_header(&self, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex_shrink_0()
            .w_full()
            .p(px(10.0))
            .border_b_1()
            .border_color(rgb(0x0018_1a1f))
            .flex()
            .flex_col()
            .gap_y(px(8.0))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x00ff_ffff))
                            .child("Æmacs Mesh"),
                    )
                    .child(
                        div()
                            .id("maximize_btn")
                            .cursor_pointer()
                            .text_color(rgb(0x00ab_b2bf))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.is_maximized = !this.is_maximized;
                                cx.notify();
                            }))
                            .child(if self.is_maximized { "[ _ ]" } else { "[ + ]" }),
                    ),
            )
            .child(self.render_agent_selector(cx))
            .child(self.render_tier_selector(cx))
            .child(self.render_context_selector(cx))
            .child(self.render_vram_estimate())
    }

    fn render_task_board(&self) -> impl IntoElement {
        div()
            .flex_shrink_0()
            .w_full()
            .max_h(px(400.0))
            .border_b_1()
            .border_color(rgb(0x0018_1a1f))
            .p(px(10.0))
            .flex()
            .flex_col()
            .gap_y(px(4.0))
            .id("task_board")
            .overflow_y_scroll()
            .children(self.tasks.iter().map(|task| {
                let icon = match task.status {
                    aemacs_core::task::TaskStatus::Pending => "[ ]",
                    aemacs_core::task::TaskStatus::InProgress => "[>]",
                    aemacs_core::task::TaskStatus::Completed => "[x]",
                    aemacs_core::task::TaskStatus::Failed => "[!]",
                };
                let color = match task.status {
                    aemacs_core::task::TaskStatus::Pending => rgb(0x005c_6370),
                    aemacs_core::task::TaskStatus::InProgress => rgb(0x0061_afef),
                    aemacs_core::task::TaskStatus::Completed => rgb(0x0098_c379),
                    aemacs_core::task::TaskStatus::Failed => rgb(0x00e0_6c75),
                };
                div()
                    .flex()
                    .gap_x(px(8.0))
                    .text_size(px(12.0))
                    .text_color(color)
                    .child(div().child(icon))
                    .child(div().child(task.description.clone()))
            }))
    }

    fn render_message_area(&self) -> impl IntoElement {
        div()
            .w_full()
            .flex_1()
            .min_h_0()
            .flex_col()
            .gap_y(px(10.0))
            .p(px(10.0))
            .id("message_area")
            .overflow_y_scroll()
            .track_scroll(&self.message_scroll_handle)
            .children(self.messages.iter().map(Self::render_message))
    }

    fn render_message(msg: &ChatMessage) -> gpui::AnyElement {
        let is_user = msg.role == "User";
        let is_tool = msg.role == "Tool";
        let is_error = msg.content.contains("🛠️ TOOL_ERROR");

        if is_tool {
            return div()
                .flex()
                .flex_row()
                .items_center()
                .gap_x(px(8.0))
                .px(px(8.0))
                .py(px(4.0))
                .h(px(24.0))
                .rounded_full()
                .border_1()
                .border_color(if is_error { rgb(0x00e0_6c75) } else { rgb(0x0061_afef) })
                .bg(rgb(0x001e_1e1e))
                .child(div().text_size(px(12.0)).child("🛠️"))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(if is_error { rgb(0x00e0_6c75) } else { rgb(0x00ab_b2bf) })
                        .child("Specialist action performed"),
                )
                .into_any_element();
        }

        div()
            .w_full()
            .flex()
            .flex_col()
            .child(div().text_size(px(10.0)).text_color(rgb(0x005c_6370)).child(msg.role.clone()))
            .child(
                div()
                    .w_full()
                    .p(px(8.0))
                    .rounded_md()
                    .bg(if is_user { rgb(0x003e_4451) } else { rgb(0x0028_2c34) })
                    .text_color(if is_error { rgb(0x00e0_6c75) } else { rgb(0x00ab_b2bf) })
                    .flex()
                    .flex_col()
                    .gap_y(px(4.0))
                    .children(msg.parsed_blocks.iter().map(Self::render_markdown_block)),
            )
            .into_any_element()
    }

    fn render_markdown_block(block: &MarkdownBlock) -> gpui::AnyElement {
        match block {
            MarkdownBlock::Paragraph(text) => div().w_full().child(text.clone()).into_any_element(),
            MarkdownBlock::Code { language, content } => div()
                .w_full()
                .bg(rgb(0x001e_1e1e))
                .p(px(6.0))
                .rounded_sm()
                .border_1()
                .border_color(rgb(0x003e_4451))
                .overflow_x_hidden()
                .child(
                    div().text_color(rgb(0x0061_afef)).text_size(px(10.0)).child(language.clone()),
                )
                .child(
                    div()
                        .text_color(rgb(0x00ab_b2bf))
                        .font_family("monospace")
                        .child(content.clone()),
                )
                .into_any_element(),
            MarkdownBlock::Header { level, content } => {
                let size = match level {
                    1 => 18.0,
                    2 => 16.0,
                    3 => 14.0,
                    _ => 12.0,
                };
                div()
                    .w_full()
                    .text_size(px(size))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(content.clone())
                    .into_any_element()
            },
        }
    }

    fn render_progress_line(&self) -> impl IntoElement {
        div().px(px(10.0)).h(px(16.0)).flex().items_center().child(
            self.current_action.as_ref().map_or_else(div, |action| {
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0x0061_afef))
                    .italic()
                    .child(action.clone())
            }),
        )
    }

    fn handle_input_keydown(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let keystroke = &event.keystroke;
        let mode = self.input_editor.read(cx).mode;

        if keystroke.key == "enter" {
            if mode == Mode::Normal || !keystroke.modifiers.shift {
                self.send_message(cx);
                return;
            }
            self.input_editor.update(cx, |editor, _| {
                let _ = editor.run(Command::InsertNewline);
            });
            cx.notify();
            return;
        }

        if let Some(cmd) = resolve_key_command(keystroke, mode) {
            self.input_editor.update(cx, |editor, _cx| {
                let _ = editor.run(cmd);
            });
            cx.notify();
        }
    }

    fn send_message(&mut self, cx: &mut Context<'_, Self>) {
        let raw_text = self.input_editor.read(cx).buffer.text();
        if raw_text.trim().is_empty() {
            return;
        }

        let mut text = raw_text;
        let mut switched = false;

        if !self.handle_statute_tags(&mut text, cx)
            || !self.handle_material_tags(&mut text, cx)
            || !self.handle_slash_command(&mut text, &mut switched, cx)
            || !self.push_outgoing_message(&text, switched)
        {
            return;
        }

        self.clear_input(cx);

        if text.trim().is_empty() {
            cx.notify();
            return;
        }

        self.sync_active_persona();
        self.conversation.add_message(aemacs_ai::Message::user(text));

        self.message_scroll_handle.set_offset(gpui::point(px(0.0), px(999_999.0)));
        cx.notify();
        self.trigger_ai_response(cx);
    }
}

impl Render for AiPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.sync_input_list_state(cx);

        let root = if self.is_maximized {
            div().flex().flex_col().w_full()
        } else {
            div().flex().flex_col().w(px(self.width))
        };

        root.size_full()
            .min_h_0()
            .bg(rgb(0x0021_252b))
            .overflow_hidden()
            .border_l_1()
            .border_color(rgb(0x0018_1a1f))
            .child(self.render_header(cx))
            .when(!self.tasks.is_empty(), |this| this.child(self.render_task_board()))
            .child(self.render_message_area())
            .child(self.render_cognition_pulse(cx))
            .child(self.render_progress_line())
            .child(
                div()
                    .id("input_area")
                    .flex_shrink_0()
                    .h(px(100.0))
                    .max_h(px(200.0))
                    .bg(rgb(0x0028_2c34))
                    .border_t_1()
                    .border_color(rgb(0x0018_1a1f))
                    .p(px(8.0))
                    .overflow_x_hidden()
                    .track_focus(&self.focus_handle)
                    .on_click(cx.listener(|this, _, window, cx| {
                        window.focus(&this.focus_handle, cx);
                    }))
                    .on_key_down(cx.listener(Self::handle_input_keydown))
                    .child({
                        let editor = self.input_editor.read(cx);
                        render_editor_view(editor, self.input_list_state.clone(), true)
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This quest verifies that our regex can distinguish between the Holy Statute (@)
    /// and the Raw Materials (@@). It ensures that '@@' is never accidentally
    /// identified as a single-@ statute.
    #[test]
    fn test_statute_differentiation_logic() -> anyhow::Result<()> {
        let statute_regex = statute_regex()?;
        let find_statute_indices = |text: &str| {
            statute_regex
                .find_iter(text)
                .filter(|m: &regex::Match<'_>| {
                    let start = m.start();
                    if start > 0 && text.as_bytes()[start - 1] == b'@' {
                        return false;
                    }
                    true
                })
                .map(|m: regex::Match<'_>| (m.start(), m.end()))
                .collect::<Vec<(usize, usize)>>()
        };

        // Case 1: Multiple statutes (The Law-Confusion Dragon)
        let text1 = "@file1.md @file2.md build the forge";
        assert_eq!(find_statute_indices(text1).len(), 2);

        // Case 2: Mixed materials and statutes
        let text2 = "@@src/main.rs @profile.md";
        let matches2 = find_statute_indices(text2);
        assert_eq!(matches2.len(), 1);
        assert_eq!(&text2[matches2[0].0..matches2[0].1], "@profile.md");

        // Case 3: Pure materials (Should be ignored by this lexer)
        let text3 = "@@src/main.rs @@src/lib.rs";
        assert_eq!(find_statute_indices(text3).len(), 0);
        Ok(())
    }

    /// This quest verifies that the Material Lexer (@@) can find multiple
    /// attachments within a single string of intent.
    #[test]
    fn test_material_collection_logic() -> anyhow::Result<()> {
        let material_regex = material_regex()?;
        let text = "Analyze @@src/main.rs and @@crates/core/lib.rs please.";
        let matches: Vec<_> = material_regex.find_iter(text).collect();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].as_str(), "@@src/main.rs");
        assert_eq!(matches[1].as_str(), "@@crates/core/lib.rs");
        Ok(())
    }

    /// The Ultimate Quest! This test verifies the full transformation of a complex
    /// mixed input. It ensures that statutes and materials are correctly extracted
    /// and that ALL tags are stripped from the final payload sent to the specialists.
    #[test]
    fn test_mixed_lexer_transformation() -> anyhow::Result<()> {
        let statute_regex = statute_regex()?;
        let material_regex = material_regex()?;

        let mut text = "@ai/profiles/rust.md @@src/rag.rs @@src/mcp.rs please analyze.".to_string();

        // 1. Extract Statute
        let statute_matches: Vec<_> = statute_regex
            .find_iter(&text)
            .filter(|m: &regex::Match<'_>| {
                let start = m.start();
                if start > 0 && text.as_bytes()[start - 1] == b'@' {
                    return false;
                }
                true
            })
            .collect();

        assert_eq!(statute_matches.len(), 1);

        // 2. Extract Materials
        let material_matches: Vec<_> = material_regex.find_iter(&text).collect();
        assert_eq!(material_matches.len(), 2);

        // 3. Execute Stripping (Simulating the reverse-iteration logic used in send_message)
        let mut all_tags: Vec<_> =
            statute_matches.iter().map(|m: &regex::Match<'_>| (m.start(), m.end())).collect();
        all_tags.extend(material_matches.iter().map(|m: &regex::Match<'_>| (m.start(), m.end())));
        all_tags.sort_by_key(|k| k.0);

        for (start, end) in all_tags.iter().rev() {
            text.replace_range(start..end, "");
        }

        let cleaned_text = text.trim();
        assert_eq!(
            cleaned_text, "please analyze.",
            "The final intent must be pure and free of metadata tags!"
        );
        Ok(())
    }

    #[test]
    fn test_visual_loom_pill_logic() {
        // Quest: Verify that the UI correctly identifies tool roles and errors

        // Case 1: Successful tool
        let msg_ok = ChatMessage::new("Tool", "Result of read_file...");
        let is_tool_ok = msg_ok.role == "Tool";
        let is_error_ok = msg_ok.content.contains("🛠️ TOOL_ERROR");

        assert!(is_tool_ok, "Should identify 'Tool' role.");
        assert!(!is_error_ok, "Should not identify error in successful result.");

        // Case 2: Failed tool
        let msg_err = ChatMessage::new("Tool", "🛠️ TOOL_ERROR: [File not found]");
        let is_tool_err = msg_err.role == "Tool";
        let is_error_err = msg_err.content.contains("🛠️ TOOL_ERROR");

        assert!(is_tool_err, "Should identify 'Tool' role.");
        assert!(is_error_err, "Should identify error when TOOL_ERROR prefix is present.");
    }
}
