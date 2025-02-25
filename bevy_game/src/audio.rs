//use crate::input::MyInput;
use crate::loading::AudioAssets;
use crate::AppState;
use bevy::prelude::*;
use bevy_kira_audio::{Audio, AudioPlugin, AudioControl};

pub struct InternalAudioPlugin;

// This plugin is responsible to control the game audio
impl Plugin for InternalAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(AudioPlugin)
            .add_system_set(SystemSet::on_enter(AppState::Playing).with_system(start_audio)
//            .add_system_set(SystemSet::on_update(GameState::Playing).with_system(control_flying_sound),
            );
    }
}

fn start_audio(audio_assets: Res<AudioAssets>, audio: Res<Audio>) {
    audio.set_volume(0.3);
    audio.play(audio_assets.flying.clone()).looped();
    audio.pause();
}

/* 
fn control_flying_sound(actions: Res<MyInput>, audio: Res<Audio>) {
    if actions.player_movement.is_some() {
        audio.resume();
    } else {
        audio.pause()
    }
}*/
