//! 効果音。外部ファイルを使わず、起動時に波形を合成して WAV として登録する。

use std::{collections::HashMap, f32::consts::TAU, sync::Arc};

use bevy::{audio::Volume, prelude::*};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sfx {
    /// インターホン「ピンポーン」
    Chime,
    Throw,
    SaltHit,
    Splash,
    Sticker,
    /// 撃退 (営業マンが逃げ帰る)
    Repel,
    /// 名刺・パンフレットが当たった
    Ouch,
    WaveStart,
    GameOver,
    Victory,
    Click,
}

#[derive(Message, Clone, Copy)]
pub struct PlaySfx(pub Sfx);

#[derive(Resource, Default)]
struct SfxBank(HashMap<Sfx, Handle<AudioSource>>);

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySfx>()
            .init_resource::<SfxBank>()
            .add_systems(Startup, build_bank)
            .add_systems(Last, play_sfx);
    }
}

const RATE: u32 = 22_050;

fn build_bank(mut bank: ResMut<SfxBank>, mut sources: ResMut<Assets<AudioSource>>) {
    let table: [(Sfx, Vec<f32>); 11] = [
        (Sfx::Chime, chime()),
        (Sfx::Throw, throw()),
        (Sfx::SaltHit, salt_hit()),
        (Sfx::Splash, splash()),
        (Sfx::Sticker, sticker()),
        (Sfx::Repel, repel()),
        (Sfx::Ouch, ouch()),
        (Sfx::WaveStart, jingle(&[523.3, 659.3, 784.0, 1046.5], 0.11, 0.35)),
        (Sfx::GameOver, jingle(&[392.0, 370.0, 349.2, 261.6], 0.28, 0.9)),
        (Sfx::Victory, jingle(&[523.3, 659.3, 784.0, 659.3, 784.0, 1046.5], 0.12, 0.8)),
        (Sfx::Click, tone(900.0, 0.05, 0.35)),
    ];
    for (kind, samples) in table {
        let handle = sources.add(AudioSource {
            bytes: Arc::from(to_wav(&samples)),
        });
        bank.0.insert(kind, handle);
    }
}

fn play_sfx(mut commands: Commands, mut msgs: MessageReader<PlaySfx>, bank: Res<SfxBank>) {
    // 同じフレームで同じ音が何重にも鳴らないようにする
    let mut played = Vec::new();
    for PlaySfx(kind) in msgs.read() {
        if played.contains(kind) {
            continue;
        }
        played.push(*kind);
        let Some(handle) = bank.0.get(kind) else { continue };
        let vol = match kind {
            Sfx::Throw | Sfx::SaltHit => 0.35,
            Sfx::Chime => 0.45,
            _ => 0.6,
        };
        commands.spawn((
            AudioPlayer(handle.clone()),
            PlaybackSettings {
                volume: Volume::Linear(vol),
                ..PlaybackSettings::DESPAWN
            },
        ));
    }
}

// ---- 波形合成 ----

fn len(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

fn t(i: usize) -> f32 {
    i as f32 / RATE as f32
}

/// 簡易な擬似乱数ノイズ (決定的)。
fn noise(i: usize) -> f32 {
    let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn tone(freq: f32, secs: f32, amp: f32) -> Vec<f32> {
    (0..len(secs))
        .map(|i| {
            let tt = t(i);
            let env = (1.0 - tt / secs).powi(2) * (tt * 400.0).min(1.0);
            (tt * freq * TAU).sin() * env * amp
        })
        .collect()
}

fn chime() -> Vec<f32> {
    // ピン (高) → ポーン (低) の 2 音。倍音を少し足してベルっぽく
    let bell = |freq: f32, secs: f32| -> Vec<f32> {
        (0..len(secs))
            .map(|i| {
                let tt = t(i);
                let env = (-tt * 3.2).exp() * (tt * 300.0).min(1.0);
                ((tt * freq * TAU).sin() + 0.3 * (tt * freq * 2.0 * TAU).sin()) * env * 0.5
            })
            .collect()
    };
    let mut out = bell(784.0, 0.32);
    out.extend(bell(622.3, 0.9));
    out
}

fn throw() -> Vec<f32> {
    let secs = 0.14;
    let mut lp = 0.0;
    (0..len(secs))
        .map(|i| {
            let tt = t(i);
            let env = (tt / secs * std::f32::consts::PI).sin();
            lp += (noise(i) - lp) * (0.05 + 0.4 * tt / secs);
            lp * env * 1.2
        })
        .collect()
}

fn salt_hit() -> Vec<f32> {
    let secs = 0.08;
    (0..len(secs))
        .map(|i| {
            let tt = t(i);
            noise(i * 7) * (1.0 - tt / secs).powi(3) * 0.5
        })
        .collect()
}

fn splash() -> Vec<f32> {
    let secs = 0.55;
    let mut lp = 0.0;
    (0..len(secs))
        .map(|i| {
            let tt = t(i);
            let env = (-tt * 6.0).exp() * (tt * 200.0).min(1.0);
            lp += (noise(i * 3) - lp) * 0.25;
            (lp * 1.6 + noise(i) * 0.25 * (-tt * 20.0).exp()) * env
        })
        .collect()
}

fn sticker() -> Vec<f32> {
    let secs = 0.18;
    (0..len(secs))
        .map(|i| {
            let tt = t(i);
            let f = 180.0 - tt * 500.0;
            let env = (-tt * 25.0).exp();
            ((tt * f * TAU).sin() * 0.8 + noise(i) * 0.5) * env
        })
        .collect()
}

fn repel() -> Vec<f32> {
    // 「ぴゅ〜ん」と上がって逃げていく音
    let secs = 0.4;
    let mut phase = 0.0;
    (0..len(secs))
        .map(|i| {
            let tt = t(i);
            let f = 300.0 + tt * 2200.0;
            phase += f / RATE as f32;
            let env = (1.0 - tt / secs) * (tt * 300.0).min(1.0);
            let sq = if (phase * TAU).sin() > 0.0 { 1.0 } else { -1.0 };
            sq * env * 0.22
        })
        .collect()
}

fn ouch() -> Vec<f32> {
    let secs = 0.22;
    let mut phase = 0.0;
    (0..len(secs))
        .map(|i| {
            let tt = t(i);
            let f = 260.0 - tt * 500.0;
            phase += f / RATE as f32;
            let env = (1.0 - tt / secs).powi(2);
            ((phase * TAU).sin().signum() * 0.3 + noise(i) * 0.2) * env
        })
        .collect()
}

fn jingle(notes: &[f32], step: f32, last: f32) -> Vec<f32> {
    let mut out = Vec::new();
    for (n, &f) in notes.iter().enumerate() {
        let secs = if n + 1 == notes.len() { last } else { step };
        out.extend((0..len(secs)).map(|i| {
            let tt = t(i);
            let env = (1.0 - tt / secs).powf(1.5) * (tt * 300.0).min(1.0);
            let s = (tt * f * TAU).sin() + 0.35 * ((tt * f * TAU).sin()).signum();
            s * env * 0.3
        }));
    }
    out
}

fn to_wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}
