//! HUD・照準・武器ボタン・タイトル / ポーズ / 結果画面。

use bevy::{prelude::*, window::CursorOptions};

use crate::{
    GameState, MAX_PATIENCE, Session,
    salesman::{Repelled, Salesman},
    sfx::{PlaySfx, Sfx},
    wave::{WAVES, WaveState},
    weapon::{Aim, Arsenal, Weapon, WeaponButton},
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenTimer>()
            .add_systems(Startup, spawn_crosshair)
            .add_systems(OnEnter(GameState::Title), (spawn_title, despawn_hud))
            .add_systems(OnExit(GameState::Title), spawn_hud)
            .add_systems(OnEnter(GameState::Paused), spawn_pause)
            .add_systems(OnEnter(GameState::GameOver), spawn_result)
            .add_systems(OnEnter(GameState::Victory), spawn_result)
            .add_systems(
                Update,
                (click_through_screens, update_hud, update_crosshair, update_cursor, repel_feed),
            );
    }
}

const INK: Color = Color::srgb(0.97, 0.95, 0.9);
const ACCENT: Color = Color::srgb(1.0, 0.82, 0.25);

#[derive(Component)]
struct Hud;
#[derive(Component)]
struct PatienceFill;
#[derive(Component)]
struct PatienceText;
#[derive(Component)]
struct WaveText;
#[derive(Component)]
struct ScoreText;
#[derive(Component)]
struct Banner;
#[derive(Component)]
struct HurtOverlay;
#[derive(Component)]
struct CooldownShade(Weapon);
#[derive(Component)]
struct Crosshair;
#[derive(Component)]
struct Feed;
/// 撃退通知 (残り秒)。
#[derive(Component)]
struct FeedItem(f32);

/// 結果画面ですぐクリックが通らないようにするための待ち時間。
#[derive(Resource, Default)]
struct ScreenTimer(f32);

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont::from_font_size(size),
        TextColor(color),
        TextLayout::justify(Justify::Center),
        TextShadow { offset: Vec2::splat(2.0), color: Color::srgba(0.0, 0.0, 0.0, 0.6) },
    )
}

fn full_screen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(14.0),
        ..default()
    }
}

fn spawn_crosshair(mut commands: Commands) {
    commands
        .spawn((
            Crosshair,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(34.0),
                height: Val::Px(34.0),
                border: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor::all(Color::WHITE),
            GlobalZIndex(20),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(6.0),
                    height: Val::Px(6.0),
                    border_radius: BorderRadius::all(Val::Percent(50.0)),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
            ));
        });
}

fn update_crosshair(
    state: Res<State<GameState>>,
    aim: Res<Aim>,
    mut q: Query<(&mut Node, &mut Visibility, &mut BorderColor), With<Crosshair>>,
) {
    let Ok((mut node, mut vis, mut border)) = q.single_mut() else { return };
    *vis = if *state.get() == GameState::Playing { Visibility::Inherited } else { Visibility::Hidden };
    node.left = Val::Px(aim.screen.x - 17.0);
    node.top = Val::Px(aim.screen.y - 17.0);
    *border = BorderColor::all(if aim.on_enemy { Color::srgb(1.0, 0.3, 0.25) } else { Color::WHITE });
}

/// プレイ中だけシステムカーソルを隠す (照準を表示するため)。
fn update_cursor(state: Res<State<GameState>>, mut q: Query<&mut CursorOptions>) {
    let visible = *state.get() != GameState::Playing;
    for mut c in &mut q {
        if c.visible != visible {
            c.visible = visible;
        }
    }
}

fn spawn_title(mut commands: Commands, session: Res<Session>) {
    commands
        .spawn((
            DespawnOnExit(GameState::Title),
            full_screen(),
            BackgroundColor(Color::srgba(0.05, 0.07, 0.12, 0.45)),
        ))
        .with_children(|p| {
            p.spawn(text("しつこい不動産訪問営業を撃退せよ", 22.0, INK));
            p.spawn(text("営業おことわり！", 72.0, ACCENT));
            p.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    margin: UiRect::vertical(Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(14.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            ))
            .with_children(|p| {
                for line in [
                    "休日の午後。今日も営業マンが次々と玄関にやってくる…",
                    "玄関に居座られると忍耐ゲージが減り、0 になると契約させられてしまう！",
                    "マウスで狙ってクリックで投げる（スマホはタップ）",
                    "1 塩（連射）　2 水バケツ（範囲・押し返し）　3 お断りステッカー（貫通・強力）",
                    "Q / ホイールで切替　飛んでくる名刺は撃ち落とせる　Esc で一時停止",
                ] {
                    p.spawn(text(line, 17.0, INK));
                }
            });
            p.spawn(text("クリック / タップでスタート", 30.0, ACCENT));
            if session.hi_score > 0 {
                p.spawn(text(format!("ハイスコア {}", session.hi_score), 18.0, INK));
            }
        });
}

fn spawn_pause(mut commands: Commands) {
    commands
        .spawn((DespawnOnExit(GameState::Paused), full_screen(), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5))))
        .with_children(|p| {
            p.spawn(text("一時停止中", 54.0, ACCENT));
            p.spawn(text("Esc / P で再開　（インターホンは鳴りやまない…）", 20.0, INK));
        });
}

fn spawn_result(
    mut commands: Commands,
    state: Res<State<GameState>>,
    session: Res<Session>,
    mut timer: ResMut<ScreenTimer>,
) {
    timer.0 = 1.2;
    let won = *state.get() == GameState::Victory;
    let s = *state.get();
    commands
        .spawn((DespawnOnExit(s), full_screen(), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6))))
        .with_children(|p| {
            if won {
                p.spawn(text("平穏な休日を取り戻した！", 56.0, ACCENT));
                p.spawn(text("営業マンたちは二度と来なかった…たぶん。", 22.0, INK));
            } else {
                p.spawn(text("根負けして契約してしまった…", 50.0, Color::srgb(1.0, 0.45, 0.4)));
                p.spawn(text("気づけば手元には 35 年ローンの書類が。", 22.0, INK));
            }
            p.spawn(text(format!("撃退した営業マン　{} 人", session.repelled), 26.0, INK));
            p.spawn(text(format!("スコア　{}", session.score), 34.0, ACCENT));
            let hi = if session.score >= session.hi_score && session.score > 0 {
                "ハイスコア更新！".to_string()
            } else {
                format!("ハイスコア　{}", session.hi_score)
            };
            p.spawn(text(hi, 20.0, INK));
            p.spawn(text("クリック / タップでタイトルへ", 24.0, ACCENT));
        });
}

fn click_through_screens(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut timer: ResMut<ScreenTimer>,
    mut next: ResMut<NextState<GameState>>,
    mut virt: ResMut<Time<Virtual>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    timer.0 -= time.delta_secs();
    let clicked = mouse.just_pressed(MouseButton::Left)
        || touches.any_just_pressed()
        || keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space);
    if !clicked {
        return;
    }
    match state.get() {
        GameState::Title => {
            virt.unpause();
            sfx.write(PlaySfx(Sfx::Click));
            next.set(GameState::Playing);
        }
        GameState::GameOver | GameState::Victory if timer.0 <= 0.0 => {
            sfx.write(PlaySfx(Sfx::Click));
            next.set(GameState::Title);
        }
        _ => {}
    }
}

fn spawn_hud(mut commands: Commands) {
    // 被ダメージ時の赤い画面
    commands.spawn((
        Hud,
        HurtOverlay,
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::srgba(0.9, 0.1, 0.05, 0.0)),
        GlobalZIndex(-2),
    ));

    commands
        .spawn((
            Hud,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(14.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
        ))
        .with_children(|p| {
            // 上段: 忍耐ゲージ / ウェーブとスコア
            p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::FlexStart, ..default() })
                .with_children(|p| {
                    p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
                        .with_children(|p| {
                            p.spawn((PatienceText, text("忍耐ゲージ", 18.0, INK)));
                            p.spawn((
                                Node {
                                    width: Val::Px(260.0),
                                    height: Val::Px(18.0),
                                    border: UiRect::all(Val::Px(2.0)),
                                    border_radius: BorderRadius::all(Val::Px(9.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                                BorderColor::all(INK),
                            ))
                            .with_children(|p| {
                                p.spawn((
                                    PatienceFill,
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        border_radius: BorderRadius::all(Val::Px(7.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.3, 0.85, 0.4)),
                                ));
                            });
                        });
                    p.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, ..default() })
                        .with_children(|p| {
                            p.spawn((WaveText, text("", 20.0, INK)));
                            p.spawn((ScoreText, text("", 20.0, ACCENT)));
                        });
                });
            // 中央の見出しと撃退通知
            p.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(6.0), ..default() })
                .with_children(|p| {
                    p.spawn((Banner, text("", 44.0, ACCENT)));
                    p.spawn((Feed, Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }));
                });
            // 下段: 武器ボタン
            p.spawn(Node { justify_content: JustifyContent::Center, column_gap: Val::Px(10.0), ..default() })
                .with_children(|p| {
                    for (n, w) in Weapon::ALL.into_iter().enumerate() {
                        p.spawn((
                            WeaponButton(w),
                            Button,
                            Node {
                                width: Val::Px(128.0),
                                height: Val::Px(54.0),
                                border: UiRect::all(Val::Px(3.0)),
                                border_radius: BorderRadius::all(Val::Px(10.0)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.05, 0.08, 0.15, 0.7)),
                            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.35)),
                        ))
                        .with_children(|p| {
                            p.spawn((
                                CooldownShade(w),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(0.0),
                                    bottom: Val::Px(0.0),
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(0.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.25)),
                            ));
                            let label = match w {
                                Weapon::Sticker => "ステッカー",
                                _ => w.name(),
                            };
                            p.spawn(text(format!("{} {}", n + 1, label), 17.0, INK));
                        });
                    }
                });
        });
}

fn despawn_hud(mut commands: Commands, q: Query<Entity, With<Hud>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn update_hud(
    time: Res<Time<Real>>,
    mut session: ResMut<Session>,
    wave: Res<WaveState>,
    arsenal: Res<Arsenal>,
    salesmen: Query<&Salesman>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), (With<PatienceFill>, Without<CooldownShade>)>,
    mut texts: ParamSet<(
        Query<&mut Text, With<WaveText>>,
        Query<&mut Text, With<ScoreText>>,
        Query<&mut Text, With<Banner>>,
    )>,
    mut overlay: Query<&mut BackgroundColor, (With<HurtOverlay>, Without<PatienceFill>, Without<WeaponButton>, Without<CooldownShade>)>,
    mut shades: Query<(&mut Node, &CooldownShade), Without<PatienceFill>>,
    mut buttons: Query<(&WeaponButton, &mut BorderColor, &Interaction)>,
) {
    let dt = time.delta_secs();
    let el = time.elapsed_secs();
    let ratio = session.patience / MAX_PATIENCE;
    if let Ok((mut node, mut bg)) = fill.single_mut() {
        node.width = Val::Percent(ratio * 100.0);
        bg.0 = if ratio > 0.6 {
            Color::srgb(0.3, 0.85, 0.4)
        } else if ratio > 0.3 {
            Color::srgb(0.95, 0.8, 0.2)
        } else {
            Color::srgb(0.95, 0.3, 0.25)
        };
    }
    if let Ok(mut t) = texts.p0().single_mut() {
        t.0 = format!("WAVE {}/{}  {}", wave.index + 1, WAVES, crate::wave::wave_title(wave.index));
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        t.0 = format!("SCORE {}   HI {}", session.score, session.hi_score.max(session.score));
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        t.0 = wave.banner.as_ref().map(|(s, _)| s.clone()).unwrap_or_default();
    }

    // 玄関に居座られている間は画面の縁が赤く脈打つ
    let ringing = salesmen.iter().any(|s| s.is_ringing());
    let pulse = if ringing { ((el * 6.0).sin() * 0.5 + 0.5) * 0.12 } else { 0.0 };
    session.hurt_flash = (session.hurt_flash - dt * 1.2).max(0.0);
    if let Ok(mut bg) = overlay.single_mut() {
        bg.0 = Color::srgba(0.9, 0.1, 0.05, session.hurt_flash + pulse);
    }

    for (mut node, CooldownShade(w)) in &mut shades {
        let r = arsenal.cooldown[w.index()] / w.cooldown();
        node.height = Val::Percent(r.clamp(0.0, 1.0) * 100.0);
    }
    for (WeaponButton(w), mut border, interaction) in &mut buttons {
        let c = if *w == arsenal.current {
            ACCENT
        } else if *interaction == Interaction::Hovered {
            Color::srgba(1.0, 1.0, 1.0, 0.7)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.35)
        };
        *border = BorderColor::all(c);
    }
}

fn repel_feed(
    mut commands: Commands,
    time: Res<Time>,
    mut msgs: MessageReader<Repelled>,
    feed: Query<Entity, With<Feed>>,
    mut items: Query<(Entity, &mut FeedItem, &mut TextColor)>,
) {
    let Ok(feed) = feed.single() else {
        msgs.clear();
        return;
    };
    for Repelled(kind) in msgs.read() {
        let item = commands
            .spawn((FeedItem(1.6), text(format!("{}を撃退！ +{}", kind.label(), kind.score()), 22.0, INK)))
            .id();
        commands.entity(feed).add_child(item);
    }
    for (e, mut item, mut color) in &mut items {
        item.0 -= time.delta_secs();
        if item.0 <= 0.0 {
            commands.entity(e).despawn();
        } else {
            color.0 = INK.with_alpha(item.0.min(0.5) * 2.0);
        }
    }
}
