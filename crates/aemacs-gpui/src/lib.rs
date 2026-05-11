use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use aemacs_core::{Editor, command::Command, mode::Mode, runtime};
use anyhow::Result;
use gpui::{
    App, Application, Bounds, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use log::info;

pub mod ai_panel;
use ai_panel::AiPanel;

pub mod editor_view;
use editor_view::render_editor_view;

pub mod input_handler;
use input_handler::resolve_key_command;

pub mod ai_utils;

/// Initializes the GPUI subsystem for Æmacs.
///
/// # Errors
/// Returns an error if graphics initialization fails.
pub fn init() -> Result<()> {
    info!("🎨 [GPUI] Initializing Graphics Engine...");
    Ok(())
}

use std::sync::Arc;

use aemacs_ai::{PersonaRegistry, mcp::ToolRegistry, rag::KnowledgeBase};

/// The primary visual container for the editor.
/// It manages the coordination between the text editor, the AI side panel,
/// and the global system event bus.
pub struct Workspace {
    /// The main text editor engine.
    editor: Entity<Editor>,
    /// State for the high-performance editor list view.
    editor_list_state: gpui::ListState,
    /// State for the line number gutter.
    gutter_list_state: gpui::ListState,
    /// The integrated AI assistant panel.
    ai_panel: Entity<AiPanel>,
    /// Manages keyboard focus within the workspace.
    focus_handle: FocusHandle,
    /// Tracks the last key pressed for chord detection (e.g., 'fd' to escape).
    last_key: Option<(String, Instant)>,
    /// Controls the visibility of the AI panel.
    show_ai: bool,
    /// Stores the active system notification message.
    notification: Option<String>,
    /// The active project plan/roadmap.
    tasks: Vec<aemacs_core::task::Task>,
    /// Handle to the current window.
    window_handle: gpui::AnyWindowHandle,
    /// Reference to the RAG memory system.
    pub kb: Arc<KnowledgeBase>,
    /// Reference to the global tool registry.
    pub registry: Arc<ToolRegistry>,
    /// Reference to the agent persona registry.
    pub persona_registry: Arc<PersonaRegistry>,
    /// Tracks the previous line position of the cursor for optimized rendering.
    last_cursor_line: usize,
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("show_ai", &self.show_ai)
            .field("tasks", &self.tasks)
            .field("notification", &self.notification)
            .field("last_cursor_line", &self.last_cursor_line)
            .finish_non_exhaustive()
    }
}

impl Workspace {
    /// Orchestrates the construction of a new Workspace.
    /// It initializes the AI mesh, loads agent personas, and configures the editor buffer.
    pub fn build(
        cx: &mut App,
        file_path: Option<PathBuf>,
        window_handle: gpui::AnyWindowHandle,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let config = aemacs_core::config::get_config();
            let (kb, persona_registry, bus, registry) = Self::initialize_ai_infrastructure(
                cx,
                config.qdrant_url.as_deref(),
                config.ollama_url.as_deref(),
            );
            let editor = Self::build_editor(cx, file_path);
            let (ai_panel, host_rx) = Self::build_ai_panel(
                cx,
                window_handle,
                config.hardware_tier.as_deref(),
                kb.clone(),
                registry.clone(),
                persona_registry.clone(),
                bus.clone(),
            );
            let editor_list_state = gpui::ListState::new(0, gpui::ListAlignment::Top, px(100.0));
            let gutter_list_state = gpui::ListState::new(0, gpui::ListAlignment::Top, px(100.0));
            let focus_handle = cx.focus_handle();

            Self::spawn_host_request_listener(cx, host_rx);
            Self::spawn_workspace_bus_listener(cx, &bus);

            Self {
                editor,
                editor_list_state,
                gutter_list_state,
                ai_panel,
                focus_handle,
                last_key: None,
                show_ai: true, // Default to visible for testing
                notification: None,
                tasks: Vec::new(),
                window_handle,
                kb,
                registry,
                persona_registry,
                last_cursor_line: 0,
            }
        })
    }

    fn initialize_ai_infrastructure(
        cx: &Context<'_, Self>,
        qdrant_url: Option<&str>,
        ollama_url: Option<&str>,
    ) -> (Arc<KnowledgeBase>, Arc<PersonaRegistry>, aemacs_core::bus::EventBus, Arc<ToolRegistry>)
    {
        let kb = Arc::new(
            futures::executor::block_on(KnowledgeBase::new(
                qdrant_url.unwrap_or("http://localhost:6334"),
                ollama_url.unwrap_or("http://localhost:11434"),
                aemacs_ai::rag::Environment::Production,
            ))
            .unwrap_or_else(|e| {
                log::error!("Failed to initialize KnowledgeBase: {e}");
                std::process::exit(1);
            }),
        );

        let persona_registry = futures::executor::block_on(PersonaRegistry::new(
            aemacs_core::runtime::Tokio::handle(cx),
        ))
        .unwrap_or_else(|e| {
            log::error!("Failed to initialize PersonaRegistry: {e}");
            std::process::exit(1);
        });
        persona_registry.clone().start_watching().ok();

        let bus = cx.global::<aemacs_core::bus::EventBus>().clone();
        let registry = Arc::new(ToolRegistry::with_core_tools(
            kb.clone(),
            persona_registry.clone(),
            Some(bus.tx.clone()),
        ));

        (kb, persona_registry, bus, registry)
    }

    fn build_editor(cx: &mut Context<'_, Self>, file_path: Option<PathBuf>) -> Entity<Editor> {
        cx.new(|_cx| {
            file_path.map_or_else(Editor::new, |path| match Editor::from_file(path) {
                Ok(ed) => {
                    log::info!("📂 [Workspace] File loaded successfully.");
                    ed
                },
                Err(e) => {
                    log::error!("⚠️ [Workspace] Failed to load file: {e}");
                    Editor::new()
                },
            })
        })
    }

    fn build_ai_panel(
        cx: &mut Context<'_, Self>,
        window_handle: gpui::AnyWindowHandle,
        hardware_tier: Option<&str>,
        kb: Arc<KnowledgeBase>,
        registry: Arc<ToolRegistry>,
        persona_registry: Arc<PersonaRegistry>,
        bus: aemacs_core::bus::EventBus,
    ) -> (Entity<AiPanel>, async_channel::Receiver<ai_panel::HostRequest>) {
        let tier_str = hardware_tier.unwrap_or("LOW");
        let available_models = aemacs_ai::models::get_models_for_tier(tier_str);
        log::info!(
            "🚀 [Workspace] Hardware Tier: {} ({} models loaded)",
            tier_str,
            available_models.len()
        );

        let (host_tx, host_rx) = async_channel::unbounded::<ai_panel::HostRequest>();
        let ai_panel = AiPanel::new(
            cx,
            ai_panel::AiPanelInit {
                window_handle,
                host_tx,
                kb,
                registry,
                persona_registry,
                bus,
                available_models,
            },
        );

        (ai_panel, host_rx)
    }

    fn spawn_host_request_listener(
        cx: &Context<'_, Self>,
        host_rx: async_channel::Receiver<ai_panel::HostRequest>,
    ) {
        cx.spawn(|workspace: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
            let mut cx = cx.clone();
            async move {
                while let Ok(request) = host_rx.recv().await {
                    let _ = workspace.update(&mut cx, |this, cx| {
                        Self::handle_host_request(this, request, cx);
                    });
                }
            }
        })
        .detach();
    }

    fn handle_host_request(
        this: &Self,
        request: ai_panel::HostRequest,
        cx: &mut Context<'_, Self>,
    ) {
        let window_handle = this.window_handle;
        let _ = cx.update_window(window_handle, |_, window, cx| match request {
            ai_panel::HostRequest::Approval { description, responder } => {
                let future = window.prompt(
                    gpui::PromptLevel::Warning,
                    "AI Permission Request",
                    Some(&description),
                    &["Approve", "Deny"],
                    cx,
                );
                cx.background_executor()
                    .spawn(async move {
                        let answer = future.await.unwrap_or(1usize);
                        let _ = responder.send(answer == 0);
                    })
                    .detach();
            },
            ai_panel::HostRequest::UserPrompt { question, responder } => {
                let future = window.prompt(
                    gpui::PromptLevel::Info,
                    "AI Question",
                    Some(&question),
                    &["Acknowledge"],
                    cx,
                );
                cx.background_executor()
                    .spawn(async move {
                        let _ = future.await;
                        let _ = responder.send("Acknowledged".to_string());
                    })
                    .detach();
            },
        });
    }

    fn spawn_workspace_bus_listener(cx: &Context<'_, Self>, bus: &aemacs_core::bus::EventBus) {
        let mut rx = bus.subscribe();
        cx.spawn(|workspace: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
            let mut cx = cx.clone();
            async move {
                while let Ok(event) = rx.recv().await {
                    let _ = workspace.update(&mut cx, |this, cx| {
                        this.handle_system_event(event, cx);
                    });
                }
            }
        })
        .detach();
    }

    fn handle_system_event(
        &mut self,
        event: aemacs_core::bus::SystemEvent,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            aemacs_core::bus::SystemEvent::FileModified(modified_path) => {
                let editor_path = self.editor.read(cx).buffer.path.clone();
                if let Some(current_path) = editor_path
                    && current_path == modified_path
                {
                    log::info!("🔄 [Workspace] Auto-reloading buffer: {}", current_path.display());
                    let _ = self.editor.update(cx, |ed, _| ed.reload());
                    cx.notify();
                }
            },
            aemacs_core::bus::SystemEvent::Notification(msg) => {
                log::info!("🔔 [Workspace] Notification: {msg}");
                self.notification = Some(msg);
                cx.notify();
            },
            aemacs_core::bus::SystemEvent::OpenFile(path) => {
                match aemacs_core::Editor::from_file(path.clone()) {
                    Ok(new_editor) => {
                        log::info!("📂 [Workspace] Switching to file: {}", path.display());
                        self.editor.update(cx, |ed, _| *ed = new_editor);
                        self.notification = Some(format!("Opened: {}", path.display()));
                    },
                    Err(e) => {
                        log::error!("⚠️ [Workspace] Failed to open file: {e}");
                        self.notification = Some(format!("Error opening file: {e}"));
                    },
                }
                cx.notify();
            },
            aemacs_core::bus::SystemEvent::PlanCreated(tasks) => {
                log::info!("📋 [Workspace] Plan created with {} tasks.", tasks.len());
                self.tasks = tasks
                    .into_iter()
                    .map(|desc| aemacs_core::task::Task {
                        description: desc,
                        status: aemacs_core::task::TaskStatus::Pending,
                    })
                    .collect();
                self.sync_tasks_to_panel(cx);
                cx.notify();
            },
            aemacs_core::bus::SystemEvent::TaskUpdated { index, status } => {
                if let Some(task) = self.tasks.get_mut(index) {
                    log::info!("✅ [Workspace] Task {index} updated to {status:?}.");
                    task.status = status;
                    self.sync_tasks_to_panel(cx);
                    cx.notify();
                }
            },
            aemacs_core::bus::SystemEvent::PersonaChanged { name, message } => {
                log::info!("🔄 [Workspace] Programmatic persona switch: {name}");
                self.ai_panel.update(cx, |panel, cx| {
                    panel.handoff_persona(&name, message, cx);
                });
            },
            aemacs_core::bus::SystemEvent::Signal { .. } => {},
        }
    }

    fn sync_tasks_to_panel(&self, cx: &mut Context<'_, Self>) {
        let tasks = self.tasks.clone();
        self.ai_panel.update(cx, |panel, cx| {
            panel.update_tasks(tasks, cx);
        });
    }

    fn render_welcome() -> impl IntoElement {
        let bg_color = rgb(0x0028_2c34);
        let accent_color = rgb(0x00bd_93f9);
        let key_hint_bg = rgb(0x003e_4451);

        div()
            .flex()
            .flex_col()
            .size_full()
            .justify_center()
            .items_center()
            .bg(bg_color)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_y(px(20.0))
                    .child(div().text_size(px(64.0)).child("🦀"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_x(px(10.0))
                            .child(
                                div()
                                    .text_size(px(48.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x00e0_e0e0))
                                    .child("Æmacs"),
                            )
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded_md()
                                    .bg(accent_color)
                                    .text_color(rgb(0x0028_2c34))
                                    .text_size(px(12.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("PRE-ALPHA"),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(px(40.0))
                    .flex()
                    .gap_x(px(20.0))
                    .text_sm()
                    .text_color(rgb(0x00ab_b2bf))
                    .child(Self::render_key_hint("Type", "Insert Text", key_hint_bg.into()))
                    .child(Self::render_key_hint("i", "Insert Mode", key_hint_bg.into()))
                    .child(Self::render_key_hint("ESC", "Normal Mode", key_hint_bg.into())),
            )
    }

    fn render_key_hint(key: &str, desc: &str, bg: gpui::Hsla) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_x(px(8.0))
            .child(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .bg(bg)
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x001e_222a))
                    .font_family("Fira Code")
                    .child(key.to_string()),
            )
            .child(desc.to_string())
    }

    fn sync_editor_list_state(&mut self, line_count: usize, cursor_line: usize) {
        let current_count = self.editor_list_state.item_count();

        if current_count == line_count {
            if cursor_line == self.last_cursor_line {
                self.editor_list_state.splice(cursor_line..cursor_line + 1, 1);
                self.gutter_list_state.splice(cursor_line..cursor_line + 1, 1);
            } else {
                let min_line = std::cmp::min(cursor_line, self.last_cursor_line);
                let max_line = std::cmp::max(cursor_line, self.last_cursor_line);
                if min_line != max_line {
                    self.editor_list_state.splice(min_line..min_line + 1, 1);
                    self.editor_list_state.splice(max_line..max_line + 1, 1);
                    self.gutter_list_state.splice(min_line..min_line + 1, 1);
                    self.gutter_list_state.splice(max_line..max_line + 1, 1);
                }
            }
        } else {
            self.editor_list_state.splice(0..current_count, line_count);
            self.gutter_list_state.splice(0..current_count, line_count);
        }

        self.last_cursor_line = cursor_line;
    }

    fn render_main_view(&self, cx: &Context<'_, Self>, is_empty: bool) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let focus_listener = cx.listener(|this, _, window, cx| {
            window.focus(&this.focus_handle, cx);
        });
        let key_listener = cx.listener(Self::handle_keydown);
        div()
            .id("main_editor_area")
            .flex()
            .flex_1()
            .flex_row()
            .track_focus(&self.focus_handle)
            .on_click(focus_listener)
            .on_key_down(key_listener)
            .child(
                div()
                    .w(px(50.0))
                    .bg(rgb(0x0021_252b))
                    .flex()
                    .flex_col()
                    .items_end()
                    .pr(px(8.0))
                    .pt(px(16.0))
                    .text_size(px(14.0))
                    .font_family("Fira Code")
                    .text_color(rgb(0x0049_5162))
                    .child(
                        gpui::list(
                            self.gutter_list_state.clone(),
                            move |line_idx, _window, _cx| {
                                if is_empty {
                                    div().child("~").h(px(20.0)).into_any_element()
                                } else {
                                    div()
                                        .child((line_idx + 1).to_string())
                                        .h(px(20.0))
                                        .into_any_element()
                                }
                            },
                        )
                        .w_full()
                        .h_full(),
                    ),
            )
            .child(if is_empty {
                Self::render_welcome().into_any_element()
            } else {
                render_editor_view(editor, self.editor_list_state.clone(), false).into_any_element()
            })
    }

    fn render_status_bar(
        &self,
        mode_name: String,
        mode_color: gpui::Rgba,
        line: usize,
        col: usize,
    ) -> impl IntoElement {
        div()
            .h(px(30.0))
            .bg(rgb(0x0021_252b))
            .border_t_1()
            .border_color(rgb(0x0018_1a1f))
            .flex()
            .items_center()
            .px(px(10.0))
            .gap_x(px(10.0))
            .text_size(px(12.0))
            .font_family("Fira Code")
            .child(
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded_sm()
                    .bg(mode_color)
                    .text_color(rgb(0x0028_2c34))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(mode_name),
            )
            .child(div().text_color(rgb(0x009d_a5b4)).child("buffer-1.rs"))
            .when_some(self.notification.clone(), |this, msg| {
                this.child(div().px(px(8.0)).text_color(rgb(0x00bd_93f9)).italic().child(msg))
            })
            .child(div().flex_1())
            .child(div().text_color(rgb(0x00ff_5555)).child(if self.show_ai {
                "AI: ON"
            } else {
                "AI: OFF"
            }))
            .child(div().text_color(rgb(0x009d_a5b4)).child(format!("Ln {line}, Col {col}")))
            .child(div().text_color(rgb(0x0049_5162)).child("UTF-8"))
    }

    fn handle_keydown(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let keystroke = &event.keystroke;
        let current_time = Instant::now();

        if self.editor.read(cx).mode == Mode::Insert
            && let Some((last_char, last_time)) = &self.last_key
            && last_char == "f"
            && keystroke.key == "d"
            && current_time.duration_since(*last_time) < Duration::from_millis(250)
        {
            self.editor.update(cx, |editor, _| editor.backspace());

            self.editor.update(cx, |editor, _| {
                if let Err(e) = editor.run(Command::EnterMode(Mode::Normal)) {
                    log::error!("Failed to switch mode: {e}");
                }
            });

            self.last_key = None;
            cx.notify();
            return;
        }

        let current_mode = self.editor.read(cx).mode;

        // 1. Special Handling: Enter -> Newline
        let command = if keystroke.key == "enter" {
            Some(Command::InsertNewline)
        } else {
            // 2. Delegate to shared logic
            resolve_key_command(keystroke, current_mode)
        };

        if let Some(text) = &keystroke.key_char {
            // Update last key for special combos (like fd)
            // Only if it was a text input
            self.last_key = Some((text.clone(), current_time));
        }

        if let Some(cmd) = command {
            self.editor.update(cx, |editor, _cx| {
                if let Err(e) = editor.run(cmd) {
                    log::error!("Editor command failed: {e}");
                }
            });
            cx.notify();
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let (line, col, line_count, mode, is_empty) = {
            let editor = self.editor.read(cx);
            (
                editor.cursor_position().0,
                editor.cursor_position().1,
                editor.line_count(),
                editor.mode,
                editor.buffer.len_chars() == 0,
            )
        };
        let cursor_line = line.saturating_sub(1);
        self.sync_editor_list_state(line_count, cursor_line);
        let mode_name = format!("{mode:?}").to_uppercase();
        let mode_color = match mode {
            Mode::Normal => rgb(0x00d1_9a66),
            Mode::Insert => rgb(0x0098_c379),
            Mode::Visual => rgb(0x003e_4451),
        };
        let ai_maximized = self.ai_panel.read(cx).is_maximized;
        let main_view = self.render_main_view(cx, is_empty);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x0028_2c34))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .flex_row()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .when(self.show_ai && ai_maximized, gpui::Styled::hidden)
                            .child(main_view),
                    )
                    .when(self.show_ai, |this| {
                        this.child(
                            div()
                                .h_full() // Force strict height boundary
                                .min_h_0() // Allow shrinking below content size
                                .when(!ai_maximized, |this| this.w(px(350.0)))
                                .when(ai_maximized, gpui::Styled::flex_1)
                                .overflow_hidden()
                                .border_l_1()
                                .border_color(rgb(0x0018_1a1f))
                                .child(self.ai_panel.clone()),
                        )
                    }),
            )
            .child(self.render_status_bar(mode_name, mode_color, line, col))
    }
}

/// The main entry point for the Æmacs graphical application.
/// It initializes the asynchronous runtime, boots the core systems, and starts the GPUI event loop.
pub fn run_app(file_to_open: Option<PathBuf>) {
    let runtime =
        match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                log::error!("Failed to initialize Tokio runtime: {e}");
                std::process::exit(1);
            },
        };

    let handle = runtime.handle().clone();

    // Perform System Boot Sequence within the owned runtime
    if let Err(e) = runtime.block_on(async {
        // A. Core System (Configs, Global State, Buffer Manager)
        aemacs_core::init()?;

        // B. Legacy Bridge (Python environment must be ready before loading plugins)
        aemacs_bridge::init()?;

        // C. LSP Subsystem (Language Servers can start in background)
        aemacs_lsp::init()?;

        // D. UI Preparation (Load assets, cache fonts, compile shaders)
        init()?;

        Ok::<(), anyhow::Error>(())
    }) {
        log::error!("💥 [APP] Critical System Failure during boot: {e}");
        std::process::exit(1);
    }

    info!("✨ [APP] System fully operational. Handing over main thread to GPU Interface.");

    Application::new().run(move |cx: &mut App| {
        runtime::init_from_handle(cx, handle);
        info!("🚀 [RUNTIME] Tokio Bridge active.");

        // Global Event Bus (ACO-031 Phase 2)
        let bus = aemacs_core::bus::EventBus::new();
        cx.set_global(bus);

        let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Æmacs".into()),
                appears_transparent: true,
                traffic_light_position: Some(gpui::Point::new(px(8.0), px(8.0))),
            }),
            ..Default::default()
        };

        match cx.open_window(options, |window, cx| {
            let view = Workspace::build(cx, file_to_open, window.window_handle());
            let focus_handle = view.read(cx).focus_handle.clone();
            window.focus(&focus_handle, cx);
            view
        }) {
            Ok(_) => {},
            Err(e) => {
                log::error!("Failed to open window: {e:?}");
                std::process::exit(1);
            },
        }

        cx.activate(true);
    });
}
