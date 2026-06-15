"""Log an in-memory WAV file as a Rerun audio asset."""

from __future__ import annotations

import argparse
import io
import math
import wave
from pathlib import Path

import rerun as rr
import rerun.blueprint as rrb


def make_sine_wav(
    *,
    frequency_hz: float = 440.0,
    duration_s: float = 2.0,
    sample_rate: int = 44_100,
    amplitude: float = 0.35,
) -> bytes:
    """Generate a mono 16-bit PCM WAV file."""
    sample_count = int(duration_s * sample_rate)
    wav_buffer = io.BytesIO()

    with wave.open(wav_buffer, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(sample_rate)

        for sample_index in range(sample_count):
            t = sample_index / sample_rate
            sample = amplitude * math.sin(2.0 * math.pi * frequency_hz * t)
            wav.writeframesraw(int(sample * 32767).to_bytes(2, byteorder="little", signed=True))

    return wav_buffer.getvalue()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--connect", action="store_true", help="Connect to an already-running viewer.")
    parser.add_argument(
        "--viewer-path",
        type=Path,
        default=None,
        help="Path to a local rerun viewer executable to spawn.",
    )
    parser.add_argument(
        "--save",
        type=Path,
        default=None,
        help="Save the recording to this .rrd file instead of spawning a viewer.",
    )
    args = parser.parse_args()

    if not hasattr(rr.archetypes, "AssetAudio"):
        raise RuntimeError(
            "This local Python SDK does not expose rr.archetypes.AssetAudio yet. "
            "Run `pixi run codegen` and then `pixi run py-build` from the repository root."
        )
    if not hasattr(rrb, "AudioView"):
        raise RuntimeError(
            "This local Python SDK does not expose rrb.AudioView yet. "
            "Run `pixi run codegen` and then `pixi run py-build` from the repository root."
        )

    blueprint = rrb.Blueprint(
        rrb.AudioView(origin="/audio/sine_440hz", name="Audio waveform"),
        rrb.TimeSeriesView(origin="/timeline", name="Sample timeline"),
    )

    rr.init("rerun_example_audio_asset")
    if args.save is not None:
        rr.save(args.save, default_blueprint=blueprint)
    elif args.connect:
        rr.connect_grpc()
    else:
        repo_root = Path(__file__).resolve().parents[3]
        local_viewer = repo_root / "target" / "debug" / "rerun.exe"
        viewer_path = args.viewer_path or (local_viewer if local_viewer.exists() else None)
        rr.spawn(executable_path=str(viewer_path) if viewer_path is not None else None)

    audio_bytes = make_sine_wav()

    if args.save is None:
        rr.send_blueprint(blueprint)

    rr.log(
        "audio/sine_440hz",
        rr.archetypes.AssetAudio(blob=audio_bytes, media_type="audio/wav"),
        static=True,
    )

    for sample_index in range(20):
        rr.set_time("sample", sequence=sample_index)
        rr.log("timeline/sample_index", rr.Scalars(sample_index))

    if args.save is not None:
        rr.disconnect()
        print(f"Saved recording to {args.save}")


if __name__ == "__main__":
    main()
