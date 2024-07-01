use gst::prelude::*;
use gstqrtimestamp::MAXIMUM_FPS;
use std::sync::{Arc, Mutex};

fn prepare() {
    gst::init().unwrap();

    gstqrtimestamp::plugin_register_static().unwrap();
}

#[test]
fn main() {
    prepare();

    // Build the test pipeline
    let buffers = 10;
    let fps = MAXIMUM_FPS;
    let pipeline = gst::parse::launch(&format!(
        concat!(
            "qrtimestampsrc name=src num-buffers={buffers}",
            " ! video/x-raw,framerate={fps}/1",
            " ! qrtimestampsink name=sink",
        ),
        buffers = buffers,
        fps = fps,
    ))
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();

    // Gather all sent infos
    let sent_infos = Arc::new(Mutex::new(Vec::with_capacity(buffers)));
    let sent_infos_cloned = sent_infos.clone();
    let qrtimestampsrc = pipeline.by_name("src").unwrap();
    qrtimestampsrc.connect("on-create", false, move |values| {
        let _element = values[0].get::<gst::Element>().expect("Invalid argument");
        let info = values[1]
            .get::<gst_video::VideoInfo>()
            .expect("Invalid argument");

        sent_infos_cloned.lock().unwrap().push(info);

        None
    });

    // Gather all received infos
    let recv_infos = Arc::new(Mutex::new(Vec::with_capacity(buffers)));
    let recv_infos_cloned = recv_infos.clone();
    let qrtimestampsink = pipeline.by_name("sink").unwrap();
    qrtimestampsink.connect("on-render", false, move |values| {
        let _element = values[0].get::<gst::Element>().expect("Invalid argument");
        let info = values[1]
            .get::<gst_video::VideoInfo>()
            .expect("Invalid argument");

        recv_infos_cloned.lock().unwrap().push(info);

        None
    });

    // Start
    pipeline.set_state(gst::State::Playing).unwrap();

    // Wait for EOS
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => {
                println!("EOS Recived");
                break;
            }
            MessageView::Error(err) => {
                eprintln!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                break;
            }
            MessageView::Latency(_latency) => {
                pipeline.recalculate_latency().unwrap();
            }
            _ => (),
        }
    }

    // Cleanup
    pipeline.set_state(gst::State::Null).unwrap();
    while pipeline.current_state() != gst::State::Null {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    // Asserts
    let sent = sent_infos.lock().unwrap();
    let recv = recv_infos.lock().unwrap();
    let all_same = !recv
        .iter()
        .zip(sent.iter())
        .any(|(recv, sent)| !recv.eq(sent));

    dbg!(&sent[0], &recv[0]);

    assert!(all_same, "{recv:#?}\n{sent:#?}");
}
