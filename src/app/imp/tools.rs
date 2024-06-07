use std::collections::BTreeSet;

use egui::{Context, ScrollArea, Ui};

use super::extra_viewport;
use super::pages::video::Video;
//use super::widgets::about::About;
//use super::widgets::gst_elements_info::GstElementInfo;
//use super::widgets::tools;
//use super::widgets::Tool;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct InnerTools {
    #[serde(skip)]
    tools: Vec<Vec<Box<dyn Tool>>>,

    open: BTreeSet<String>,
}

impl Default for InnerTools {
    fn default() -> Self {
        Self::from_tools(vec![
            vec![
                Box::<extra_viewport::ExtraViewport>::default(),
                Box::<tools::WindowResizeTest>::default(),
            ],
            vec![
                Box::<tools::CursorTest>::default(),
                Box::<tools::IdTest>::default(),
                Box::<tools::InputTest>::default(),
                Box::<tools::ManualLayoutTest>::default(),
                Box::<tools::TableTest>::default(),
            ],
        ])
    }
}

impl InnerTools {
    pub fn from_tools(tools: Vec<Vec<Box<dyn Tool>>>) -> Self {
        let open = BTreeSet::new();
        Self { tools, open }
    }

    pub fn checkboxes(&mut self, ui: &mut Ui) {
        let Self { tools, open } = self;
        for group in tools {
            for tool in group {
                if tool.is_enabled(ui.ctx()) {
                    let mut is_open = open.contains(tool.name());
                    ui.toggle_value(&mut is_open, tool.name());
                    set_open(open, tool.name(), is_open);
                }
            }
            ui.separator();
        }
    }

    pub fn windows(&mut self, ctx: &Context) {
        let Self { tools, open } = self;
        let flattened_tools: Vec<&mut Box<dyn Tool>> = tools.into_iter().flatten().collect();
        for tool in flattened_tools {
            let mut is_open = open.contains(tool.name());
            tool.show(ctx, &mut is_open);
            set_open(open, tool.name(), is_open);
        }
    }
}

fn set_open(open: &mut BTreeSet<String>, key: &'static str, is_open: bool) {
    if is_open {
        if !open.contains(key) {
            open.insert(key.to_owned());
        }
    } else {
        open.remove(key);
    }
}

/// A menu bar in which you can select different demo windows to show.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Tools {
    about_is_open: bool,
    about: About,
    gst_elements_info: GstElementInfo,

    #[serde(skip)]
    video: Video,

    tools: InnerTools,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            about_is_open: true,
            about: Default::default(),
            gst_elements_info: Default::default(),
            video: Default::default(),
            tools: Default::default(),
        }
    }
}

impl eframe::App for Tools {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui(ctx);
    }
}

impl Tools {
    pub fn ui(&mut self, ctx: &Context) {
        self.desktop_ui(ctx);
    }

    fn desktop_ui(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.push_id("Video Show", |ui| {
                egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                    self.video.show(ui);
                });
            });
        });

        egui::SidePanel::right("egui_demo_panel")
            .resizable(false)
            .default_width(150.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("🛠 Tools");
                });

                ui.separator();

                self.demo_list_ui(ui);

                ui.separator();
            });

        self.show_windows(ctx);
    }

    fn show_windows(&mut self, ctx: &Context) {
        self.about.show(ctx, &mut self.about_is_open);
        self.gst_elements_info.show(ctx, &mut self.about_is_open);
        self.tools.windows(ctx);
    }

    fn demo_list_ui(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                ui.toggle_value(&mut self.about_is_open, self.about.name());
                ui.toggle_value(&mut self.about_is_open, self.gst_elements_info.name());

                ui.separator();
                self.tools.checkboxes(ui);
                // Tools will add a separator in the end for us

                if ui.button("Organize windows").clicked() {
                    ui.ctx().memory_mut(|mem| mem.reset_areas());
                }
            });
        });
    }
}
