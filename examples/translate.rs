use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    size,
};
use gpui_animation::{
    animation::TransitionExt,
    transition::{self},
};

struct TranslateDemo;

impl Render for TranslateDemo {
    fn render(&mut self, _window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let ease = std::sync::Arc::new(transition::general::EaseInOutCubic);

        div()
            .id("root")
            .flex()
            .flex_col()
            .gap_8()
            .size(px(500.0))
            .justify_center()
            .items_center()
            .bg(gpui::rgb(0x0a0a0a))
            // ── translate_x demo ──
            .child(
                div().flex().flex_col().items_center().gap_2().child(
                    div()
                        .id("translate_x")
                        .relative()
                        .size_12()
                        .rounded_md()
                        .bg(gpui::red())
                        .with_transition("translate_x")
                        .transition_on_hover(
                            std::time::Duration::from_millis(400),
                            ease.clone(),
                            |hovered, state| {
                                if *hovered {
                                    state.translate_x(px(80.0))
                                } else {
                                    state.translate_x(px(0.0))
                                }
                            },
                        ),
                ),
            )
            // ── translate_y demo ──
            .child(
                div().flex().flex_col().items_center().gap_2().child(
                    div()
                        .id("translate_y")
                        .relative()
                        .size_12()
                        .rounded_md()
                        .bg(gpui::blue())
                        .with_transition("translate_y")
                        .transition_on_hover(
                            std::time::Duration::from_millis(400),
                            ease.clone(),
                            |hovered, state| {
                                if *hovered {
                                    state.translate_y(px(60.0))
                                } else {
                                    state.translate_y(px(0.0))
                                }
                            },
                        ),
                ),
            )
            // ── translate (x + y) demo ──
            .child(
                div().flex().flex_col().items_center().gap_2().child(
                    div()
                        .id("translate_xy")
                        .relative()
                        .size_12()
                        .rounded_md()
                        .bg(gpui::green())
                        .with_transition("translate_xy")
                        .transition_on_hover(
                            std::time::Duration::from_millis(400),
                            ease,
                            |hovered, state| {
                                if *hovered {
                                    state.translate(px(60.0), px(40.0))
                                } else {
                                    state.translate(px(0.0), px(0.0))
                                }
                            },
                        ),
                ),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| TranslateDemo),
        )
        .unwrap();
    });
}
