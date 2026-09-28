//! 営業おことわり！ — しつこい不動産訪問営業を玄関先で撃退する 3D ゲーム (Bevy + WebAssembly)。

// Bevy のシステム関数は引数・クエリ型が長くなりがちなので許容する
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod salesman;
mod scene;
mod sfx;
mod storage;
mod ui;
mod wave;
mod weapon;

use bevy::{
    asset::AssetId,
    prelude::*,
    window::{CursorOptions, WindowResolution},
};

/// 日本語表示用フォント (Noto Sans CJK JP をゲーム内の文字だけにサブセット化したもの)。
const FONT_DATA: &[u8] = include_bytes!("../assets/fonts/game-font.otf");

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Title,
    Playing,
    Paused,
    GameOver,
    Victory,
}

/// 1 プレイ分の進行状況。
#[derive(Resource)]
pub struct Session {
    /// 忍耐ゲージ (0 で根負けして契約 = ゲームオーバー)。
    pub patience: f32,
    pub score: u32,
    pub hi_score: u32,
    /// 撃退した営業マンの数。
    pub repelled: u32,
    /// 被ダメージ時の画面の赤フラッシュ強度。
    pub hurt_flash: f32,
    /// カメラの揺れ。
    pub shake: f32,
}

pub const MAX_PATIENCE: f32 = 100.0;

impl Default for Session {
    fn default() -> Self {
        Self {
            patience: MAX_PATIENCE,
            score: 0,
            hi_score: storage::load_hi_score(),
            repelled: 0,
            hurt_flash: 0.0,
            shake: 0.0,
        }
    }
}

impl Session {
    pub fn hurt(&mut self, amount: f32) {
        self.patience = (self.patience - amount).max(0.0);
        self.hurt_flash = (self.hurt_flash + amount * 0.06).min(0.6);
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "営業おことわり！".into(),
                    canvas: Some("#game".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                }),
                primary_cursor_options: Some(CursorOptions {
                    visible: false,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            }),
    );

    // 既定フォントを日本語フォントに差し替える (全 Text がこれを使う)
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(AssetId::default(), Font::from_bytes(FONT_DATA.to_vec()))
        .expect("フォント登録に失敗");

    app.init_state::<GameState>()
        .init_resource::<Session>()
        .add_plugins((
            scene::ScenePlugin,
            sfx::SfxPlugin,
            weapon::WeaponPlugin,
            salesman::SalesmanPlugin,
            wave::WavePlugin,
            ui::UiPlugin,
        ))
        .add_systems(Update, (toggle_pause, check_game_over));
    app.run();
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut time: ResMut<Time<Virtual>>,
) {
    if !(keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyP)) {
        return;
    }
    match state.get() {
        GameState::Playing => {
            time.pause();
            next.set(GameState::Paused);
        }
        GameState::Paused => {
            time.unpause();
            next.set(GameState::Playing);
        }
        _ => {}
    }
}

fn check_game_over(
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut session: ResMut<Session>,
    mut sfx: MessageWriter<sfx::PlaySfx>,
) {
    if *state.get() == GameState::Playing && session.patience <= 0.0 {
        if session.score > session.hi_score {
            session.hi_score = session.score;
            storage::save_hi_score(session.score);
        }
        sfx.write(sfx::PlaySfx(sfx::Sfx::GameOver));
        next.set(GameState::GameOver);
    }
}
