# QRTimeStampSrc

Simple helper to test pipelines where the qrcode content is the unix timestamp of the machine in ms

To run:
```bash
cargo build --release
gst-launch-1.0 --gst-plugin-path=$PWD/target/release qrtimestampsrc ! fpsdisplaysink
```