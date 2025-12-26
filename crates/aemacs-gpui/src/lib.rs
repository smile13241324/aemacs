use aemacs_core::{Editor, runtime};
use anyhow::Result;
use log::info;

// 1. Imports aufräumen
// Wir brauchen Entity! AppContext ist raus (wir nutzen App).
use gpui::prelude::*;
use gpui::{
    App,
    Application,
    Bounds,
    Context,
    Entity, // <--- NEU: Der Ersatz für Model/View
    IntoElement,
    Window,
    WindowBounds,
    WindowOptions,
    div,
    px,
    rgb,
    size,
};

pub fn init() -> Result<()> {
    info!("🎨 [GPUI] Initializing Graphics Engine...");
    Ok(())
}

pub struct Workspace {
    // 2. State-Haltung
    // Statt Model<Editor> nutzen wir jetzt Entity<Editor>.
    // Ein Entity ist ein Handle auf ein Objekt, das GPUI verwaltet.
    editor: Entity<Editor>,
}

impl Workspace {
    // 3. Builder Update
    // Statt 'WindowContext' nutzen wir '&mut App' (oder Context).
    // Der Return-Type ist jetzt Entity<Self> (statt View<Self>).
    pub fn build(cx: &mut App) -> Entity<Self> {
        // cx.new() ist der neue Universal-Konstruktor für alles (Views & Models)
        cx.new(|cx| {
            // Auch den Editor erstellen wir mit cx.new()
            let editor = cx.new(|_cx| Editor::new());

            Workspace { editor }
        })
    }
}

impl Render for Workspace {
    // Signatur ist korrekt (Window + Context)
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .bg(rgb(0x282c34))
            .size_full()
            .justify_center()
            .items_center()
            .text_xl()
            .text_color(rgb(0xabb2bf))
            .child("Welcome to Æmacs - The Iron Core 🦀")
    }
}

pub fn run_app() {
    Application::new().run(|cx: &mut App| {
        // Runtime starten
        runtime::init(cx);
        info!("🚀 [RUNTIME] Tokio Bridge active.");

        let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                // Hier übergeben wir 'cx' (welches &mut App ist) an build
                Workspace::build(cx)
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
