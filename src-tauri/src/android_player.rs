//! Android only: hands playback to the Kotlin PlayerPlugin (Media3 player).

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Emitter, Manager, Runtime};

use super::{
    ani_engine, data_dir, now_secs, persist, prepare_stream, store, PlaybackEnded, PlaybackStarted,
};

pub struct PlayerHandle<R: Runtime>(pub PluginHandle<R>);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NativePlayRequest {
    title: String,
    video_url: String,
    referer: String,
    audio_url: Option<String>,
    subtitle_url: Option<String>,
    start_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativePlayResult {
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    failed: bool,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    position_ms: u64,
    #[serde(default)]
    duration_ms: u64,
}

impl NativePlayResult {
    fn failure(message: String) -> Self {
        Self {
            completed: false,
            failed: true,
            message: Some(message),
            position_ms: 0,
            duration_ms: 0,
        }
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("player")
        .setup(|app, api| {
            let handle = api.register_android_plugin("com.gravityanime.app", "PlayerPlugin")?;
            app.manage(PlayerHandle(handle));
            Ok(())
        })
        .build()
}

#[allow(clippy::too_many_arguments)]
pub async fn play(
    app: tauri::AppHandle,
    anime_id: String,
    anime_title: String,
    anime_poster: Option<String>,
    episode: ani_engine::Episode,
    dub: bool,
    quality: String,
    from_start: Option<bool>,
) -> Result<PlaybackStarted, String> {
    let dir = data_dir(&app)?;
    let prepared = prepare_stream(&anime_id, &episode, dub, &quality).await?;
    let start_secs = if from_start.unwrap_or(false) {
        0
    } else {
        store::resume_point(&persist::load_history(&dir), &anime_id, &episode.id)
    };

    let request = NativePlayRequest {
        title: format!("{} Episode {}", anime_title, episode.number),
        video_url: prepared.video_url.clone(),
        referer: prepared.referer.clone(),
        audio_url: prepared.audio_url.clone(),
        subtitle_url: prepared.subtitle_url.clone(),
        start_ms: start_secs * 1000,
    };

    let _ = persist::update_history(&dir, |entries| {
        store::record_started(
            entries,
            store::HistoryEntry {
                anime_id: anime_id.clone(),
                anime_title: anime_title.clone(),
                poster: anime_poster.clone(),
                episode_id: episode.id.clone(),
                episode_number: episode.number.clone(),
                completed: false,
                position_secs: start_secs,
                duration_secs: 0,
                updated_at: now_secs(),
            },
        )
    });

    let episode_id = episode.id.clone();
    let episode_number = episode.number.clone();
    let ended_anime_id = anime_id.clone();
    let task_episode_id = episode.id.clone();
    let task_episode_number = episode.number.clone();
    let task_quality = quality.clone();
    let history_dir = dir.clone();
    let task_app = app.clone();

    tauri::async_runtime::spawn(async move {
        let mut request = request;
        let mut retried = false;
        let outcome = loop {
            let blocking_app = task_app.clone();
            let req = request.clone();
            let run = tauri::async_runtime::spawn_blocking(move || {
                let handle = blocking_app.state::<PlayerHandle<tauri::Wry>>();
                handle.0.run_mobile_plugin::<NativePlayResult>("play", req)
            })
            .await;
            let result = match run {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => NativePlayResult::failure(e.to_string()),
                Err(e) => NativePlayResult::failure(e.to_string()),
            };

            if result.failed && !retried {
                retried = true;
                eprintln!("[play] the stream failed; retrying once with a fresh link");
                if result.position_ms >= 5_000 {
                    request.start_ms = result.position_ms;
                }
                if let Ok(fresh) =
                    prepare_stream(&ended_anime_id, &episode, dub, &task_quality).await
                {
                    request.video_url = fresh.video_url;
                    request.referer = fresh.referer;
                    request.audio_url = fresh.audio_url;
                    request.subtitle_url = fresh.subtitle_url;
                    continue;
                }
            }
            break result;
        };

        let failed = outcome.failed;
        let completed = outcome.completed && !failed;
        if completed {
            let _ = persist::update_history(&history_dir, |entries| {
                store::mark_completed(entries, &ended_anime_id, &task_episode_id, now_secs());
            });
        } else if outcome.position_ms >= 5_000 {
            let _ = persist::update_history(&history_dir, |entries| {
                store::record_progress(
                    entries,
                    &ended_anime_id,
                    &task_episode_id,
                    outcome.position_ms / 1000,
                    outcome.duration_ms / 1000,
                    now_secs(),
                );
            });
        }
        let message = if failed {
            format!(
                "The player could not play episode {task_episode_number}: {}",
                outcome
                    .message
                    .unwrap_or_else(|| "unknown error".to_string())
            )
        } else if completed {
            format!("Finished episode {task_episode_number}.")
        } else {
            format!("Player closed during episode {task_episode_number}.")
        };
        let _ = task_app.emit(
            "playback-ended",
            PlaybackEnded {
                anime_id: ended_anime_id,
                episode_id: task_episode_id,
                exit_code: None,
                failed,
                completed,
                message,
            },
        );
    });

    let resume_note = if start_secs > 0 {
        format!(" Resuming from {}:{:02}.", start_secs / 60, start_secs % 60)
    } else {
        String::new()
    };
    let wanted = if quality.trim().is_empty() {
        "best"
    } else {
        quality.trim()
    };
    let mut message = format!(
        "Playing episode {} at {} in the built-in player.{}",
        episode_number, prepared.quality, resume_note
    );
    if prepared.fell_back {
        message = format!(
            "{wanted} isn't available for this episode, so the best quality was used. {message}"
        );
    }
    Ok(PlaybackStarted {
        episode_id,
        player: "Built-in player".to_string(),
        quality: prepared.quality,
        has_subtitles: prepared.subtitle_url.is_some(),
        has_separate_audio: prepared.audio_url.is_some(),
        message,
    })
}