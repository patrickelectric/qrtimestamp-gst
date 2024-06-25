use glib::bool_error;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;

use std::sync::Mutex;

use once_cell::sync::Lazy;
use qrc::{qr_code_to, QRCode};

use crate::MAXIMUM_FPS;
use crate::MINIMUM_FPS;
use crate::MINIMUM_SIZE;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "qrtimestampsrc",
        gst::DebugColorFlags::empty(),
        Some("Generate qrcodes based on current linux timestamp"),
    )
});

const DEFAULT_FPS: i32 = 30;
const DEFAULT_SIZE: u32 = MINIMUM_SIZE;

#[derive(Debug, Clone, Copy)]
struct Settings {
    fps: gst::Fraction,
    width: u32,
    height: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            fps: gst::Fraction::from(DEFAULT_FPS),
            width: DEFAULT_SIZE,
            height: DEFAULT_SIZE,
        }
    }
}

#[derive(Default)]
struct State {
    info: Option<gst_video::VideoInfo>,

    /// Total running time for current caps
    running_time: gst::ClockTime,
    /// Total frames sent for current caps
    n_frames: u64,

    /// Accumulated running_time for previous caps
    accum_rtime: gst::ClockTime,
    /// Accumulated frames for previous caps
    accum_frames: u64,
}

#[derive(Default)]
pub struct QRTimeStampSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for QRTimeStampSrc {
    const NAME: &'static str = "GstRsQRTimeStampSrc";
    type Type = super::QRTimeStampSrc;
    type ParentType = gst_base::PushSrc;
}

impl ObjectImpl for QRTimeStampSrc {
    fn constructed(&self) {
        self.parent_constructed();

        // Set the obj defaults
        let obj = self.obj();
        obj.set_live(true);
        obj.set_format(gst::Format::Time);
        obj.set_num_buffers(-1);
        obj.set_automatic_eos(true);
        obj.set_do_timestamp(false);
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
            vec![glib::subclass::Signal::builder("on-create")
                .param_types([gst_video::VideoInfo::static_type()])
                .build()]
        });

        SIGNALS.as_ref()
    }
}

impl GstObjectImpl for QRTimeStampSrc {}

impl ElementImpl for QRTimeStampSrc {
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
            let caps = gst_video::VideoCapsBuilder::default()
                .format_list([gst_video::VideoFormat::Rgb])
                .height_range(MINIMUM_SIZE as i32..i32::MAX)
                .width_range(MINIMUM_SIZE as i32..i32::MAX)
                .framerate_range(
                    gst::Fraction::from(MINIMUM_FPS)..=gst::Fraction::from(MAXIMUM_FPS),
                )
                .build();
            // The src pad template must be named "src" for basesrc
            // and specific a pad that is always there
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    // Called whenever the state of the element should be changed. This allows for
    // starting up the element, allocating/deallocating resources or shutting down
    // the element again.
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        let res = self.parent_change_state(transition);
        match res {
            Ok(gst::StateChangeSuccess::Success) => {
                if transition.next() == gst::State::Paused {
                    // this is a live source
                    Ok(gst::StateChangeSuccess::NoPreroll)
                } else {
                    Ok(gst::StateChangeSuccess::Success)
                }
            }
            x => x,
        }
    }
}

impl BaseSrcImpl for QRTimeStampSrc {
    // Called whenever the input/output caps are changing
    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let mut settings = self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        let info = gst_video::VideoInfo::from_caps(caps).map_err(|_| {
            gst::loggable_error!(CAT, "Failed to build `VideoInfo` from caps {caps}")
        })?;

        gst::debug!(CAT, imp: self, "Configuring for caps {caps}");

        let width = info.width();
        let height = info.height();
        if width != height {
            return Err(gst::LoggableError::new(
                *CAT,
                bool_error!("Width ({width}) and height ({height}) should be from the same size"),
            ));
        }
        settings.width = width;
        settings.height = height;
        settings.fps = info.fps();

        state.info.replace(info);
        state.accum_rtime = state.accum_rtime + state.running_time;
        state.accum_frames += state.n_frames;
        state.running_time = gst::ClockTime::default();
        state.n_frames = 0;

        Ok(())
    }

    // Called when starting, so we can initialize all stream-related state to its defaults
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        // Reset state
        *self.state.lock().unwrap() = Default::default();

        gst::debug!(CAT, imp: self, "Started");

        Ok(())
    }

    #[doc(alias = "get_times")]
    fn times(&self, buffer: &gst::BufferRef) -> (Option<gst::ClockTime>, Option<gst::ClockTime>) {
        let mut start = None;
        let mut end = None;

        // For live sources, sync on the timestamp of the buffer
        if let Some(timestamp) = buffer.pts() {
            if let Some(duration) = buffer.duration() {
                end.replace(timestamp + duration);
            }
            start.replace(timestamp);
        }

        (start, end)
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;
        gst::debug!(CAT, imp: self, "Query: {query:#?}");

        match query.view_mut() {
            QueryViewMut::Convert(convert_query) => {
                let state = self.state.lock().unwrap();

                if let Some(info) = &state.info {
                    let (src_val, dest_fmt) = convert_query.get();

                    if let Some(dest_val) =
                        gst_video::VideoInfo::convert_generic(info, src_val, dest_fmt)
                    {
                        convert_query.set(src_val, dest_val);

                        #[allow(clippy::needless_return)]
                        return true;
                    }
                }

                #[allow(clippy::needless_return)]
                return false;
            }
            QueryViewMut::Latency(latency_query) => {
                let settings = self.settings.lock().unwrap();

                let fps = settings.fps;
                let latency = gst::ClockTime::SECOND
                    .mul_div_floor(fps.denom() as u64, fps.numer() as u64)
                    .unwrap();

                gst::debug!(CAT, imp: self, "Reporting latency of {latency}");

                latency_query.set(true, latency, gst::ClockTime::NONE);

                #[allow(clippy::needless_return)]
                return true;
            }
            QueryViewMut::Duration(duration_query) => {
                match duration_query.format() {
                    gst::Format::Bytes => {
                        let settings = self.settings.lock().unwrap();

                        let bytes = gst_round_up_4(
                            self.obj().num_buffers() as u32 * settings.width * settings.height,
                        );

                        let dur = gst::format::Bytes::from_u64(bytes as u64);

                        duration_query.set(dur);
                    }
                    gst::Format::Time => {
                        let settings = self.settings.lock().unwrap();

                        let dur = gst::ClockTime::SECOND
                            .mul_div_round(settings.fps.denom() as u64, settings.fps.numer() as u64)
                            .unwrap();

                        duration_query.set(dur);
                    }
                    _ =>
                    {
                        #[allow(clippy::needless_return)]
                        return false
                    }
                }

                #[allow(clippy::needless_return)]
                return true;
            }
            _ => BaseSrcImplExt::parent_query(self, query),
        }
    }

    fn is_seekable(&self) -> bool {
        false
    }
}

impl PushSrcImpl for QRTimeStampSrc {
    fn create(
        &self,
        _buffer: Option<&mut gst::BufferRef>,
    ) -> Result<CreateSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        if state.info.is_none() {
            gst::element_imp_error!(self, gst::CoreError::Negotiation, ["Have no caps yet"]);
            return Err(gst::FlowError::NotNegotiated);
        };

        let mut buffer =
            gst::Buffer::with_size((settings.width * settings.height * 3) as usize).unwrap();
        let buffer = buffer.make_mut();

        // Time
        {
            let pts = state.accum_rtime + state.running_time;
            buffer.set_pts(pts);
            buffer.set_dts(gst::ClockTime::NONE);

            gst::trace!(CAT,
                imp: self,
                "Timestamp: {pts} = accumulated {accum_rtime} + running time: {running_time}",
                pts=pts,
                accum_rtime=state.accum_rtime,
                running_time=state.running_time,
            );

            let offset = state.accum_frames + state.n_frames;
            buffer.set_offset(offset);
            state.n_frames += 1;

            let offset_end = offset + 1;
            buffer.set_offset_end(offset_end);

            let fps = settings.fps;
            let next_time = (gst::ClockTime::SECOND * state.n_frames)
                .mul_div_floor(fps.denom() as u64, fps.numer() as u64)
                .unwrap();

            let duration = next_time - state.running_time;
            buffer.set_duration(duration);

            state.running_time = next_time;
        }

        // Image
        {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();

            let data = (current_time.as_millis() as u64).to_string();
            let png = qr_code_to!(data.into(), "png", settings.width);

            let mut buffer_map = buffer.map_writable().unwrap();
            let rgb_channels = buffer_map.chunks_exact_mut(3);
            let rgba_channels = png.chunks_exact(4);

            // Copy the png (RGBA) into the RGB, omitting the alpha channel
            rgb_channels
                .zip(rgba_channels)
                .for_each(|(dest, src)| dest.copy_from_slice(&src[..3]));
        }

        if let Some(info) = &state.info {
            let obj = self.obj();
            obj.emit_by_name::<()>("on-create", &[info]);
        }

        Ok(CreateSuccess::NewBuffer(buffer.to_owned()))
    }
}

/// Rounds an integer value up to the next multiple of 4.
/// reference: https://gstreamer.freedesktop.org/documentation/gstreamer/gstutils.html?gi-language=c#GST_ROUND_UP_4
fn gst_round_up_4(num: u32) -> u32 {
    (num + 3) & !3
}
