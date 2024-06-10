use egui::util::History;
use egui_plot::{Legend, Line, PlotPoints};

pub struct FrameHistory {
    frame_times: History<f32>,
    last_fps: f32,
    last_mean_time: f32,
}

impl Default for FrameHistory {
    fn default() -> Self {
        let max_age: f32 = 1.0;
        let max_len = (max_age * 300.0).round() as usize;
        Self {
            frame_times: History::new(0..max_len, max_age),
            last_fps: 0.0,
            last_mean_time: 0.0,
        }
    }
}

impl FrameHistory {
    pub fn on_new_frame(&mut self, now: f64, previous_frame_time: Option<f32>) {
        let previous_frame_time = previous_frame_time.unwrap_or_default();
        if let Some(latest) = self.frame_times.latest_mut() {
            *latest = previous_frame_time; // rewrite history now that we know
        }
        self.frame_times.add(now, previous_frame_time);
    }

    pub fn mean_frame_time(&self) -> f32 {
        self.frame_times.average().unwrap_or_default()
    }

    pub fn fps(&self) -> f32 {
        let mean_time_interval = self.frame_times.mean_time_interval().unwrap_or_default();
        // Avoid division by zero
        if mean_time_interval < f32::EPSILON {
            return 0.0;
        }
        1.0 / mean_time_interval
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if self.last_fps == 0.0 {
            self.last_fps = self.fps();
        }
        self.last_fps = 0.95 * self.last_fps + 0.05 * self.fps();

        if self.last_mean_time == 0.0 {
            self.last_mean_time = self.mean_frame_time();
        }
        self.last_mean_time = 0.95 * self.last_mean_time + 0.05 * self.mean_frame_time();

        ui.label(format!("FPS: {:.2}", self.last_fps));
        ui.label(format!(
            "Mean CPU usage: {:.2} ms / frame",
            1e3 * self.last_mean_time
        ))
        .on_hover_text(
            "Includes all app logic, egui layout, tessellation, and rendering.\n\
            Does not include waiting for vsync.",
        );
        egui::warn_if_debug_build(ui);

        egui::CollapsingHeader::new("📊 CPU usage history")
            .default_open(false)
            .show(ui, |ui| {
                self.graph(ui);
            });
    }

    fn graph(&mut self, ui: &mut egui::Ui) {
        egui_plot::Plot::new("plot")
            .set_margin_fraction(egui::Vec2::new(0.0, 0.0))
            .y_axis_position(egui_plot::HPlacement::Right)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .legend(Legend::default())
            .include_y(0)
            .include_y(100)
            .height(ui.spacing().slider_width * 3.0)
            .show(ui, |plot_ui| {
                let sine_points = PlotPoints::from(
                    self.frame_times
                        .iter()
                        .map(|(time, cpu_usage)| [time, 1000.0 * cpu_usage as f64])
                        .collect::<Vec<[f64; 2]>>(),
                );
                plot_ui.line(Line::new(sine_points).name("Render (ms)"));
            });
    }
}
