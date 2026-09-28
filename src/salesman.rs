//! しつこい不動産訪問営業マン: 見た目・移動 AI・被弾・名刺投げ・頭上の吹き出しと HP バー。

use bevy::prelude::*;
use rand::{Rng as _, seq::IndexedRandom};

use crate::{
    GameState, Session,
    scene::{DOOR_Z, EYE, GATE_HALF, GATE_Z, MainCamera, YARD_HALF, ground_y},
    sfx::{PlaySfx, Sfx},
    weapon::{GameRng, Weapon},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 新人営業: 遅くて打たれ弱い
    Rookie,
    /// ベテラン営業: ジグザグに近づき名刺を投げる
    Veteran,
    /// 新人チーム (2〜3 人組で来る)
    Team,
    /// 支店長: 最終ボス
    Boss,
}

struct Stats {
    hp: f32,
    speed: f32,
    scale: f32,
    score: u32,
    /// 玄関に居座ったときに毎秒削る忍耐
    drain: f32,
    suit: Color,
    tie: Color,
}

impl Kind {
    fn stats(self) -> Stats {
        match self {
            Kind::Rookie => Stats {
                hp: 40.0, speed: 1.7, scale: 1.0, score: 100, drain: 5.0,
                suit: Color::srgb(0.15, 0.18, 0.3), tie: Color::srgb(0.75, 0.12, 0.12),
            },
            Kind::Veteran => Stats {
                hp: 75.0, speed: 2.1, scale: 1.02, score: 200, drain: 7.0,
                suit: Color::srgb(0.12, 0.12, 0.14), tie: Color::srgb(0.2, 0.3, 0.7),
            },
            Kind::Team => Stats {
                hp: 45.0, speed: 2.0, scale: 0.97, score: 120, drain: 5.0,
                suit: Color::srgb(0.2, 0.25, 0.22), tie: Color::srgb(0.9, 0.55, 0.1),
            },
            Kind::Boss => Stats {
                hp: 900.0, speed: 1.15, scale: 1.4, score: 3000, drain: 14.0,
                suit: Color::srgb(0.42, 0.42, 0.45), tie: Color::srgb(0.9, 0.75, 0.15),
            },
        }
    }

    pub fn score(self) -> u32 {
        self.stats().score
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Rookie => "新人営業",
            Kind::Veteran => "ベテラン営業",
            Kind::Team => "営業チーム",
            Kind::Boss => "支店長",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// 道路を歩いて門へ
    ToGate,
    /// 前庭を横切って玄関へ
    ToDoor,
    /// インターホンを連打中
    Ringing,
    /// 撃退されて逃げ帰る
    Fleeing,
}

#[derive(Component)]
pub struct Salesman {
    pub kind: Kind,
    pub hp: f32,
    pub max_hp: f32,
    phase: Phase,
    speed: f32,
    scale: f32,
    /// 玄関前で立つ位置の x
    lane_x: f32,
    seed: f32,
    push: Vec3,
    stun: f32,
    flinch: f32,
    walk: f32,
    age: f32,
    throw_cd: f32,
    chime_cd: f32,
    talk_cd: f32,
    talk_left: f32,
    flee_dir: f32,
    parts: Parts,
    overhead: Overhead,
}

impl Salesman {
    pub fn is_fleeing(&self) -> bool {
        self.phase == Phase::Fleeing
    }
    pub fn radius(&self) -> f32 {
        0.36 * self.scale
    }
    pub fn height(&self) -> f32 {
        1.8 * self.scale
    }
    pub fn is_ringing(&self) -> bool {
        self.phase == Phase::Ringing
    }
}

/// アニメーションさせる関節。
#[derive(Clone, Copy)]
struct Parts {
    upper: Entity,
    legs: [Entity; 2],
    arms: [Entity; 2],
}

/// 頭上の UI (吹き出しと HP バー)。
#[derive(Clone, Copy)]
struct Overhead {
    root: Entity,
    bubble: Entity,
    text: Entity,
    bar: Entity,
    fill: Entity,
}

/// 営業マンが投げてくる名刺・パンフレット。
#[derive(Component)]
pub struct Shot {
    vel: Vec3,
    damage: f32,
    life: f32,
}

/// 営業マンへの攻撃。
#[derive(Message, Clone, Copy)]
pub struct Hit {
    pub target: Entity,
    pub damage: f32,
    pub knock: Vec3,
    pub stun: f32,
    pub weapon: Weapon,
}

/// 営業マンを出現させる要求 (wave から送る)。
#[derive(Message, Clone, Copy)]
pub struct SpawnSalesman(pub Kind);

/// 撃退時に wave / UI へ知らせる。
#[derive(Message, Clone, Copy)]
pub struct Repelled(pub Kind);

#[derive(Resource)]
struct Looks {
    capsule_leg: Handle<Mesh>,
    capsule_arm: Handle<Mesh>,
    torso: Handle<Mesh>,
    head: Handle<Mesh>,
    shoe: Handle<Mesh>,
    strip: Handle<Mesh>,
    tie: Handle<Mesh>,
    case: Handle<Mesh>,
    glasses: Handle<Mesh>,
    card: Handle<Mesh>,
    pamphlet: Handle<Mesh>,
    belly: Handle<Mesh>,
    skin: Handle<StandardMaterial>,
    hair: Handle<StandardMaterial>,
    shirt: Handle<StandardMaterial>,
    black: Handle<StandardMaterial>,
    leather: Handle<StandardMaterial>,
    lanyard: Handle<StandardMaterial>,
    card_mat: Handle<StandardMaterial>,
    pamphlet_mat: Handle<StandardMaterial>,
    suits: Vec<(Kind, Handle<StandardMaterial>, Handle<StandardMaterial>)>,
}

pub struct SalesmanPlugin;

impl Plugin for SalesmanPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Hit>()
            .add_message::<SpawnSalesman>()
            .add_message::<Repelled>()
            .add_systems(Startup, setup_looks)
            .add_systems(
                Update,
                (spawn, apply_hits, think, separate, animate, throw_shots, move_shots)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(PostUpdate, place_overheads.after(TransformSystems::Propagate))
            .add_systems(OnEnter(GameState::Title), cleanup);
    }
}

const APPROACH: &[&str] = &[
    "ごめんくださ〜い！",
    "不動産のご案内です！",
    "5分だけお時間を！",
    "近所で売却のご相談が多くて…",
    "今だけの特別なご提案が！",
    "お宅、今いくらで売れるかご存知ですか？",
];
const RINGING: &[&str] = &[
    "お話だけでも！",
    "いらっしゃいますよね？",
    "ピンポーン♪",
    "査定は無料です！",
    "開けてくださ〜い！",
    "資産運用、ご興味ありませんか？",
    "お留守ですか〜？",
];
const BOSS: &[&str] = &[
    "支店長の私が直々に参りました",
    "この物件、今が売り時ですぞ！",
    "契約書はこちらに！",
    "部下がお世話になりましたな",
];
const FLEE: &[&str] = &[
    "また来ます！",
    "失礼しました〜！",
    "上司に怒られる〜！",
    "次は負けませんよ！",
    "パンフだけでも〜！",
];

fn setup_looks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mut m = |c: Color, rough: f32| {
        mats.add(StandardMaterial { base_color: c, perceptual_roughness: rough, ..default() })
    };
    let skin = m(Color::srgb(0.96, 0.8, 0.66), 0.8);
    let hair = m(Color::srgb(0.08, 0.07, 0.07), 0.6);
    let shirt = m(Color::srgb(0.96, 0.96, 0.96), 0.7);
    let black = m(Color::srgb(0.05, 0.05, 0.05), 0.35);
    let leather = m(Color::srgb(0.35, 0.2, 0.1), 0.5);
    let lanyard = m(Color::srgb(0.2, 0.55, 0.9), 0.6);
    let card_mat = m(Color::srgb(0.98, 0.98, 0.95), 0.6);
    let pamphlet_mat = m(Color::srgb(0.95, 0.4, 0.2), 0.6);
    let suits = [Kind::Rookie, Kind::Veteran, Kind::Team, Kind::Boss]
        .into_iter()
        .map(|k| {
            let s = k.stats();
            (k, m(s.suit, 0.75), m(s.tie, 0.5))
        })
        .collect();
    commands.insert_resource(Looks {
        capsule_leg: meshes.add(Capsule3d::new(0.1, 0.62)),
        capsule_arm: meshes.add(Capsule3d::new(0.075, 0.48)),
        torso: meshes.add(Cuboid::new(0.5, 0.62, 0.28)),
        head: meshes.add(Sphere::new(0.16).mesh().ico(3).unwrap()),
        shoe: meshes.add(Cuboid::new(0.13, 0.08, 0.28)),
        strip: meshes.add(Cuboid::new(0.14, 0.5, 0.02)),
        tie: meshes.add(Cuboid::new(0.06, 0.4, 0.02)),
        case: meshes.add(Cuboid::new(0.1, 0.28, 0.4)),
        glasses: meshes.add(Cuboid::new(0.26, 0.05, 0.03)),
        card: meshes.add(Cuboid::new(0.22, 0.01, 0.13)),
        pamphlet: meshes.add(Cuboid::new(0.4, 0.02, 0.3)),
        belly: meshes.add(Sphere::new(0.26).mesh().ico(2).unwrap()),
        skin, hair, shirt, black, leather, lanyard, card_mat, pamphlet_mat,
        suits,
    });
}

fn spawn(
    mut commands: Commands,
    mut reqs: MessageReader<SpawnSalesman>,
    looks: Res<Looks>,
    mut rng: ResMut<GameRng>,
) {
    for SpawnSalesman(kind) in reqs.read() {
        let rng = &mut rng.0;
        let n = if *kind == Kind::Team { rng.random_range(2..=3) } else { 1 };
        let side = if rng.random_bool(0.5) { -1.0 } else { 1.0 };
        let base_x = side * rng.random_range(10.0..18.0);
        for i in 0..n {
            let pos = Vec3::new(base_x + side * i as f32 * 1.1, 0.0, GATE_Z - rng.random_range(3.5..6.5));
            spawn_one(&mut commands, &looks, rng, *kind, pos);
        }
    }
}

fn spawn_one(commands: &mut Commands, looks: &Looks, rng: &mut rand::rngs::SmallRng, kind: Kind, pos: Vec3) {
    let st = kind.stats();
    let (_, suit, tie) = looks.suits.iter().find(|(k, _, _)| *k == kind).unwrap();
    let mesh = |h: &Handle<Mesh>, mat: &Handle<StandardMaterial>, t: Transform| {
        (Mesh3d(h.clone()), MeshMaterial3d(mat.clone()), t)
    };

    // 脚 (腰を軸に振る)
    let legs = [-1.0f32, 1.0].map(|s| {
        commands
            .spawn((Transform::from_xyz(s * 0.12, 0.9, 0.0), Visibility::default()))
            .with_children(|p| {
                p.spawn(mesh(&looks.capsule_leg, suit, Transform::from_xyz(0.0, -0.42, 0.0)));
                p.spawn(mesh(&looks.shoe, &looks.black, Transform::from_xyz(0.0, -0.86, 0.05)));
            })
            .id()
    });
    // 腕 (肩を軸に振る。位置は上半身基準)。右手にカバン
    let arms = [-1.0f32, 1.0].map(|s| {
        commands
            .spawn((Transform::from_xyz(s * 0.31, 0.58, 0.0), Visibility::default()))
            .with_children(|p| {
                p.spawn(mesh(&looks.capsule_arm, suit, Transform::from_xyz(0.0, -0.3, 0.0)));
                p.spawn(mesh(&looks.head, &looks.skin, Transform::from_xyz(0.0, -0.6, 0.0).with_scale(Vec3::splat(0.4))));
                if s > 0.0 {
                    p.spawn(mesh(&looks.case, &looks.leather, Transform::from_xyz(0.02, -0.78, 0.0)));
                }
            })
            .id()
    });
    // 上半身 (腰を軸に傾ける)
    let upper = commands
        .spawn((Transform::from_xyz(0.0, 0.9, 0.0), Visibility::default()))
        .with_children(|p| {
            p.spawn(mesh(&looks.torso, suit, Transform::from_xyz(0.0, 0.32, 0.0)));
            p.spawn(mesh(&looks.strip, &looks.shirt, Transform::from_xyz(0.0, 0.36, 0.141)));
            p.spawn(mesh(&looks.tie, tie, Transform::from_xyz(0.0, 0.34, 0.153)));
            p.spawn(mesh(&looks.head, &looks.skin, Transform::from_xyz(0.0, 0.8, 0.0)));
            for x in [-0.055f32, 0.055] {
                p.spawn(mesh(&looks.head, &looks.black, Transform::from_xyz(x, 0.81, 0.145).with_scale(Vec3::splat(0.14))));
            }
            match kind {
                Kind::Boss => {
                    // 支店長: 薄い頭頂・ヒゲ・お腹
                    p.spawn(mesh(&looks.head, &looks.hair, Transform::from_xyz(0.0, 0.78, -0.06).with_scale(Vec3::new(1.05, 0.55, 0.9))));
                    p.spawn(mesh(&looks.glasses, &looks.hair, Transform::from_xyz(0.0, 0.74, 0.15).with_scale(Vec3::new(0.6, 0.9, 1.0))));
                    p.spawn(mesh(&looks.belly, suit, Transform::from_xyz(0.0, 0.18, 0.1).with_scale(Vec3::new(1.0, 1.0, 0.8))));
                }
                _ => {
                    p.spawn(mesh(&looks.head, &looks.hair, Transform::from_xyz(0.0, 0.9, -0.035).with_scale(Vec3::new(1.06, 0.55, 1.0))));
                }
            }
            if kind == Kind::Veteran {
                p.spawn(mesh(&looks.glasses, &looks.black, Transform::from_xyz(0.0, 0.82, 0.15)));
            }
            if kind == Kind::Team {
                p.spawn(mesh(&looks.strip, &looks.lanyard, Transform::from_xyz(0.1, 0.35, 0.16).with_scale(Vec3::new(0.35, 0.6, 1.0))));
            }
        })
        .id();

    let overhead = spawn_overhead(commands, kind);

    let mut root = commands.spawn((
        Transform::from_translation(pos).with_scale(Vec3::splat(st.scale)),
        Visibility::default(),
    ));
    root.add_children(&[upper, legs[0], legs[1]]);
    let root_id = root.id();
    // 腕は上半身の子にして、お辞儀やのけぞりに追従させる
    commands.entity(upper).add_children(&arms);

    let lane_x = rng.random_range(-2.1..2.1);
    commands.entity(root_id).insert(Salesman {
        kind,
        hp: st.hp,
        max_hp: st.hp,
        phase: Phase::ToGate,
        speed: st.speed * rng.random_range(0.9..1.1),
        scale: st.scale,
        lane_x,
        seed: rng.random_range(0.0..100.0),
        push: Vec3::ZERO,
        stun: 0.0,
        flinch: 0.0,
        walk: 0.0,
        age: 0.0,
        throw_cd: rng.random_range(2.0..4.0),
        chime_cd: 0.3,
        talk_cd: rng.random_range(0.5..2.5),
        talk_left: 0.0,
        flee_dir: if pos.x < 0.0 { -1.0 } else { 1.0 },
        parts: Parts { upper, legs, arms },
        overhead,
    });
}

fn spawn_overhead(commands: &mut Commands, kind: Kind) -> Overhead {
    let text = commands
        .spawn((
            Text::new(""),
            TextFont::from_font_size(15.0),
            TextColor(Color::srgb(0.1, 0.1, 0.12)),
            TextLayout::justify(Justify::Center),
        ))
        .id();
    let bubble = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                max_width: Val::Px(220.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.95)),
            BorderColor::all(Color::srgb(0.2, 0.2, 0.25)),
            Visibility::Hidden,
        ))
        .add_child(text)
        .id();
    let fill = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(if kind == Kind::Boss {
                Color::srgb(0.95, 0.3, 0.2)
            } else {
                Color::srgb(0.95, 0.65, 0.2)
            }),
        ))
        .id();
    let bar_w = if kind == Kind::Boss { 120.0 } else { 56.0 };
    let bar = commands
        .spawn((
            Node {
                width: Val::Px(bar_w),
                height: Val::Px(6.0),
                margin: UiRect::top(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.8)),
        ))
        .add_child(fill)
        .id();
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(240.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(-1),
        ))
        .add_children(&[bubble, bar])
        .id();
    Overhead { root, bubble, text, bar, fill }
}

fn say(commands: &mut Commands, s: &mut Salesman, line: &str, secs: f32) {
    commands.entity(s.overhead.text).insert(Text::new(line));
    commands.entity(s.overhead.bubble).insert(Visibility::Inherited);
    s.talk_left = secs;
}

fn apply_hits(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    mut q: Query<&mut Salesman>,
    mut session: ResMut<Session>,
    mut repelled: MessageWriter<Repelled>,
    mut sfx: MessageWriter<PlaySfx>,
    mut rng: ResMut<GameRng>,
) {
    for hit in hits.read() {
        let Ok(mut s) = q.get_mut(hit.target) else { continue };
        if s.is_fleeing() {
            continue;
        }
        s.hp -= hit.damage;
        let resist = if s.kind == Kind::Boss { 0.3 } else { 1.0 };
        s.push += hit.knock * resist;
        s.stun = s.stun.max(hit.stun * resist);
        s.flinch = 1.0;
        if s.phase == Phase::Ringing && hit.knock.length_squared() > 0.1 {
            s.phase = Phase::ToDoor;
        }
        if s.hp <= 0.0 {
            s.phase = Phase::Fleeing;
            s.stun = 0.25;
            let line = if s.kind == Kind::Boss {
                "…お、覚えておきなさい！"
            } else {
                FLEE.choose(&mut rng.0).unwrap()
            };
            say(&mut commands, &mut s, line, 2.5);
            commands.entity(s.overhead.bar).insert(Visibility::Hidden);
            session.score += s.kind.score();
            session.repelled += 1;
            repelled.write(Repelled(s.kind));
            sfx.write(PlaySfx(Sfx::Repel));
        } else if s.talk_left <= 0.0 || rng.0.random_bool(0.3) {
            let line = match hit.weapon {
                Weapon::Salt => *["しょっぱい！", "塩!?", "うわっ！"].choose(&mut rng.0).unwrap(),
                Weapon::Water => *["冷たっ！", "スーツが〜！", "ひぃ！"].choose(&mut rng.0).unwrap(),
                Weapon::Sticker => "お断り…だと…!?",
            };
            say(&mut commands, &mut s, line, 0.9);
        }
    }
}

/// 移動 AI。
fn think(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Salesman)>,
    mut session: ResMut<Session>,
    mut rng: ResMut<GameRng>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut s) in &mut q {
        s.age += dt;
        s.flinch = (s.flinch - dt * 4.0).max(0.0);
        s.talk_left -= dt;
        if s.talk_left <= 0.0 {
            commands.entity(s.overhead.bubble).insert(Visibility::Hidden);
        }

        // ノックバック
        let push = s.push;
        tf.translation += push * dt * 3.0;
        s.push *= (1.0 - dt * 3.0).max(0.0);

        let enraged = s.kind == Kind::Boss && s.hp < s.max_hp * 0.5;
        let speed = if enraged { s.speed * 1.6 } else { s.speed };
        let pos = tf.translation;
        let mut target: Option<(Vec3, f32)> = None;

        match s.phase {
            Phase::ToGate => {
                let gate = Vec3::new((s.lane_x * 0.3).clamp(-0.8, 0.8), 0.0, GATE_Z + 0.8);
                if Vec2::new(pos.x - gate.x, pos.z - gate.z).length() < 0.6 || (pos.z > GATE_Z + 0.3 && pos.x.abs() < GATE_HALF) {
                    s.phase = Phase::ToDoor;
                } else if pos.z < GATE_Z - 1.2 && pos.x.abs() > 1.0 {
                    // まず道路を門の正面まで歩く
                    target = Some((Vec3::new(gate.x, 0.0, GATE_Z - 1.8), speed * 1.2));
                } else {
                    target = Some((gate, speed));
                }
            }
            Phase::ToDoor => {
                if pos.z < GATE_Z {
                    s.phase = Phase::ToGate;
                }
                let door = Vec3::new(s.lane_x, 0.0, DOOR_Z);
                let progress = ((pos.z - GATE_Z) / (DOOR_Z - GATE_Z)).clamp(0.0, 1.0);
                let mut goal = door;
                if matches!(s.kind, Kind::Veteran | Kind::Boss) {
                    // ジグザグに近づいて狙いを外させる
                    let amp = if s.kind == Kind::Boss { 2.5 } else { 4.0 };
                    goal.x += (s.age * 1.3 + s.seed).sin() * amp * (1.0 - progress);
                    goal.z = (pos.z + 3.0).min(DOOR_Z);
                }
                if Vec2::new(pos.x - door.x, pos.z - door.z).length() < 0.35 {
                    s.phase = Phase::Ringing;
                    s.chime_cd = 0.2;
                    s.talk_cd = 0.4;
                } else {
                    target = Some((goal, speed));
                }
            }
            Phase::Ringing => {
                session.hurt(s.kind.stats().drain * dt);
                s.chime_cd -= dt;
                if s.chime_cd <= 0.0 {
                    s.chime_cd = if s.kind == Kind::Boss { 1.2 } else { 2.2 };
                    sfx.write(PlaySfx(Sfx::Chime));
                    session.shake = session.shake.max(0.25);
                }
            }
            Phase::Fleeing => {
                let dest = if pos.z > GATE_Z + 0.5 {
                    Vec3::new(0.0, 0.0, GATE_Z - 2.0)
                } else {
                    Vec3::new(s.flee_dir * 45.0, 0.0, GATE_Z - 3.0)
                };
                target = Some((dest, 7.5));
                if pos.x.abs() > 40.0 {
                    despawn_salesman(&mut commands, e, &s);
                    continue;
                }
            }
        }

        // おしゃべり
        s.talk_cd -= dt;
        if s.talk_cd <= 0.0 && s.phase != Phase::Fleeing {
            let lines = if s.kind == Kind::Boss {
                BOSS
            } else if s.phase == Phase::Ringing {
                RINGING
            } else {
                APPROACH
            };
            let line = *lines.choose(&mut rng.0).unwrap();
            say(&mut commands, &mut s, line, 2.0);
            s.talk_cd = rng.0.random_range(3.0..6.0);
        }

        let mut moving = false;
        if s.stun > 0.0 {
            s.stun -= dt;
        } else if let Some((goal, spd)) = target {
            let d = Vec3::new(goal.x - pos.x, 0.0, goal.z - pos.z);
            if d.length() > 0.05 {
                let dir = d.normalize();
                tf.translation += dir * spd * dt;
                let yaw = dir.x.atan2(dir.z);
                let want = Quat::from_rotation_y(yaw);
                tf.rotation = tf.rotation.slerp(want, (dt * 8.0).min(1.0));
                s.walk += dt * spd * 3.2;
                moving = true;
            }
        }
        if !moving && s.phase == Phase::Ringing {
            // 玄関 (プレイヤー) の方を向く
            let d = EYE - tf.translation;
            let want = Quat::from_rotation_y(d.x.atan2(d.z));
            tf.rotation = tf.rotation.slerp(want, (dt * 6.0).min(1.0));
        }

        // 塀の中では塀を通り抜けない・家に入らない
        let p = &mut tf.translation;
        if p.z > GATE_Z - 0.3 {
            p.x = p.x.clamp(-YARD_HALF + 0.5, YARD_HALF - 0.5);
            if p.z < GATE_Z + 0.4 && p.x.abs() > GATE_HALF - 0.3 && s.phase != Phase::ToGate {
                p.z = GATE_Z + 0.4;
            }
        }
        p.z = p.z.min(DOOR_Z + 0.3);
        p.y = ground_y(*p);
    }
}

pub fn despawn_salesman(commands: &mut Commands, e: Entity, s: &Salesman) {
    commands.entity(s.overhead.root).despawn();
    commands.entity(e).despawn();
}

/// 営業マン同士が重ならないように押し合う。
fn separate(mut q: Query<(&mut Transform, &Salesman)>) {
    let items: Vec<(Vec3, f32, bool)> = q
        .iter()
        .map(|(t, s)| (t.translation, s.radius(), s.is_fleeing()))
        .collect();
    for (i, (mut tf, s)) in q.iter_mut().enumerate() {
        if s.is_fleeing() {
            continue;
        }
        let mut push = Vec3::ZERO;
        for (j, (p, r, fleeing)) in items.iter().enumerate() {
            if i == j || *fleeing {
                continue;
            }
            let d = Vec3::new(tf.translation.x - p.x, 0.0, tf.translation.z - p.z);
            let min = s.radius() + r + 0.1;
            let len = d.length();
            if len < min && len > 0.001 {
                push += d / len * (min - len) * 0.5;
            }
        }
        tf.translation += push;
    }
}

fn animate(
    time: Res<Time>,
    q: Query<&Salesman>,
    mut joints: Query<&mut Transform, Without<Salesman>>,
) {
    let el = time.elapsed_secs();
    for s in &q {
        let swing = s.walk.sin();
        let (leg_amp, arm_amp) = match s.phase {
            Phase::Fleeing => (0.9, 1.2),
            Phase::Ringing => (0.0, 0.0),
            _ => (0.5, 0.4),
        };
        for (i, leg) in s.parts.legs.iter().enumerate() {
            if let Ok(mut t) = joints.get_mut(*leg) {
                let sign = if i == 0 { 1.0 } else { -1.0 };
                t.rotation = Quat::from_rotation_x(swing * leg_amp * sign);
            }
        }
        for (i, arm) in s.parts.arms.iter().enumerate() {
            if let Ok(mut t) = joints.get_mut(*arm) {
                let sign = if i == 0 { -1.0 } else { 1.0 };
                let mut rot = Quat::from_rotation_x(swing * arm_amp * sign);
                if s.phase == Phase::Ringing && i == 0 {
                    // 左手でインターホンを連打
                    rot = Quat::from_rotation_x(-1.4 + (el * 14.0 + s.seed).sin() * 0.15);
                }
                if s.phase == Phase::Fleeing {
                    rot *= Quat::from_rotation_z(if i == 0 { -0.6 } else { 0.6 });
                }
                t.rotation = rot;
            }
        }
        if let Ok(mut t) = joints.get_mut(s.parts.upper) {
            // 被弾でのけぞる、玄関前ではぺこぺこお辞儀
            let bow = if s.phase == Phase::Ringing {
                ((el * 2.5 + s.seed).sin() * 0.5 + 0.5) * 0.35
            } else {
                0.0
            };
            let lean = if s.phase == Phase::Fleeing { 0.35 } else { 0.08 };
            t.rotation = Quat::from_rotation_x(lean + bow - s.flinch * 0.6);
        }
    }
}

fn throw_shots(
    mut commands: Commands,
    time: Res<Time>,
    looks: Res<Looks>,
    mut q: Query<(&Transform, &mut Salesman)>,
    mut rng: ResMut<GameRng>,
) {
    let dt = time.delta_secs();
    for (tf, mut s) in &mut q {
        if !matches!(s.kind, Kind::Veteran | Kind::Boss) || s.phase != Phase::ToDoor || s.stun > 0.0 {
            continue;
        }
        if tf.translation.z > DOOR_Z - 3.0 {
            continue;
        }
        s.throw_cd -= dt;
        if s.throw_cd > 0.0 {
            continue;
        }
        let boss = s.kind == Kind::Boss;
        let enraged = boss && s.hp < s.max_hp * 0.5;
        s.throw_cd = if boss {
            if enraged { 1.8 } else { 2.8 }
        } else {
            rng.0.random_range(3.5..5.5)
        };
        let from = tf.translation + Vec3::Y * 1.3 * s.scale;
        let count = if boss { 3 } else { 1 };
        for i in 0..count {
            let spread = (i as f32 - (count - 1) as f32 * 0.5) * 0.9;
            let aim = EYE + Vec3::new(spread, rng.0.random_range(-0.2..0.2), 0.0);
            let flight = from.distance(aim) / if boss { 8.0 } else { 9.5 };
            let vel = (aim - from) / flight + Vec3::Y * 0.5 * 3.0 * flight;
            let (mesh, mat) = if boss {
                (&looks.pamphlet, &looks.pamphlet_mat)
            } else {
                (&looks.card, &looks.card_mat)
            };
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(from),
                Shot { vel, damage: if boss { 7.0 } else { 5.0 }, life: 6.0 },
            ));
        }
        let line = if boss { "パンフレットをどうぞ！" } else { "名刺だけでも！" };
        say(&mut commands, &mut s, line, 1.2);
    }
}

fn move_shots(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Shot)>,
    mut session: ResMut<Session>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut shot) in &mut q {
        shot.life -= dt;
        shot.vel.y -= 3.0 * dt;
        tf.translation += shot.vel * dt;
        tf.rotate_y(dt * 12.0);
        if tf.translation.distance(EYE) < 0.7 {
            session.hurt(shot.damage);
            session.shake = 1.0;
            sfx.write(PlaySfx(Sfx::Ouch));
            commands.entity(e).despawn();
        } else if shot.life <= 0.0 || tf.translation.y < 0.0 || tf.translation.z > EYE.z + 1.0 {
            commands.entity(e).despawn();
        }
    }
}

/// 頭上 UI をスクリーン座標に合わせる。
fn place_overheads(
    cam: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q: Query<(&GlobalTransform, &Salesman)>,
    mut nodes: Query<(&mut Node, &mut Visibility, &ComputedNode)>,
) {
    let Ok((camera, cam_tf)) = cam.single() else { return };
    for (gt, s) in &q {
        let head = gt.translation() + Vec3::Y * (s.height() + 0.15);
        let Ok((mut node, mut vis, computed)) = nodes.get_mut(s.overhead.root) else { continue };
        match camera.world_to_viewport(cam_tf, head) {
            Ok(p) => {
                let h = computed.size().y * computed.inverse_scale_factor();
                node.left = Val::Px(p.x - 120.0);
                node.top = Val::Px(p.y - h);
                *vis = Visibility::Inherited;
            }
            Err(_) => *vis = Visibility::Hidden,
        }
        if let Ok((mut fill, _, _)) = nodes.get_mut(s.overhead.fill) {
            fill.width = Val::Percent((s.hp / s.max_hp * 100.0).clamp(0.0, 100.0));
        }
    }
}

fn cleanup(mut commands: Commands, q: Query<(Entity, &Salesman)>, shots: Query<Entity, With<Shot>>) {
    for (e, s) in &q {
        despawn_salesman(&mut commands, e, s);
    }
    for e in &shots {
        commands.entity(e).despawn();
    }
}
