"""Log an audio asset."""

from __future__ import annotations

import io
import math
import wave

import rerun as rr


def make_sine_wav() -> bytes:
    sample_rate = 44_100
    duration_s = 2.0
    frequency_hz = 440.0
    amplitude = 0.35
    sample_count = int(duration_s * sample_rate)
    wav_buffer = io.BytesIO()

    with wave.open(wav_buffer, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(sample_rate)

        for sample_index in range(sample_count):
            t = sample_index / sample_rate
            sample = amplitude * math.sin(2.0 * math.pi * frequency_hz * t)
            wav.writeframesraw(
                int(sample * 32767).to_bytes(2, byteorder="little", signed=True)
            )

    return wav_buffer.getvalue()


rr.init("rerun_example_asset_audio", spawn=True)

rr.log(
    "audio/sine_440hz",
    rr.archetypes.AssetAudio(blob=make_sine_wav(), media_type="audio/wav"),
    static=True,
)
