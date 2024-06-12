use gst::prelude::*;

use gst_app::AppSink;

use tokio::sync::mpsc;

// use super::video_frame_history::VideoFrameHistory;

use egui::{vec2, Align2};

//use crate::app::gst_elements::gst_color_subtract;

pub struct Video {
    pipeline: gst::Pipeline,

    // change to signals
    rx: mpsc::Receiver<(gst::Sample, f64)>,

    image: Option<egui::ColorImage>,
    // frame_history: VideoFrameHistory,
}

impl Default for Video {
    fn default() -> Self {
        // gst_color_subtract::register_color_subtract_plugin();

        let pipeline_str = "videotestsrc pattern=ball is-live=true do-timestamp=true ! videoconvert ! video/x-raw,format=RGBA ! appsink name=sink emit-signals=true sync=false";
        let pipeline = gst::parse::launch(pipeline_str).unwrap();

        let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();
        let appsink = pipeline
            .by_name("sink")
            .unwrap()
            .dynamic_cast::<AppSink>()
            .unwrap();

        pipeline.set_state(gst::State::Playing).unwrap();

        let (tx, rx) = mpsc::channel(1);

        let started = std::time::Instant::now();
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink
                        .pull_sample()
                        .map_err(|_| gst::FlowError::Eos)
                        .unwrap();
                    tx.blocking_send((
                        sample,
                        std::time::Instant::now()
                            .duration_since(started)
                            .as_millis() as f64
                            / 1000.0,
                    ))
                    .map_err(|_| gst::FlowError::Eos)
                    .unwrap();
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        Self {
            pipeline,
            rx,
            image: None,
            // frame_history: VideoFrameHistory::default(),
        }
    }
}

impl Video {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        if let Ok((image, time)) = self.rx.try_recv() {
            let buffer = image.buffer().expect("Failed to get buffer from sample");
            let caps = image.caps().expect("Failed to get caps from sample");
            let caps_struct = caps.structure(0).unwrap();
            let width: i32 = caps_struct.get("width").unwrap();
            let height: i32 = caps_struct.get("height").unwrap();
            let _fraction = caps_struct.get::<gst::Fraction>("framerate").unwrap();
            self.image = Some(egui::ColorImage::from_rgba_unmultiplied(
                [width as usize, height as usize],
                buffer.map_readable().unwrap().as_slice(),
            ));
            // self.frame_history.on_new_frame(time);
        }
        if let Some(image) = self.image.clone() {
            let texture = ui
                .ctx()
                .load_texture("video_frame", image, egui::TextureOptions::LINEAR);
            let image = egui::Image::new(&texture)
                .fit_to_exact_size(vec2(ui.available_size().x, ui.available_size().y));
            let response = ui.add(image);

            let widget_rect = response.rect;
            let text = "🔥 Potato".to_string();
            let color = if ui.visuals().dark_mode {
                egui::Visuals::light().extreme_bg_color
            } else {
                egui::Visuals::dark().extreme_bg_color
            };
            let painter = egui::Painter::new(
                ui.ctx().clone(),
                egui::layers::LayerId::background(),
                widget_rect,
            );

            painter.rect_stroke(widget_rect, 0.0, (2.0, color));

            painter.debug_text(
                vec2(widget_rect.min.x + 4.0, widget_rect.min.y + 4.0).to_pos2(),
                Align2::LEFT_TOP,
                color,
                text,
            );
        }
    }
}
