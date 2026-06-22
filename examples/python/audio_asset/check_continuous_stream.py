"""Log timestamped WAV files as an interrupted Rerun AudioStream."""

from __future__ import annotations

import argparse
import re
import wave
from dataclasses import dataclass
from datetime import datetime, timedelta
from pathlib import Path
from zoneinfo import ZoneInfo

import rerun as rr
import rerun.blueprint as rrb


DEFAULT_AUDIO_DIR = Path(r"C:\Users\Hasan\Downloads\OneDrive_1_6-22-2026\Case")
FILENAME_PATTERN = re.compile(
    r"^\d+_(?P<timestamp>\d{4}-\d{2}-\d{2}-\d{2}-\d{2}-\d{2})\.wav$",
    re.IGNORECASE,
)
FILENAME_TIME_FORMAT = "%Y-%m-%d-%H-%M-%S"


@dataclass(frozen=True)
class AudioChunk:
    path: Path
    timestamp: datetime
    duration: timedelta


def wav_duration(path: Path) -> timedelta:
    with wave.open(str(path), "rb") as wav:
        return timedelta(seconds=wav.getnframes() / wav.getframerate())


def find_audio_chunks(audio_dir: Path, timezone: ZoneInfo) -> list[AudioChunk]:
    chunks: list[AudioChunk] = []
    unmatched: list[Path] = []

    for path in audio_dir.rglob("*.wav"):
        match = FILENAME_PATTERN.match(path.name)
        if match is None:
            unmatched.append(path)
            continue

        local_timestamp = datetime.strptime(
            match.group("timestamp"), FILENAME_TIME_FORMAT
        ).replace(tzinfo=timezone)
        chunks.append(
            AudioChunk(
                path=path,
                timestamp=local_timestamp,
                duration=wav_duration(path),
            )
        )

    if unmatched:
        print(f"Skipping {len(unmatched)} WAV files with unrecognized filenames.")

    return sorted(chunks, key=lambda chunk: (chunk.timestamp, chunk.path.name))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--audio-dir", type=Path, default=DEFAULT_AUDIO_DIR)
    parser.add_argument(
        "--timezone",
        default="UTC",
        help=(
            "Timezone represented by filename timestamps. The default UTC preserves the "
            "filename wall-clock values in a UTC-configured viewer."
        ),
    )
    parser.add_argument("--timeline", default="recording_time")
    parser.add_argument("--save", type=Path, default=None)
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--connect", action="store_true")
    parser.add_argument("--viewer-path", type=Path, default=None)
    args = parser.parse_args()

    if not args.audio_dir.is_dir():
        raise FileNotFoundError(f"Audio directory does not exist: {args.audio_dir}")
    if not hasattr(rr.archetypes, "AudioStream"):
        raise RuntimeError(
            "This local Python SDK does not expose rr.archetypes.AudioStream. "
            "Run `pixi run codegen` and `pixi run py-build` first."
        )

    chunks = find_audio_chunks(args.audio_dir, ZoneInfo(args.timezone))
    if args.limit is not None:
        chunks = chunks[: args.limit]
    if not chunks:
        raise RuntimeError(f"No timestamped WAV files found in: {args.audio_dir}")

    blueprint = rrb.Blueprint(
        rrb.AudioView(origin="/audio/case", name="Case audio stream")
    )

    rr.init("case_audio_stream")
    if args.save is not None:
        rr.save(args.save, default_blueprint=blueprint)
    elif args.connect:
        rr.connect_grpc()
        rr.send_blueprint(blueprint)
    else:
        repo_root = Path(__file__).resolve().parents[3]
        local_viewer = repo_root / "target" / "debug" / "rerun.exe"
        viewer_path = args.viewer_path or (
            local_viewer if local_viewer.exists() else None
        )
        rr.spawn(executable_path=str(viewer_path) if viewer_path is not None else None)
        rr.send_blueprint(blueprint)

    print(
        f"Logging {len(chunks)} chunks from {chunks[0].timestamp.isoformat()} "
        f"to {chunks[-1].timestamp.isoformat()}"
    )

    for index, chunk in enumerate(chunks, start=1):
        rr.set_time(args.timeline, timestamp=chunk.timestamp)
        rr.log(
            "audio/case",
            rr.archetypes.AudioStream(
                chunk=chunk.path.read_bytes(),
                media_type="audio/wav"
                # filename=chunk.path.name,
            ),
        )

        if index % 100 == 0 or index == len(chunks):
            print(f"Logged {index}/{len(chunks)}: {chunk.path.name}")

    # Ensure global playback reaches the end of the final audio chunk.
    stream_end = max(chunk.timestamp + chunk.duration for chunk in chunks)
    rr.set_time(args.timeline, timestamp=stream_end)
    rr.log("timeline/audio_stream_end", rr.Scalars(1.0))

    rr.disconnect()
    if args.save is not None:
        print(f"Saved recording to {args.save}")


if __name__ == "__main__":
    main()
