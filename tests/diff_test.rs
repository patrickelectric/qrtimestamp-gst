use gst::prelude::*;
use std::sync::{Arc, Mutex};

fn prepare() {
    gst::init().unwrap();

    gstqrtimestamp::plugin_register_static().unwrap();
}

#[test]
/// Here we are creating a pipeline with a queue that adds a fixed time delay, so we can assert the correctness of the computed latency (diff)
fn main() {
    prepare();

    // Build the test pipeline
    let fps = 100;
    let queue_buffers = 10;
    let latency = std::time::Duration::from_secs_f64((queue_buffers as f64) / (fps as f64));
    let minimum_test_bufferrs = 100;
    let buffers = 1 + minimum_test_bufferrs + queue_buffers;
    dbg!(&queue_buffers, &fps, &latency, &buffers);
    let pipeline_description = format!(
        concat!(
            "qrtimestampsrc name=src num-buffers={buffers} do-timestamp=true",
            " ! video/x-raw,framerate={fps}/1",
            " ! queue leaky=downstream silent=true",
            " max-size-buffers={queue_buffers} min-threshold-buffers={queue_buffers}", // Limit in buffers
            " max-size-bytes=0 min-threshold-bytes=0", // Disable bytes
            " max-size-time=0 min-threshold-time=0",   // Disable time
            " ! qrtimestampsink name=sink sync=false",
        ),
        buffers = buffers,
        fps = fps,
        queue_buffers = queue_buffers as u64,
    );
    let pipeline = gst::parse::launch(&pipeline_description)
        .unwrap()
        .downcast::<gst::Pipeline>()
        .unwrap();

    // Gather all latencies
    let latencies = Arc::new(Mutex::new(Vec::with_capacity(buffers)));
    let latencies_cloned = latencies.clone();
    let qrtimestampsink = pipeline.by_name("sink").unwrap();
    qrtimestampsink.connect("on-render", false, move |values| {
        let _element = values[0].get::<gst::Element>().expect("Invalid argument");
        let _info = values[1]
            .get::<gst_video::VideoInfo>()
            .expect("Invalid argument");
        let diff = values[2].get::<i64>().expect("Invalid argument");

        latencies_cloned.lock().unwrap().push(diff);

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

    // Preparing results
    let latencies = latencies.lock().unwrap();
    // Notes:
    //      1. We are skipping the first frame as it will always have a high value, possibly from the negotiation
    //      2. We are skipping the last frames because they are the ones that were retained in the queue,
    //      thus as there is no more frames coming in, they are released faster, which lowers their latency
    let latencies = &latencies.iter().map(|i| *i as f64).collect::<Vec<_>>()
        [1..=latencies.len() - queue_buffers];
    dbg!(&latencies);

    let jitters = &latencies
        .windows(2)
        .map(|a| a[1] - a[0])
        .collect::<Vec<_>>()[..];
    dbg!(&jitters);

    let expected_latency = latency.as_millis() as f64;
    let expected_max_jitter = 1f64;

    // Asserts
    use statrs::statistics::Statistics;
    let latency_min = latencies.min();
    let latency_max = latencies.max();
    let latency_mean = latencies.mean();
    let latency_std_dev = latencies.std_dev();
    let latency_variance = latencies.variance();
    let jitter_min = jitters.min();
    let jitter_max = jitters.max();
    let jitter_mean = jitters.mean();
    let jitter_std_dev = jitters.std_dev();
    let jitter_variance = jitters.variance();

    dbg!(
        &latency_min,
        &latency_max,
        &latency_mean,
        &latency_std_dev,
        &latency_variance
    );
    dbg!(
        &jitter_min,
        &jitter_max,
        &jitter_mean,
        &jitter_std_dev,
        &jitter_variance
    );

    dbg!(&expected_latency, &expected_max_jitter);

    assert!(latency_mean >= expected_latency - expected_max_jitter);
    assert!(latency_max <= expected_latency + expected_max_jitter);
    assert!(jitter_mean <= expected_max_jitter);
    assert!(jitter_max <= expected_max_jitter);
}
