#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

use gst::glib;

mod qrsrc;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    qrsrc::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    qrtimestamp,
    "license of the plugin, source package name, binary package name, origin where it comes from alllalalalalala \0",
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
