use std::time::Instant;

use egui::{NumExt as _, Sense};
use re_sdk_types::blueprint::components::PlayState;
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, UiExt as _, icons};
use re_viewer_context::external::re_log_types::{EntityPath, TimeInt, TimeType};
use re_viewer_context::{
    IdentifiedViewSystem as _, Item, SystemCommand, SystemCommandSender as _, ViewClass,
    ViewClassRegistryError, ViewId, ViewQuery, ViewSpawnHeuristics, ViewState, ViewStateExt as _,
    ViewSystemExecutionError, ViewerContext, suggest_view_for_each_entity,
};

use crate::visualizer_system::{AudioEntry, AudioStreamEntry, AudioStreamSystem, AudioSystem};
use crate::waveform::{WavWaveform, waveform_ui};

#[derive(Default)]
pub struct AudioViewState {
    #[cfg(not(target_arch = "wasm32"))]
    playback: Option<Playback>,
}

impl re_byte_size::SizeBytes for AudioViewState {
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl ViewState for AudioViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn heap_size_bytes(&self) -> u64 {
        re_byte_size::SizeBytes::heap_size_bytes(self)
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct Playback {
    key: AudioKey,
    _stream: rodio::OutputStream,
    sink: rodio::Sink,
    started_at: Instant,
    timeline_offset_secs: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AudioKey {
    len: usize,
    prefix: [u8; 16],
}

impl AudioKey {
    fn from_blob(blob: &re_sdk_types::datatypes::Blob) -> Self {
        let bytes = blob.0.as_ref();
        let mut prefix = [0; 16];
        let copy_len = bytes.len().min(prefix.len());
        prefix[..copy_len].copy_from_slice(&bytes[..copy_len]);
        Self {
            len: bytes.len(),
            prefix,
        }
    }
}

#[derive(Default)]
pub struct AudioView;

type ViewType = re_sdk_types::blueprint::views::AudioView;

impl ViewClass for AudioView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Audio"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &icons::VIEW_GENERIC
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Audio view").markdown(
            "Shows audio assets and timeline-synchronized audio streams as waveforms and plays them through the native audio output.",
        )
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<AudioSystem>()?;
        system_registry.register_visualizer::<AudioStreamSystem>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<AudioViewState>::default()
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();
        let asset_views =
            suggest_view_for_each_entity::<AudioSystem>(ctx, include_entity).into_vec();
        let stream_views =
            suggest_view_for_each_entity::<AudioStreamSystem>(ctx, include_entity).into_vec();
        ViewSpawnHeuristics::new(asset_views.into_iter().chain(stream_views))
    }

    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        _view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        ui.weak("Audio playback and waveform are shown in the viewport.");
        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<AudioViewState>()?;
        let audio_entries = system_output
            .visualizer_data_or_default::<Vec<AudioEntry>>(AudioSystem::identifier())?;
        let stream_entries = system_output
            .visualizer_data_or_default::<Vec<AudioStreamEntry>>(AudioStreamSystem::identifier())?;

        let tokens = ui.tokens();
        let frame = egui::Frame::new().inner_margin(tokens.view_padding());
        let response = frame
            .show(ui, |ui| {
                let inner_ui_builder = egui::UiBuilder::new()
                    .layout(egui::Layout::top_down(egui::Align::LEFT))
                    .sense(Sense::click());
                ui.scope_builder(inner_ui_builder, |ui| {
                    audio_view_ui(ctx, ui, state, &audio_entries, &stream_entries)
                })
                .inner
            })
            .inner?;

        let hovered = ui.ctx().rect_contains_pointer(ui.layer_id(), response.rect);
        let clicked = hovered && ui.input(|i| i.pointer.primary_pressed());

        if hovered {
            ctx.selection_state().set_hovered(Item::View(query.view_id));
        }

        if clicked {
            ctx.command_sender()
                .send_system(SystemCommand::set_selection(Item::View(query.view_id)));
        }

        Ok(())
    }
}

fn audio_view_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut AudioViewState,
    audio_entries: &[AudioEntry],
    stream_entries: &[AudioStreamEntry],
) -> Result<egui::Response, ViewSystemExecutionError> {
    if !stream_entries.is_empty() {
        return Ok(audio_stream_ui(ctx, ui, state, stream_entries));
    }

    if audio_entries.is_empty() {
        return Ok(ui.weak("(empty)"));
    }

    if audio_entries.len() > 1 {
        ui.error_label(format!(
            "Can only show one audio asset at a time; was given {}. Update the query so that it returns a single audio asset and create additional views for the others.",
            audio_entries.len()
        ));
    }

    let entry = &audio_entries[0];
    let key = AudioKey::from_blob(&entry.blob);
    let waveform = WavWaveform::parse(entry.blob.0.as_ref());

    ui.horizontal(|ui| {
        playback_controls_ui(ui, state, entry, key);
        ui.label(entry.media_type.as_str());

        if let Some(waveform) = &waveform {
            ui.weak(format!(
                "{} Hz, {} channel{}, {:.2} s",
                waveform.sample_rate,
                waveform.channels,
                if waveform.channels == 1 { "" } else { "s" },
                waveform.duration_secs()
            ));
        }
    });

    ui.add_space(4.0);

    if let Some(waveform) = &waveform {
        let progress_secs = playback_progress_secs(state, key);
        let response = waveform_ui(ui, waveform, progress_secs);
        if progress_secs.is_some() {
            ui.ctx().request_repaint();
        }
        Ok(response)
    } else {
        Ok(ui.label("Waveform preview is available for uncompressed WAV assets."))
    }
}

fn audio_stream_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut AudioViewState,
    entries: &[AudioStreamEntry],
) -> egui::Response {
    let time_ctrl = ctx.time_ctrl;
    let Some(current_time) = time_ctrl.time_int() else {
        return ui.weak("Move the timeline cursor onto the audio stream.");
    };
    let Some(time_type) = time_ctrl.time_type() else {
        return ui.weak("The active timeline has no time type.");
    };

    let active_entry = entries.iter().find_map(|entry| {
        let offset_secs =
            timeline_delta_secs(time_type, entry.start_time, current_time, time_ctrl.fps())?;
        (0.0..f64::from(entry.waveform.duration_secs()))
            .contains(&offset_secs)
            .then_some((entry, offset_secs))
    });

    sync_stream_playback(
        state,
        active_entry,
        time_ctrl.play_state(),
        time_ctrl.speed(),
    );

    ui.horizontal(|ui| {
        ui.label(format!("Synchronized to {}", time_ctrl.timeline_name()));
        ui.label(entries[0].media_type.as_str());
        ui.weak(format!("{} chunks", entries.len()));
        let status = match (time_ctrl.play_state(), active_entry) {
            (PlayState::Paused, _) => "Paused",
            (_, Some(_)) => "Playing",
            (_, None) => "Interruption",
        };
        ui.weak(status);
    });
    ui.add_space(4.0);

    if time_ctrl.play_state() != PlayState::Paused {
        ui.ctx().request_repaint();
    }

    stream_waveform_ui(ui, entries, current_time, time_type, time_ctrl.fps())
}

fn stream_waveform_ui(
    ui: &mut egui::Ui,
    entries: &[AudioStreamEntry],
    current_time: TimeInt,
    time_type: TimeType,
    fps: Option<f32>,
) -> egui::Response {
    let desired_size = egui::vec2(ui.available_width().at_least(220.0), ui.available_height());
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());
    let painter = ui.painter_at(rect);
    let tokens = ui.tokens();
    painter.rect_filled(rect, 6.0, tokens.panel_bg_color);
    painter.hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
    );

    let first_time = entries[0].start_time;
    let total_secs = entries
        .iter()
        .filter_map(|entry| {
            timeline_delta_secs(time_type, first_time, entry.start_time, fps)
                .map(|start| start + f64::from(entry.waveform.duration_secs()))
        })
        .fold(0.0f64, f64::max)
        .max(f64::EPSILON);

    for entry in entries {
        let Some(start_secs) = timeline_delta_secs(time_type, first_time, entry.start_time, fps)
        else {
            continue;
        };
        let end_secs = start_secs + f64::from(entry.waveform.duration_secs());
        let left = egui::lerp(rect.left()..=rect.right(), (start_secs / total_secs) as f32);
        let right = egui::lerp(rect.left()..=rect.right(), (end_secs / total_secs) as f32);
        let segment_width = (right - left).max(1.0);
        let bar_count =
            ((segment_width / 5.0).floor() as usize).clamp(1, entry.waveform.samples.len());
        let center_y = rect.center().y;
        let half_height = rect.height() * 0.42;
        let color = egui::Color32::from_rgb(82, 168, 255);

        for index in 0..bar_count {
            let sample_index = index * entry.waveform.samples.len() / bar_count;
            let amplitude = entry.waveform.samples[sample_index];
            let x = left + segment_width * (index as f32 + 0.5) / bar_count as f32;
            let y = (half_height * amplitude).max(2.0);
            painter.vline(
                x,
                center_y - y..=center_y + y,
                egui::Stroke::new(2.0, color),
            );
        }
    }

    if let Some(cursor_secs) = timeline_delta_secs(time_type, first_time, current_time, fps) {
        let progress = (cursor_secs / total_secs).clamp(0.0, 1.0) as f32;
        let x = egui::lerp(rect.left()..=rect.right(), progress);
        let cursor_color = egui::Color32::from_rgb(255, 210, 80);
        painter.vline(x, rect.y_range(), egui::Stroke::new(2.0, cursor_color));
    }

    response
}

fn timeline_delta_secs(
    time_type: TimeType,
    start: TimeInt,
    end: TimeInt,
    fps: Option<f32>,
) -> Option<f64> {
    let delta = end.as_i64().checked_sub(start.as_i64())? as f64;
    match time_type {
        TimeType::DurationNs | TimeType::TimestampNs => Some(delta / 1_000_000_000.0),
        TimeType::Sequence => Some(delta / f64::from(fps?)),
    }
}

fn sync_stream_playback(
    state: &mut AudioViewState,
    active_entry: Option<(&AudioStreamEntry, f64)>,
    play_state: PlayState,
    speed: f32,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if play_state == PlayState::Paused || speed <= 0.0 {
            state.playback = None;
            return;
        }

        let Some((entry, offset_secs)) = active_entry else {
            state.playback = None;
            return;
        };
        let key = AudioKey::from_blob(&entry.blob);
        let needs_restart = state.playback.as_ref().is_none_or(|playback| {
            let playing_offset = playback.timeline_offset_secs
                + playback.sink.get_pos().as_secs_f64() * f64::from(speed);
            playback.key != key || (playing_offset - offset_secs).abs() > 0.25
        });

        if needs_restart
            && let Err(err) = start_stream_playback(state, entry, key, offset_secs, speed)
        {
            state.playback = None;
            re_log::warn_once!("Failed to play audio stream: {err}");
        } else if let Some(playback) = &state.playback {
            playback.sink.set_speed(speed);
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = (state, active_entry, play_state, speed);
    }
}

fn playback_controls_ui(
    ui: &mut egui::Ui,
    state: &mut AudioViewState,
    entry: &AudioEntry,
    key: AudioKey,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if playback_finished(state) {
            state.playback = None;
        }

        let is_playing = state
            .playback
            .as_ref()
            .is_some_and(|playback| playback.key == key);

        let icon = if is_playing {
            &icons::PAUSE
        } else {
            &icons::PLAY
        };
        let tooltip = if is_playing {
            "Stop audio"
        } else {
            "Play audio"
        };
        let response = ui.small_icon_button(icon, tooltip).on_hover_text(tooltip);

        if response.clicked() {
            if is_playing {
                state.playback = None;
            } else if let Err(err) = start_playback(state, entry, key) {
                re_log::warn!("Failed to play audio asset: {err}");
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = (state, entry, key);
        ui.add_enabled_ui(false, |ui| {
            ui.small_icon_button(&icons::PLAY, "Play audio")
                .on_disabled_hover_text("Audio playback is only available in native builds.");
        });
    }
}

fn playback_progress_secs(state: &mut AudioViewState, key: AudioKey) -> Option<f32> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if playback_finished(state) {
            state.playback = None;
            return None;
        }

        state
            .playback
            .as_ref()
            .filter(|playback| playback.key == key)
            .map(|playback| playback.started_at.elapsed().as_secs_f32())
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = (state, key);
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn playback_finished(state: &AudioViewState) -> bool {
    state
        .playback
        .as_ref()
        .is_some_and(|playback| playback.sink.empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn start_playback(
    state: &mut AudioViewState,
    entry: &AudioEntry,
    key: AudioKey,
) -> anyhow::Result<()> {
    let stream = rodio::OutputStreamBuilder::open_default_stream()?;
    let sink = rodio::Sink::connect_new(stream.mixer());
    let source = rodio::Decoder::try_from(std::io::Cursor::new(entry.blob.0.as_ref().to_vec()))?;
    sink.append(source);

    state.playback = Some(Playback {
        key,
        _stream: stream,
        sink,
        started_at: Instant::now(),
        timeline_offset_secs: 0.0,
    });

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn start_stream_playback(
    state: &mut AudioViewState,
    entry: &AudioStreamEntry,
    key: AudioKey,
    offset_secs: f64,
    speed: f32,
) -> anyhow::Result<()> {
    use rodio::Source as _;

    let stream = rodio::OutputStreamBuilder::open_default_stream()?;
    let sink = rodio::Sink::connect_new(stream.mixer());
    let source = rodio::Decoder::try_from(std::io::Cursor::new(entry.blob.0.as_ref().to_vec()))?
        .skip_duration(std::time::Duration::from_secs_f64(offset_secs.max(0.0)));
    sink.append(source);
    sink.set_speed(speed);

    state.playback = Some(Playback {
        key,
        _stream: stream,
        sink,
        started_at: Instant::now(),
        timeline_offset_secs: offset_secs,
    });

    Ok(())
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| AudioView.help(ctx));
}
