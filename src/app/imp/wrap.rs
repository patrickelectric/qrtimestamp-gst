use eframe::glow;

use super::backend_panel::BackendPanel;
use super::pages::video::Video;
use egui::{Modifiers, Ui};

#[derive(Default)]
pub struct ColorTestApp {
    video: Video,
}

impl eframe::App for ColorTestApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.push_id("Video Show", |ui| {
                ui.add(egui::TextEdit::singleline(&mut current_filter).desired_width(120.0));
                if ui.button("ｘ").clicked() {
                    current_filter.clear();
                }
                egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                    self.video.show(ui);
                });
            });
        });
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Anchor {
    Video,
}

impl std::fmt::Display for Anchor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<Anchor> for egui::WidgetText {
    fn from(value: Anchor) -> Self {
        Self::RichText(egui::RichText::new(value.to_string()))
    }
}

impl Default for Anchor {
    fn default() -> Self {
        Self::Video
    }
}

#[derive(Clone, Copy, Debug)]
#[must_use]
enum Command {
    Nothing,
    ResetEverything,
}

#[derive(Default)]
pub struct State {
    // tools: Tools,
    color_test: ColorTestApp,

    selected_anchor: Anchor,
    backend_panel: BackendPanel,
}

pub struct WrapApp {
    state: State,
}

impl WrapApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This gives us image support:
        egui_extras::install_image_loaders(&cc.egui_ctx);

        #[allow(unused_mut)]
        let mut slf = Self {
            state: State::default(),
        };

        slf
    }

    fn apps_iter_mut(&mut self) -> impl Iterator<Item = (&str, Anchor, &mut dyn eframe::App)> {
        let vec = vec![(
            "🎥 Video",
            Anchor::Video,
            &mut self.state.color_test as &mut dyn eframe::App,
        )];

        vec.into_iter()
    }
}

impl eframe::App for WrapApp {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
            let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }

        let mut cmd = Command::Nothing;
        egui::TopBottomPanel::top("wrap_app_top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.visuals_mut().button_frame = false;
                self.bar_contents(ui, frame, &mut cmd);
            });
        });

        self.state.backend_panel.update(ctx, frame);

        cmd = self.backend_panel(ctx, frame);

        self.show_selected_app(ctx, frame);

        self.state.backend_panel.end_of_frame(ctx);

        self.run_cmd(ctx, cmd);
    }

    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        println!("Finishing the program!");
    }

    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(&mut *self)
    }
}

impl WrapApp {
    fn backend_panel(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) -> Command {
        // The backend-panel can be toggled on/off.
        // We show a little animation when the user switches it.
        let is_open =
            self.state.backend_panel.open || ctx.memory(|mem| mem.everything_is_visible());

        let mut cmd = Command::Nothing;

        egui::SidePanel::left("backend_panel")
            .resizable(false)
            .show_animated(ctx, is_open, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("💻 Debug");
                });

                ui.separator();
                self.backend_panel_contents(ui, frame, &mut cmd);

                if !cfg!(target_arch = "wasm32") {
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            });

        cmd
    }

    fn run_cmd(&mut self, ctx: &egui::Context, cmd: Command) {
        match cmd {
            Command::Nothing => {}
            Command::ResetEverything => {
                self.state = Default::default();
                ctx.memory_mut(|mem| *mem = Default::default());
            }
        }
    }

    fn backend_panel_contents(
        &mut self,
        ui: &mut egui::Ui,
        frame: &mut eframe::Frame,
        cmd: &mut Command,
    ) {
        self.state.backend_panel.ui(ui, frame);

        ui.separator();

        ui.horizontal(|ui| {
            if ui
                .button("Reset windows")
                .on_hover_text("Forget scroll, positions, sizes etc")
                .clicked()
            {
                ui.ctx().memory_mut(|mem| *mem = Default::default());
                ui.close_menu();
            }

            if ui.button("Reset everything").clicked() {
                *cmd = Command::ResetEverything;
                ui.close_menu();
            }
        });
    }

    fn show_selected_app(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let selected_anchor = self.state.selected_anchor;
        for (_name, anchor, app) in self.apps_iter_mut() {
            if anchor == selected_anchor || ctx.memory(|mem| mem.everything_is_visible()) {
                app.update(ctx, frame);
            }
        }
    }

    fn bar_contents(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame, _cmd: &mut Command) {
        egui::widgets::global_dark_light_mode_switch(ui);

        ui.separator();

        let mut selected_anchor = self.state.selected_anchor;

        for (name, anchor, _app) in self.apps_iter_mut() {
            if ui
                .selectable_label(selected_anchor == anchor, name)
                .clicked()
            {
                selected_anchor = anchor;
                if frame.is_web() {
                    ui.ctx()
                        .open_url(egui::OpenUrl::same_tab(format!("#{anchor}")));
                }
            }
        }

        {
            ui.separator();
            ui.toggle_value(&mut self.state.backend_panel.open, "💻 Debug");
            ui.separator();
        }

        self.state.selected_anchor = selected_anchor;

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::warn_if_debug_build(ui);
        });
    }
}
