"""Log an interrupted audio stream."""

from __future__ import annotations

import io
import math
import wave

import rerun as rr


def make_sine_wav(frequency_hz: float, duration_s: float = 1.0) -> bytes:
    sample_rate = 44_100
    wav_buffer = io.BytesIO()

    with wave.open(wav_buffer, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(sample_rate)

        for sample_index in range(int(duration_s * sample_rate)):
            t = sample_index / sample_rate
            sample = 0.35 * math.sin(2.0 * math.pi * frequency_hz * t)
            wav.writeframesraw(
                int(sample * 32767).to_bytes(2, byteorder="little", signed=True)
            )

    return wav_buffer.getvalue()


rr.init("rerun_example_audio_stream", spawn=True)

rr.set_time("time", duration=0.0)
rr.log(
    "audio/stream",
    rr.archetypes.AudioStream(
        make_sine_wav(440.0),
        media_type="audio/wav",
        filename="first_chunk.wav",
    ),
)

# The missing interval from 1s to 2s is an interruption in the stream.
rr.set_time("time", duration=2.0)
rr.log(
    "audio/stream",
    rr.archetypes.AudioStream(
        make_sine_wav(660.0),
        media_type="audio/wav",
        filename="second_chunk.wav",
    ),
)
