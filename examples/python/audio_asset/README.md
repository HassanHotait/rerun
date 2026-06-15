# Audio asset

Logs an in-memory WAV file as an `AssetAudio`.
The audio asset is logged as static data, while a separate sample timeline logs scalar values.
The script sends a blueprint with an `AudioView` for the waveform and a `TimeSeriesView` for the sample counter.

Run from the repository root:

```powershell
pixi run codegen
pixi run py-build
pixi run uvpy examples/python/audio_asset/audio_asset.py
```

Press Play in the audio viewport to hear the asset through the native audio output.
The waveform cursor advances while the asset is playing.

To save the recording as an `.rrd` file:

```powershell
pixi run uvpy examples/python/audio_asset/audio_asset.py --save examples/python/audio_asset/audio_asset.rrd
```

To open the saved file with the locally built viewer:

```powershell
target\debug\rerun.exe examples/python/audio_asset/audio_asset.rrd
```

To use an already-running viewer:

```powershell
pixi run rerun
pixi run uvpy examples/python/audio_asset/audio_asset.py --connect
```
