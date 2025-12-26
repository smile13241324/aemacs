use aemacs_core::Editor;
use anyhow::Result;
use log::info;

// 1. Die harte Liste der Wahrheit
// Wir importieren jeden einzelnen Typ, den wir brauchen.
// Verlass dich nicht auf 'prelude'.
use gpui::{
    AppContext, // <--- Wir holen ihn zurück! Er existiert.
    // Der Runner & Context
    Application,
    Bounds,
    Context,

    IntoElement,
    // Die Core-Typen (die zuletzt gefehlt haben)
    Model,
    Point,
    // Traits
    Render,
    Size,
    View,
    ViewContext,

    VisualContext,

    Window,
    WindowContext,
    // Geometrie
    WindowOptions,
    // Die Basics
    div,
    rgb,
};

// Prelude nur für Methoden-Erweiterungen (wie .flex(), .bg())
use gpui::prelude::*;

pub fn init() -> Result<()> {
    info!("🎨 [GPUI] Initializing Graphics Engine (Git Master)...");
    Ok(())
}

pub struct Workspace {
    editor: Model<Editor>,
}

impl Workspace {
    pub fn build(cx: &mut WindowContext) -> View<Self> {
        cx.new_view(|cx| {
            let editor = cx.new_model(|_cx| Editor::new());
            Workspace { editor }
        })
    }
}

impl Render for Workspace {
    // (Self, Window, Context<Self>)
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
    let app = Application::new();

    // Wir nutzen wieder AppContext.
    // Falls AppContext doch fehlt (unwahrscheinlich), wäre 'Context<()>' die Alternative.
    app.run(|cx: &mut AppContext| {
        let options = WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(Bounds::new(
                Point::new(100.into(), 100.into()),
                Size::new(800.into(), 600.into()),
            ))),
            ..Default::default()
        };

        cx.open_window(options, |cx| Workspace::build(cx));
    });
}
