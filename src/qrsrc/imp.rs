use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;

use std::sync::Mutex;
use std::u32;

use qrc::QRCode;
use once_cell::sync::Lazy;
use sinais::*;

use tokio::time::{sleep, Duration};

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "qrcodelinuxtimestamp",
        gst::DebugColorFlags::empty(),
        Some("Generate qrcodes based on current linux timestamp"),
    )
});

const DEFAULT_FPS: u32 = 30;

struct VideoHandler {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
}

#[derive(Debug, Clone, Copy)]
struct Settings {
    fps: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            fps: DEFAULT_FPS,
        }
    }
}

struct State {
    info: Option<gst_video::VideoInfo>,
    sample_offset: u64,
}

impl Default for State {
    fn default() -> State {
        State {
            info: None,
            sample_offset: 0,
        }
    }
}

struct ClockWait {
    clock_id: Option<gst::SingleShotClockId>,
    flushing: bool,
}

impl Default for ClockWait {
    fn default() -> ClockWait {
        ClockWait {
            clock_id: None,
            flushing: true,
        }
    }
}

pub struct QRTimeStampSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    clock_wait: Mutex<ClockWait>,
    video_handler: Mutex<VideoHandler>,
}

impl Default for QRTimeStampSrc {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(3);
        _spawn("QCode generator".into(), async move {
            loop {
                let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                let metadata = time.as_millis().to_string();
                let qr = QRCode::from_string(metadata);
                let _ = tx.try_send(qr.to_png(200).as_raw().clone());
                sleep(Duration::from_millis(33)).await;
            }
        });
        Self {
            settings: Default::default(),
            state: Default::default(),
            clock_wait: Default::default(),
            video_handler: Mutex::new(VideoHandler { rx }),

        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QRTimeStampSrc {
    const NAME: &'static str = "GstRsQRTimeStampSrc";
    type Type = super::QRTimeStampSrc;
    type ParentType = gst_base::PushSrc;
}

impl ObjectImpl for QRTimeStampSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecUInt::builder("fps")
                    .nick("Frequency")
                    .blurb("Frequency")
                    .minimum(1)
                    .default_value(DEFAULT_FPS)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_live(true);
        obj.set_format(gst::Format::Time);
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "fps" => {
                let mut settings = self.settings.lock().unwrap();
                let fps = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp: self,
                    "Changing fps from {} to {}",
                    settings.fps,
                    fps
                );
                settings.fps = fps;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "fps" => {
                let settings = self.settings.lock().unwrap();
                settings.fps.to_value()
            }
            _ => unimplemented!(),
        }
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
                .format_list([
                    gst_video::VideoFormat::Rgb,
                ])
                .height(200)
                .width(200)
                .framerate_range(gst::Fraction::from(1)..gst::Fraction::from(100))
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
        // Configure live'ness once here just before starting the source
        if let gst::StateChange::ReadyToPaused = transition {
            self.obj().set_live(true);
        }

        self.parent_change_state(transition)
    }
}

impl BaseSrcImpl for QRTimeStampSrc {
    // Called whenever the input/output caps are changing
    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let info = gst_video::VideoInfo::from_caps(caps).map_err(|_| {
            gst::loggable_error!(CAT, "Failed to build `VideoInfo` from caps {}", caps)
        })?;

        gst::debug!(CAT, imp: self, "Configuring for caps {}", caps);

        let mut state = self.state.lock().unwrap();

        *state = State {
            info: Some(info),
            sample_offset: 0,
        };

        drop(state);

        let _ = self
            .obj()
            .post_message(gst::message::Latency::builder().src(&*self.obj()).build());

        Ok(())
    }

    // Called when starting, so we can initialize all stream-related state to its defaults
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        // Reset state
        *self.state.lock().unwrap() = Default::default();
        self.unlock_stop()?;

        gst::debug!(CAT, imp: self, "Started");

        Ok(())
    }

    // Called when shutting down the element so we can release all stream-related state
    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.lock().unwrap() = Default::default();
        self.unlock()?;

        gst::debug!(CAT, imp: self, "Stopped");

        Ok(())
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;
        gst::debug!(CAT, imp: self, "Query: {query:#?}");

        match query.view_mut() {
            QueryViewMut::Latency(_latency) => {
                let state = self.state.lock().unwrap();
                if let Some(ref _info) = state.info {
                    let latency = gst::ClockTime::SECOND
                        .mul_div_floor(1 as u64,  30 as u64)
                        .unwrap();
                    gst::info!(CAT, imp: self, "Returning latency {}", latency);
                    return true;
                }
                return false;
            }
            _ => BaseSrcImplExt::parent_query(self, query),
        }
    }

    // Fixate the caps. BaseSrc will do some fixation for us, but
    // as we allow to use something like `fixate_field_nearest_int`
    fn fixate(&self, caps: gst::Caps) -> gst::Caps {
        self.parent_fixate(caps)
    }

    fn is_seekable(&self) -> bool {
        false
    }

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp: self, "Unlocking");
        let mut clock_wait = self.clock_wait.lock().unwrap();
        if let Some(clock_id) = clock_wait.clock_id.take() {
            clock_id.unschedule();
        }
        clock_wait.flushing = true;

        Ok(())
    }

    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp: self, "Unlock stop");
        let mut clock_wait = self.clock_wait.lock().unwrap();
        clock_wait.flushing = false;

        Ok(())
    }
}


impl PushSrcImpl for QRTimeStampSrc {
    fn create(
        &self,
        _buffer: Option<&mut gst::BufferRef>,
    ) -> Result<CreateSuccess, gst::FlowError> {
        let settings = *self.settings.lock().unwrap();

        let mut state = self.state.lock().unwrap();
        if state.info.is_none() {
            gst::element_imp_error!(self, gst::CoreError::Negotiation, ["Have no caps yet"]);
            return Err(gst::FlowError::NotNegotiated);
        };

        let mut buffer =
            gst::Buffer::with_size((200 as usize) * (200 as usize) * 3).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();

            let duration = (1)
                .mul_div_floor(*gst::ClockTime::SECOND, settings.fps as u64)
                .map(gst::ClockTime::from_nseconds)
                .unwrap();
            
            let pts = (state.sample_offset)
                .mul_div_floor(*gst::ClockTime::SECOND, settings.fps as u64)
                .map(gst::ClockTime::from_nseconds)
                .unwrap();

            buffer.set_pts(pts);
            buffer.set_duration(duration);

            let mut map = buffer.map_writable().unwrap();
            let data = map.as_mut_slice();

            // Transform RGBA to RGB
            if let Ok(image_vec) = self.video_handler.lock().unwrap().rx.recv() {
                for (output, chunk) in data.chunks_exact_mut(3).zip(image_vec.chunks_exact(4)) {
                    output.copy_from_slice(&chunk[0..3]);
                }
            }
        }
        
        state.sample_offset += 1;
        drop(state);
        Ok(CreateSuccess::NewBuffer(buffer))
    }
}