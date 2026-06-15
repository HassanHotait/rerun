use std::time::Instant;

use egui::Sense;
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, UiExt as _, icons};
use re_viewer_context::external::re_log_types::EntityPath;
use re_viewer_context::{
    IdentifiedViewSystem as _, Item, SystemCommand, SystemCommandSender as _, ViewClass,
    ViewClassRegistryError, ViewId, ViewQuery, ViewState, ViewStateExt as _,
    ViewSystemExecutionError, ViewerContext, suggest_view_for_each_entity,
};

use crate::visualizer_system::{AudioEntry, AudioSystem};
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
            "Shows logged audio assets as a waveform and plays them through the native audio output.",
        )
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<AudioSystem>()
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
        suggest_view_for_each_entity::<AudioSystem>(ctx, include_entity)
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

        let tokens = ui.tokens();
        let frame = egui::Frame::new().inner_margin(tokens.view_padding());
        let response = frame
            .show(ui, |ui| {
                let inner_ui_builder = egui::UiBuilder::new()
                    .layout(egui::Layout::top_down(egui::Align::LEFT))
                    .sense(Sense::click());
                ui.scope_builder(inner_ui_builder, |ui| {
                    audio_view_ui(ui, state, &audio_entries)
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
    ui: &mut egui::Ui,
    state: &mut AudioViewState,
    audio_entries: &[AudioEntry],
) -> Result<egui::Response, ViewSystemExecutionError> {
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
    });

    Ok(())
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| AudioView.help(ctx));
}
