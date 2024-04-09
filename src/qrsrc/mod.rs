use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct QRTimeStampSrc(ObjectSubclass<imp::QRTimeStampSrc>) @extends gst_base::PushSrc, gst_base::BaseSrc, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "qrtimestampsrc",
        gst::Rank::NONE,
        QRTimeStampSrc::static_type(),
    )
}
