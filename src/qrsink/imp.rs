use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::subclass::prelude::*;
use gst_video::{VideoFrameExt, VideoFrameRef};

use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::MAXIMUM_FPS;
use crate::MINIMUM_FPS;
use crate::MINIMUM_SIZE;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "qrtimestampsink",
        gst::DebugColorFlags::empty(),
        Some("Reads qrcodes based on current linux timestamp"),
    )
});

#[derive(Default)]
struct State {
    info: Option<gst_video::VideoInfo>,
}

#[derive(Default)]
pub struct QRTimeStampSink {
    state: Mutex<State>,
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

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
            vec![glib::subclass::Signal::builder("on-render")
                .param_types([gst_video::VideoInfo::static_type(), i64::static_type()])
                .build()]
        });

        SIGNALS.as_ref()
    }
}

impl GstObjectImpl for QRTimeStampSink {}

impl ElementImpl for QRTimeStampSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "QRCode Timestamp Sink",
                "Sink/Video",
                "The sink pair of qrtimestampsrc",
                "Patrick José Pereira <patrickelectric@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst_video::VideoCapsBuilder::default()
                .format_list([gst_video::VideoFormat::Rgb])
                .height_range(MINIMUM_SIZE as i32..i32::MAX)
                .width_range(MINIMUM_SIZE as i32..i32::MAX)
                .framerate_range(gst::Fraction::from(MINIMUM_FPS)..gst::Fraction::from(MAXIMUM_FPS))
                .build();
            // The src pad template must be named "src" for basesrc
            // and specific a pad that is always there
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

        let info = gst_video::VideoInfo::from_caps(caps)
            .map_err(|_| gst::loggable_error!(CAT, "Failed to build `VideoInfo` from caps {caps}"));
        self.state.lock().unwrap().info = info.ok();
        Ok(())
    }

    fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
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

        let Some(image_buffer) = image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::from_vec(
            frame.width(),
            frame.height(),
            data.to_vec(),
        ) else {
            gst::error!(
                CAT,
                "Problem creating image buffer: {}x{} ({})",
                frame.width(),
                frame.height(),
                data.len()
            );
            return Err(gst::FlowError::Error);
        };

        let mut qrcode_image =
            rqrr::PreparedImage::prepare(image::DynamicImage::ImageRgb8(image_buffer).to_luma8());
        let grids = qrcode_image.detect_grids();
        if grids.is_empty() {
            return Ok(gst::FlowSuccess::Ok);
        }
        let (_meta, content) = grids[0].decode().unwrap();
        let content = content.parse::<u128>().unwrap();
        let diff = if time.as_millis() > content {
            (time.as_millis() - content) as i64
        } else {
            0
        };

        if let Some(info) = &self.state.lock().unwrap().info {
            let obj = self.obj();
            obj.emit_by_name::<()>("on-render", &[&info, &diff]);
        }

        gst::debug!(
            CAT,
            imp: self,
            "Time difference: {diff} ms",
        );

        Ok(gst::FlowSuccess::Ok)
    }
}
