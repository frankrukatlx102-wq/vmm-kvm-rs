#![allow(dead_code)]

mod backend;
mod hostinfo;
mod osdata;

use gpui::{
    App, Application, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};

struct VmmApp {
    line: String,
}

impl Render for VmmApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x0a141f))
            .text_color(rgb(0xdbe7f1))
            .flex()
            .items_center()
            .justify_center()
            .text_xl()
            .child(self.line.clone())
    }
}

fn main() {
    let (ram_mb, cpus) = hostinfo::host_limits();
    let line = format!("VMM-KVM (Rust)  |  gpui skeleton  |  host: {ram_mb} MB RAM, {cpus} CPUs");
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| VmmApp { line }),
        )
        .unwrap();
        cx.activate(true);
    });
}
