# Audio stream

Logs two timeline-synchronized WAV chunks with a one-second interruption between them.

Build the local SDK and viewer after changing generated types or Rust viewer code:

```bat
pixi run codegen
pixi run py-build
set RERUN_DISABLE_WEB_VIEWER_SERVER=1
cargo build -p rerun-cli --bin rerun
```

Run the example with the local viewer:

```bat
pixi run uvpy examples/python/audio_stream/audio_stream.py
```

Save and reopen the recording:

```bat
pixi run uvpy examples/python/audio_stream/audio_stream.py --save examples/python/audio_stream/audio_stream.rrd
target\debug\rerun.exe examples\python\audio_stream\audio_stream.rrd
```
