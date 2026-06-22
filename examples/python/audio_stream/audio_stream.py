"""Log interrupted audio chunks synchronized to the main Rerun timeline."""

from __future__ import annotations

import argparse
import io
import math
import wave
from pathlib import Path

import rerun as rr
import rerun.blueprint as rrb


def make_sine_wav(frequency_hz: float, duration_s: float = 1.0) -> bytes:
    """Generate a mono 16-bit PCM WAV chunk."""
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


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--save", type=Path, default=None, help="Save to this .rrd file.")
    parser.add_argument("--viewer-path", type=Path, default=None)
    args = parser.parse_args()

    if not hasattr(rr.archetypes, "AudioStream"):
        raise RuntimeError("Run `pixi run codegen` and `pixi run py-build` first.")

    blueprint = rrb.Blueprint(rrb.AudioView(origin="/audio/stream", name="Audio stream"))

    rr.init("rerun_example_audio_stream")
    if args.save is not None:
        rr.save(args.save, default_blueprint=blueprint)
    else:
        repo_root = Path(__file__).resolve().parents[3]
        local_viewer = repo_root / "target" / "debug" / "rerun.exe"
        viewer_path = args.viewer_path or (local_viewer if local_viewer.exists() else None)
        rr.spawn(executable_path=str(viewer_path) if viewer_path is not None else None)
        rr.send_blueprint(blueprint)

    chunks = [(0.0, 440.0, "first_chunk.wav"), (2.0, 660.0, "second_chunk.wav")]
    for start_time, frequency_hz, filename in chunks:
        rr.set_time("time", duration=start_time)
        rr.log(
            "audio/stream",
            rr.archetypes.AudioStream(
                make_sine_wav(frequency_hz),
                media_type="audio/wav",
                filename=filename,
            ),
        )

    # Extend the recording range beyond the final audio chunk.
    rr.set_time("time", duration=3.0)
    rr.log("timeline/end", rr.Scalars(3.0))

    if args.save is not None:
        rr.disconnect()
        print(f"Saved recording to {args.save}")


if __name__ == "__main__":
    main()
