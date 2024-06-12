mod imp;

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

fn main() -> Result<(), eframe::Error> {
    gst::init().unwrap();
    {
        // Silence wgpu log spam (https://github.com/gfx-rs/wgpu/issues/3206)
        let mut rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());
        for loud_crate in ["naga", "wgpu_core", "wgpu_hal"] {
            if !rust_log.contains(&format!("{loud_crate}=")) {
                rust_log += &format!(",{loud_crate}=warn");
            }
        }
        std::env::set_var("RUST_LOG", rust_log);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 1024.0])
            .with_drag_and_drop(true),

        renderer: eframe::Renderer::Glow,

        ..Default::default()
    };
    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        options,
        Box::new(|cc| Box::new(imp::wrap::WrapApp::new(cc))),
    )
}
