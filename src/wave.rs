//! ウェーブ (営業の波) の進行。

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::{
    GameState, MAX_PATIENCE, Session,
    salesman::{Kind, Salesman, SpawnSalesman},
    sfx::{PlaySfx, Sfx},
    storage,
};

pub const WAVES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WavePhase {
    /// 「WAVE n」表示中
    Intro(f32),
    Active,
    /// クリア表示中
    Cleared(f32),
}

#[derive(Resource)]
pub struct WaveState {
    /// 0 始まり
    pub index: usize,
    pub phase: WavePhase,
    queue: VecDeque<(f32, Kind)>,
    timer: f32,
    /// 画面中央に出す見出し (文字列, 残り秒)
    pub banner: Option<(String, f32)>,
}

impl Default for WaveState {
    fn default() -> Self {
        Self { index: 0, phase: WavePhase::Intro(3.0), queue: VecDeque::new(), timer: 0.0, banner: None }
    }
}

pub fn wave_title(i: usize) -> &'static str {
    ["新人研修", "ベテラン登場", "チーム営業", "月末のノルマ", "支店長襲来"][i]
}

/// 各ウェーブの出現表 (前の出現からの待ち秒数, 種類)。
fn plan(i: usize) -> Vec<(f32, Kind)> {
    use Kind::*;
    let seq = |kinds: &[Kind], gap: f32| -> Vec<(f32, Kind)> {
        kinds.iter().enumerate().map(|(n, k)| (if n == 0 { 0.5 } else { gap }, *k)).collect()
    };
    match i {
        0 => seq(&[Rookie, Rookie, Rookie, Rookie, Rookie], 3.2),
        1 => seq(&[Rookie, Veteran, Rookie, Rookie, Veteran, Rookie, Veteran], 2.8),
        2 => seq(&[Team, Rookie, Veteran, Team, Rookie, Veteran, Team, Rookie], 2.6),
        3 => seq(
            &[Veteran, Rookie, Veteran, Team, Veteran, Rookie, Veteran, Team, Rookie, Veteran, Rookie, Veteran],
            1.9,
        ),
        _ => {
            let mut v = vec![(1.0, Boss)];
            v.extend(seq(&[Rookie, Veteran, Team, Rookie, Veteran, Team, Veteran, Rookie], 6.5));
            v
        }
    }
}

pub struct WavePlugin;

impl Plugin for WavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaveState>()
            .add_systems(OnExit(GameState::Title), start)
            .add_systems(Update, (run_waves, tick_banner).run_if(in_state(GameState::Playing)));
    }
}

fn start(mut wave: ResMut<WaveState>, mut session: ResMut<Session>) {
    *wave = WaveState::default();
    let hi = session.hi_score;
    *session = Session { hi_score: hi, ..default() };
    announce(&mut wave);
}

fn announce(wave: &mut WaveState) {
    wave.banner = Some((format!("WAVE {}  {}", wave.index + 1, wave_title(wave.index)), 2.6));
}

fn tick_banner(time: Res<Time>, mut wave: ResMut<WaveState>) {
    if let Some((_, t)) = &mut wave.banner {
        *t -= time.delta_secs();
        if *t <= 0.0 {
            wave.banner = None;
        }
    }
}

fn run_waves(
    time: Res<Time>,
    mut wave: ResMut<WaveState>,
    mut session: ResMut<Session>,
    salesmen: Query<&Salesman>,
    mut spawn: MessageWriter<SpawnSalesman>,
    mut sfx: MessageWriter<PlaySfx>,
    mut next: ResMut<NextState<GameState>>,
) {
    let dt = time.delta_secs();
    match wave.phase {
        WavePhase::Intro(t) => {
            if t == 3.0 {
                sfx.write(PlaySfx(Sfx::WaveStart));
            }
            let t = t - dt;
            if t <= 0.0 {
                wave.queue = plan(wave.index).into();
                wave.timer = 0.0;
                wave.phase = WavePhase::Active;
            } else {
                wave.phase = WavePhase::Intro(t);
            }
        }
        WavePhase::Active => {
            wave.timer -= dt;
            while wave.timer <= 0.0 {
                let Some((_, kind)) = wave.queue.pop_front() else { break };
                spawn.write(SpawnSalesman(kind));
                wave.timer += wave.queue.front().map_or(0.0, |(gap, _)| *gap);
            }
            // 全員出し切って少し経ち、玄関に向かう者がいなくなればクリア
            let active = salesmen.iter().filter(|s| !s.is_fleeing()).count();
            if wave.queue.is_empty() && wave.timer < -1.0 && active == 0 {
                session.patience = (session.patience + 20.0).min(MAX_PATIENCE);
                let last = wave.index + 1 == WAVES;
                wave.banner = Some((
                    if last { "全営業マン撃退！".into() } else { format!("WAVE {} クリア！  忍耐 +20", wave.index + 1) },
                    2.5,
                ));
                wave.phase = WavePhase::Cleared(2.8);
                sfx.write(PlaySfx(Sfx::Victory));
            }
        }
        WavePhase::Cleared(t) => {
            let t = t - dt;
            if t > 0.0 {
                wave.phase = WavePhase::Cleared(t);
                return;
            }
            if wave.index + 1 >= WAVES {
                session.score += (session.patience * 30.0) as u32;
                if session.score > session.hi_score {
                    session.hi_score = session.score;
                    storage::save_hi_score(session.score);
                }
                next.set(GameState::Victory);
            } else {
                wave.index += 1;
                wave.phase = WavePhase::Intro(3.0);
                announce(&mut wave);
            }
        }
    }
}
