use gst::glib;
use gst::subclass::prelude::*;
use gst_base::subclass::prelude::*;
use gst_video::{VideoFrameExt, VideoFrameRef};

use std::sync::Mutex;

use image;
use once_cell::sync::Lazy;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "qrcodelinuxtimestamp",
        gst::DebugColorFlags::empty(),
        Some("Reads qrcodes based on current linux timestamp"),
    )
});

struct State {
    info: Option<gst_video::VideoInfo>,
}

impl Default for State {
    fn default() -> State {
        State { info: None }
    }
}

pub struct QRTimeStampSink {
    state: Mutex<State>,
}

impl Default for QRTimeStampSink {
    fn default() -> Self {
        Self {
            state: Default::default(),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QRTimeStampSink {
    const NAME: &'static str = "GstRsQRTimeStampSink";
    type Type = super::QRTimeStampSink;
    type ParentType = gst_base::BaseSink;
}

impl ObjectImpl for QRTimeStampSink {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl GstObjectImpl for QRTimeStampSink {}

impl ElementImpl for QRTimeStampSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "QRCode Timestamp Source",
                "Source/Video",
                "Creates a QRCode based on the current linux timestamp",
                "Patrick José Pereira <patrickelectric@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::new_any();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl BaseSinkImpl for QRTimeStampSink {
    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        // Here you would parse the caps to ensure they are what you expect
        gst::info!(CAT, "Caps set: {caps}");

        let info = gst_video::VideoInfo::from_caps(caps).map_err(|_| {
            gst::loggable_error!(CAT, "Failed to build `VideoInfo` from caps {}", caps)
        });
        self.state.lock().unwrap().info = info.ok();
        Ok(())
    }

    fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        // dbg!("frame");
        let Some(info) = self.state.lock().unwrap().info.clone() else {
            return Ok(gst::FlowSuccess::Ok);
        };
        // We need to get time asap to avoid adding the time to the decode logic
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();

        let frame = VideoFrameRef::from_buffer_ref_readable(buffer, &info)
            .map_err(|_| gst::FlowError::Error)?;

        let Ok(data) = frame.plane_data(0) else {
            return Ok(gst::FlowSuccess::Ok);
        };

        // dbg!("data");

        let Some(image_buffer) = image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::from_vec(
            frame.width(),
            frame.height(),
            data.to_vec(),
        ) else {
            println!(
                "Problem creating image buffer: {}x{} ({},)",
                frame.width(),
                frame.height(),
                data.len()
            );
            return Err(gst::FlowError::Error);
        };

        image_buffer.save("/tmp/image.png");

        // dbg!("buffer");

        let mut qrcode_image =
            rqrr::PreparedImage::prepare(image::DynamicImage::ImageRgb8(image_buffer).to_luma8());
        let grids = qrcode_image.detect_grids();
        if grids.len() == 0 {
            return Ok(gst::FlowSuccess::Ok);
        }
        let (_meta, content) = match grids[0].decode() {
            Ok(result) => result,
            Err(error) => {
                println!("Error decoding QRCode: {}", error);
                return Ok(gst::FlowSuccess::Ok);
            }
        };
        let content = content.parse::<u128>().unwrap();
        let diff = time.as_millis() as i128 - content as i128;
        println!("Time difference: {diff} ms");
        Ok(gst::FlowSuccess::Ok)
    }
}
