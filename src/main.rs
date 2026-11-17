//! Bandtools — a simple guitar studio: amps, cabinets, pedals, tabs,
//! YouTube stem splitting and local open-source AI helpers.

mod ai;
mod app;
mod audio;
mod dsp;
mod jobs;
mod offscreen;
mod tabs;
mod widgets;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let shot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1).cloned());

    let mut viewport = egui::ViewportBuilder::default().with_inner_size([1360.0, 880.0]).with_min_inner_size([980.0, 640.0]).with_title("Bandtools — Guitar Studio");
    if shot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
        // macOS stops drawing fully covered windows, so keep the (unfocused)
        // capture window above others for the second it takes.
        #[cfg(target_os = "macos")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnTop);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if shot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native("Bandtools", options, Box::new(move |cc| Ok(Box::new(app::BandApp::new(cc, shot)))))
}
