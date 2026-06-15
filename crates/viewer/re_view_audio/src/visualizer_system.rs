use re_chunk_store::LatestAtQuery;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::AssetAudio;
use re_sdk_types::components;
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

#[derive(Debug, Clone)]
pub struct AudioEntry {
    pub blob: components::Blob,
    pub media_type: components::MediaType,
}

#[derive(Default)]
pub struct AudioSystem;

impl IdentifiedViewSystem for AudioSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Audio"
        )
    }
}

impl VisualizerSystem for AudioSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<components::Blob>(
            &AssetAudio::descriptor_blob(),
            &AssetAudio::all_components(),
        )
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);
        let mut audio_entries = Vec::new();

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let results = data_result.latest_at_with_blueprint_resolved_data::<AssetAudio>(
                ctx,
                &timeline_query,
                Some(instruction),
            );

            let Some(blob) =
                results.get_mono::<components::Blob>(AssetAudio::descriptor_blob().component)
            else {
                continue;
            };

            let media_type = results
                .get_mono::<components::MediaType>(AssetAudio::descriptor_media_type().component)
                .or_else(|| components::MediaType::guess_from_data(blob.0.as_ref()))
                .unwrap_or_default();

            audio_entries.push(AudioEntry {
                blob: blob.clone(),
                media_type,
            });
        }

        Ok(VisualizerExecutionOutput::default().with_visualizer_data(audio_entries))
    }
}
