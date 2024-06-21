#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

use gst::glib;

mod qrsink;
mod qrsrc;

pub const MINIMUM_SIZE: u32 = 100;
pub const MINIMUM_FPS: i32 = 10;
pub const MAXIMUM_FPS: i32 = 240;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    qrsink::register(plugin)?;
    qrsrc::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    qrtimestamp, // This name should be the lib name in Cargo.toml without the gst prefix
    "QRTimeStamp end-to-end test plugin\0",
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
