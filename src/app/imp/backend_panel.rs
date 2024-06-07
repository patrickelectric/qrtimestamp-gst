use super::frame_history::FrameHistory;

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct BackendPanel {
    pub open: bool,

    #[serde(skip)]
    frame_history: FrameHistory,

    egui_windows: EguiWindows,
}

impl BackendPanel {
    pub fn update(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        self.frame_history
            .on_new_frame(ctx.input(|i| i.time), frame.info().cpu_usage);

        ctx.request_repaint();
    }

    pub fn end_of_frame(&mut self, ctx: &egui::Context) {
        self.egui_windows.windows(ctx);
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        integration_ui(ui, frame);

        ui.separator();

        self.frame_history.ui(ui);

        ui.separator();

        self.egui_windows.checkboxes(ui);
    }
}

fn integration_ui(ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    if _frame.gl().is_some() {
        ui.horizontal(|ui| {
            ui.label("Renderer:");
            ui.hyperlink_to("glow", "https://github.com/grovesNL/glow");
        });
    }

    if let Some(render_state) = _frame.wgpu_render_state() {
        wgpu_info_ui(ui, render_state);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        ui.horizontal(|ui| {
            let mut fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
            if ui
                .checkbox(&mut fullscreen, "🗖 Fullscreen (F11)")
                .on_hover_text("Fullscreen the window")
                .changed()
            {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
            }
        });
    }
}

fn wgpu_info_ui(ui: &mut egui::Ui, render_state: &egui_wgpu::RenderState) {
    let wgpu_adapter_details_ui = |ui: &mut egui::Ui, adapter: &eframe::wgpu::Adapter| {
        let info = &adapter.get_info();

        let eframe::wgpu::AdapterInfo {
            name,
            vendor,
            device,
            device_type,
            driver,
            driver_info,
            backend,
        } = &info;
        dbg!(&backend);

        // Example values:
        // > name: "llvmpipe (LLVM 16.0.6, 256 bits)", device_type: Cpu, backend: Vulkan, driver: "llvmpipe", driver_info: "Mesa 23.1.6-arch1.4 (LLVM 16.0.6)"
        // > name: "Apple M1 Pro", device_type: IntegratedGpu, backend: Metal, driver: "", driver_info: ""
        // > name: "ANGLE (Apple, Apple M1 Pro, OpenGL 4.1)", device_type: IntegratedGpu, backend: Gl, driver: "", driver_info: ""

        egui::Grid::new("adapter_info").show(ui, |ui| {
            ui.label("Backend:");
            ui.label(format!("{backend:?}"));
            ui.end_row();

            ui.label("Device Type:");
            ui.label(format!("{device_type:?}"));
            ui.end_row();

            if !name.is_empty() {
                ui.label("Name:");
                ui.label(format!("{name:?}"));
                ui.end_row();
            }
            if !driver.is_empty() {
                ui.label("Driver:");
                ui.label(format!("{driver:?}"));
                ui.end_row();
            }
            if !driver_info.is_empty() {
                ui.label("Driver info:");
                ui.label(format!("{driver_info:?}"));
                ui.end_row();
            }
            if *vendor != 0 {
                // TODO(emilk): decode using https://github.com/gfx-rs/wgpu/blob/767ac03245ee937d3dc552edc13fe7ab0a860eec/wgpu-hal/src/auxil/mod.rs#L7
                ui.label("Vendor:");
                ui.label(format!("0x{vendor:04X}"));
                ui.end_row();
            }
            if *device != 0 {
                ui.label("Device:");
                ui.label(format!("0x{device:02X}"));
                ui.end_row();
            }
        });
    };

    let wgpu_adapter_ui = |ui: &mut egui::Ui, adapter: &eframe::wgpu::Adapter| {
        let info = &adapter.get_info();
        ui.label(format!("{:?}", info.backend)).on_hover_ui(|ui| {
            wgpu_adapter_details_ui(ui, adapter);
        });
    };

    egui::Grid::new("wgpu_info").num_columns(2).show(ui, |ui| {
        ui.label("Renderer:");
        ui.hyperlink_to("wgpu", "https://wgpu.rs/");
        ui.end_row();

        ui.label("Backend:");
        wgpu_adapter_ui(ui, &render_state.adapter);
        ui.end_row();

        #[cfg(not(target_arch = "wasm32"))]
        if 1 < render_state.available_adapters.len() {
            ui.label("Others:");
            ui.vertical(|ui| {
                for adapter in &*render_state.available_adapters {
                    if adapter.get_info() != render_state.adapter.get_info() {
                        wgpu_adapter_ui(ui, adapter);
                    }
                }
            });
            ui.end_row();
        }
    });
}

#[derive(serde::Deserialize, serde::Serialize)]
struct EguiWindows {
    // egui stuff:
    settings: bool,
    inspection: bool,
    memory: bool,
    output_events: bool,

    #[serde(skip)]
    output_event_history: std::collections::VecDeque<egui::output::OutputEvent>,
}

impl Default for EguiWindows {
    fn default() -> Self {
        Self::none()
    }
}

impl EguiWindows {
    fn none() -> Self {
        Self {
            settings: false,
            inspection: false,
            memory: false,
            output_events: false,
            output_event_history: Default::default(),
        }
    }

    fn checkboxes(&mut self, ui: &mut egui::Ui) {
        let Self {
            settings,
            inspection,
            memory,
            output_events,
            output_event_history: _,
        } = self;

        ui.checkbox(settings, "🔧 Settings");
        ui.checkbox(inspection, "🔍 Inspection");
        ui.checkbox(memory, "📝 Memory");
        ui.checkbox(output_events, "📤 Output Events");
    }

    fn windows(&mut self, ctx: &egui::Context) {
        let Self {
            settings,
            inspection,
            memory,
            output_events,
            output_event_history,
        } = self;

        ctx.output(|o| {
            for event in &o.events {
                output_event_history.push_back(event.clone());
            }
        });
        while output_event_history.len() > 1000 {
            output_event_history.pop_front();
        }

        egui::Window::new("🔧 Settings")
            .open(settings)
            .vscroll(true)
            .show(ctx, |ui| {
                ctx.settings_ui(ui);
            });

        egui::Window::new("🔍 Inspection")
            .open(inspection)
            .vscroll(true)
            .show(ctx, |ui| {
                ctx.inspection_ui(ui);
            });

        egui::Window::new("📝 Memory")
            .open(memory)
            .resizable(false)
            .show(ctx, |ui| {
                ctx.memory_ui(ui);
            });

        egui::Window::new("📤 Output Events")
            .open(output_events)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(
                    "Recent output events from egui. \
            These are emitted when you interact with widgets, or move focus between them with TAB. \
            They can be hooked up to a screen reader on supported platforms.",
                );

                ui.separator();

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for event in output_event_history {
                            ui.label(format!("{event:?}"));
                        }
                    });
            });
    }
}
