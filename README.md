# QRTimeStampSrc

Simple helper to test pipelines where the qrcode content is the unix timestamp of the machine in ms

To run:
```bash
cargo build --release
gst-launch-1.0 --gst-plugin-path=$PWD/target/release qrtimestampsrc ! fpsdisplaysink
```

If you want to decode and get the time difference, you can run:
```bash
cargo build --release
gst-launch-1.0 --gst-plugin-path=$PWD/target/release/ qrtimestampsrc ! qrtimestampsink
```

It's also possible to change framerate and resolution from caps:
```bash
gst-launch-1.0 --gst-plugin-path=$PWD/target/release/ qrtimestampsrc ! video/x-raw,width=300,height=300,framerate=16/1 ! fpsdisplaysink
```