use egui::{Color32, NumExt as _, Stroke, Vec2};
use re_sdk_types::components::MediaType;
use re_ui::UiExt as _;
use re_viewer_context::UiLayout;

pub struct AudioUi {
    blob: re_sdk_types::datatypes::Blob,
    media_type: MediaType,
}

impl AudioUi {
    pub fn from_blob(
        blob: &re_sdk_types::datatypes::Blob,
        media_type: Option<&MediaType>,
    ) -> Option<Self> {
        let media_type = media_type
            .cloned()
            .or_else(|| MediaType::guess_from_data(blob.0.as_ref()))?;

        media_type.is_audio().then(|| Self {
            blob: blob.clone(),
            media_type,
        })
    }

    pub fn data_ui(&self, ui: &mut egui::Ui, ui_layout: UiLayout) {
        if ui_layout.is_single_line() {
            self.compact_ui(ui);
        } else {
            ui.separator();
            ui.strong("Audio Asset");
            self.selection_ui(ui);
        }
    }

    fn compact_ui(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(self.media_type.as_str());
        });
    }

    fn selection_ui(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(self.media_type.as_str());
            ui.weak("Open an Audio view to play this asset.");
        });

        if let Some(waveform) = WavWaveform::parse(self.blob.0.as_ref()) {
            waveform_ui(ui, &waveform);
            ui.label(format!(
                "{} Hz, {} channel{}, {:.2} s",
                waveform.sample_rate,
                waveform.channels,
                if waveform.channels == 1 { "" } else { "s" },
                waveform.duration_secs()
            ));
        } else {
            ui.label("Waveform preview is available for uncompressed WAV assets.");
        }
    }
}

struct WavWaveform {
    sample_rate: u32,
    channels: u16,
    peaks: Vec<f32>,
    sample_count: usize,
}

impl WavWaveform {
    fn duration_secs(&self) -> f32 {
        self.sample_count as f32 / self.sample_rate as f32
    }

    fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 12 || !bytes.starts_with(b"RIFF") || &bytes[8..12] != b"WAVE" {
            return None;
        }

        let mut format = None;
        let mut data = None;
        let mut offset = 12;

        while offset + 8 <= bytes.len() {
            let chunk_id = &bytes[offset..offset + 4];
            let chunk_len =
                u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
            let chunk_start = offset + 8;
            let chunk_end = chunk_start.checked_add(chunk_len)?;
            if chunk_end > bytes.len() {
                return None;
            }

            match chunk_id {
                b"fmt " => format = WavFormat::parse(&bytes[chunk_start..chunk_end]),
                b"data" => data = Some(&bytes[chunk_start..chunk_end]),
                _ => {}
            }

            offset = chunk_end + (chunk_len % 2);
        }

        let format = format?;
        let data = data?;
        if format.sample_rate == 0 || format.channels == 0 {
            return None;
        }

        let bytes_per_sample = usize::from(format.bits_per_sample / 8);
        let frame_size = bytes_per_sample.checked_mul(usize::from(format.channels))?;
        if bytes_per_sample == 0 || frame_size == 0 {
            return None;
        }

        let frame_count = data.len() / frame_size;
        let bucket_count = frame_count.clamp(1, 512);
        let frames_per_bucket = frame_count.div_ceil(bucket_count).max(1);
        let mut peaks = Vec::with_capacity(bucket_count);

        for bucket_start_frame in (0..frame_count).step_by(frames_per_bucket) {
            let bucket_end_frame = (bucket_start_frame + frames_per_bucket).min(frame_count);
            let mut peak = 0.0f32;

            for frame_index in bucket_start_frame..bucket_end_frame {
                let frame_start = frame_index * frame_size;
                for channel in 0..usize::from(format.channels) {
                    let sample_start = frame_start + channel * bytes_per_sample;
                    let sample = decode_wav_sample(
                        &data[sample_start..sample_start + bytes_per_sample],
                        format.audio_format,
                        format.bits_per_sample,
                    )?;
                    peak = peak.max(sample.abs());
                }
            }

            peaks.push(peak.at_most(1.0));
        }

        Some(Self {
            sample_rate: format.sample_rate,
            channels: format.channels,
            peaks,
            sample_count: frame_count,
        })
    }
}

struct WavFormat {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
}

impl WavFormat {
    fn parse(bytes: &[u8]) -> Option<Self> {
        (bytes.len() >= 16).then(|| Self {
            audio_format: u16::from_le_bytes([bytes[0], bytes[1]]),
            channels: u16::from_le_bytes([bytes[2], bytes[3]]),
            sample_rate: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            bits_per_sample: u16::from_le_bytes([bytes[14], bytes[15]]),
        })
    }
}

fn decode_wav_sample(bytes: &[u8], audio_format: u16, bits_per_sample: u16) -> Option<f32> {
    match (audio_format, bits_per_sample) {
        (1, 8) => Some((f32::from(bytes[0]) - 128.0) / 128.0),
        (1, 16) => Some(f32::from(i16::from_le_bytes(bytes.try_into().ok()?)) / 32768.0),
        (1, 24) => {
            let value = i32::from_le_bytes([
                bytes[0],
                bytes[1],
                bytes[2],
                if bytes[2] & 0x80 == 0 { 0x00 } else { 0xff },
            ]);
            Some(value as f32 / 8_388_608.0)
        }
        (1, 32) => Some(i32::from_le_bytes(bytes.try_into().ok()?) as f32 / 2_147_483_648.0),
        (3, 32) => Some(f32::from_le_bytes(bytes.try_into().ok()?)),
        _ => None,
    }
}

fn waveform_ui(ui: &mut egui::Ui, waveform: &WavWaveform) {
    let desired_size = Vec2::new(ui.available_width().at_least(160.0), 96.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let tokens = ui.tokens();

    painter.rect_filled(rect, 4.0, tokens.panel_bg_color);
    painter.hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, ui.visuals().weak_text_color()),
    );

    if waveform.peaks.is_empty() {
        return;
    }

    let color = Color32::from_rgb(82, 168, 255);
    let width = rect.width();
    let center_y = rect.center().y;
    let half_height = rect.height() * 0.42;

    for (index, peak) in waveform.peaks.iter().enumerate() {
        let x = rect.left() + width * (index as f32 + 0.5) / waveform.peaks.len() as f32;
        let y = half_height * peak;
        painter.vline(x, center_y - y..=center_y + y, Stroke::new(1.0, color));
    }
}
