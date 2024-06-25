use glib::bool_error;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;

use std::sync::Mutex;

use once_cell::sync::Lazy;
use qrc::QRCode;

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
    time_frame_creation: u128,
    time_previous_iteration: u128,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            fps: gst::Fraction::from(DEFAULT_FPS),
            width: DEFAULT_SIZE,
            height: DEFAULT_SIZE,
            time_frame_creation: 0,
            time_previous_iteration: 0,
        }
    }
}

#[derive(Default)]
struct State {
    info: Option<gst_video::VideoInfo>,
    sample_offset: u64,
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

#[derive(Default)]
pub struct QRTimeStampSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    clock_wait: Mutex<ClockWait>,
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
            gst::loggable_error!(CAT, "Failed to build `VideoInfo` from caps {caps}")
        })?;

        gst::debug!(CAT, imp: self, "Configuring for caps {caps}");

        self.settings.lock().unwrap().fps = gst_video::VideoInfo::from_caps(caps).unwrap().fps();
        let width = gst_video::VideoInfo::from_caps(caps).unwrap().width();
        let height = gst_video::VideoInfo::from_caps(caps).unwrap().height();
        if width != height {
            return Err(gst::LoggableError::new(
                *CAT,
                bool_error!("Width ({width}) and height ({height}) should be from the same size"),
            ));
        }
        self.settings.lock().unwrap().width = width;
        self.settings.lock().unwrap().height = height;

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
                    let fps = self.settings.lock().unwrap().fps;
                    let latency = gst::ClockTime::SECOND
                        .mul_div_floor(fps.denom() as u64, fps.numer() as u64)
                        .unwrap();
                    gst::info!(CAT, imp: self, "Returning latency {}", latency);
                    return true;
                }

                #[allow(clippy::needless_return)]
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
        let mut settings = self.settings.lock().unwrap();
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();

        if settings.time_previous_iteration == 0 {
            settings.time_previous_iteration = current_time;
        }

        let mut state = self.state.lock().unwrap();
        if state.info.is_none() {
            gst::element_imp_error!(self, gst::CoreError::Negotiation, ["Have no caps yet"]);
            return Err(gst::FlowError::NotNegotiated);
        };
        let mut buffer =
            gst::Buffer::with_size((settings.width * settings.height * 3) as usize).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();

            let duration = (gst::ClockTime::SECOND)
                .mul_div_floor(settings.fps.denom() as u64, settings.fps.numer() as u64)
                .unwrap();

            let pts = (state.sample_offset * gst::ClockTime::SECOND)
                .mul_div_floor(settings.fps.denom() as u64, settings.fps.numer() as u64)
                .unwrap();

            buffer.set_pts(pts);
            buffer.set_duration(duration);

            let mut map = buffer.map_writable().unwrap();
            let data = map.as_mut_slice();

            {
                let time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                let metadata = (time.as_millis()
                    + (settings.time_frame_creation + current_time
                        - settings.time_previous_iteration)
                        / 1000)
                    .to_string();
                let qr = QRCode::from_string(metadata);

                // RGBA to RGB transformation
                for (output, chunk) in data
                    .chunks_exact_mut(3)
                    .zip(qr.to_png(settings.width).as_raw().chunks_exact(4))
                {
                    output.copy_from_slice(&chunk[0..3]);
                }
            }

            settings.time_frame_creation = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros()
                - current_time;
            settings.time_previous_iteration = current_time;
        }

        state.sample_offset += 1;
        if state.sample_offset > settings.num_buffers as u64 {
            gst::debug!(CAT, imp: self, "EOS after iterations as requested.");
            return Err(gst::FlowError::Eos);
        }
        drop(state);

        // If we're live, we are waiting until the time of the last sample in our buffer has
        // arrived. This is the very reason why we have to report that much latency.
        // A real live-source would of course only allow us to have the data available after
        // that latency, e.g. when capturing from a microphone, and no waiting from our side
        // would be necessary..
        //
        // Waiting happens based on the pipeline clock, which means that a real live source
        // with its own clock would require various translations between the two clocks.
        // This is out of scope for the tutorial though.
        {
            let (clock, base_time) = match Option::zip(self.obj().clock(), self.obj().base_time()) {
                None => return Ok(CreateSuccess::NewBuffer(buffer)),
                Some(res) => res,
            };

            let segment = self
                .obj()
                .segment()
                .downcast::<gst::format::Time>()
                .unwrap();
            let running_time = segment.to_running_time(buffer.pts().opt_add(buffer.duration()));

            // The last sample's clock time is the base time of the element plus the
            // running time of the last sample
            let wait_until = match running_time.opt_add(base_time) {
                Some(wait_until) => wait_until,
                None => return Ok(CreateSuccess::NewBuffer(buffer)),
            };

            // Store the clock ID in our struct unless we're flushing anyway.
            // This allows to asynchronously cancel the waiting from unlock()
            // so that we immediately stop waiting on e.g. shutdown.
            let mut clock_wait = self.clock_wait.lock().unwrap();
            if clock_wait.flushing {
                gst::debug!(CAT, imp: self, "Flushing");
                return Err(gst::FlowError::Flushing);
            }

            let id = clock.new_single_shot_id(wait_until);
            clock_wait.clock_id = Some(id.clone());
            drop(clock_wait);

            gst::log!(
                CAT,
                imp: self,
                "Waiting until {}, now {}",
                wait_until,
                clock.time().display(),
            );
            let (res, jitter) = id.wait();
            gst::log!(CAT, imp: self, "Waited res {:?} jitter {}", res, jitter);
            self.clock_wait.lock().unwrap().clock_id.take();

            // If the clock ID was unscheduled, unlock() was called
            // and we should return Flushing immediately.
            if res == Err(gst::ClockError::Unscheduled) {
                gst::debug!(CAT, imp: self, "Flushing");
                return Err(gst::FlowError::Flushing);
            }
        }

        if let Some(info) = &self.state.lock().unwrap().info {
            let obj = self.obj();
            obj.emit_by_name::<()>("on-create", &[&info]);
        }

        Ok(CreateSuccess::NewBuffer(buffer))
    }
}
