//! 照準・武器切替・投擲物・ヒット判定・パーティクル。

use bevy::{input::mouse::AccumulatedMouseScroll, prelude::*};
use rand::{Rng, SeedableRng, rngs::SmallRng};

use crate::{
    GameState, Session,
    salesman::{Salesman, Shot, Hit},
    scene::{EYE, GATE_Z, MainCamera, ground_y},
    sfx::{PlaySfx, Sfx},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weapon {
    Salt,
    Water,
    Sticker,
}

impl Weapon {
    pub const ALL: [Weapon; 3] = [Weapon::Salt, Weapon::Water, Weapon::Sticker];

    pub fn index(self) -> usize {
        match self {
            Weapon::Salt => 0,
            Weapon::Water => 1,
            Weapon::Sticker => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Weapon::Salt => "塩",
            Weapon::Water => "水バケツ",
            Weapon::Sticker => "お断りステッカー",
        }
    }

    pub fn cooldown(self) -> f32 {
        match self {
            Weapon::Salt => 0.15,
            Weapon::Water => 0.9,
            Weapon::Sticker => 2.4,
        }
    }
}

/// 照準の状態。
#[derive(Resource, Default)]
pub struct Aim {
    /// 画面上のカーソル位置 (論理ピクセル)。
    pub screen: Vec2,
    /// 正規化した画面位置 (-1..1, 上が +)。
    pub ndc: Vec2,
    /// 照準がワールドで指している点。
    pub target: Vec3,
    /// 照準が営業マンに重なっているか。
    pub on_enemy: bool,
    pub valid: bool,
}

#[derive(Resource)]
pub struct Arsenal {
    pub current: Weapon,
    /// 各武器の残りクールダウン (秒)。
    pub cooldown: [f32; 3],
    /// プレイ開始直後にクリックが暴発しないための猶予。
    pub grace: f32,
}

impl Default for Arsenal {
    fn default() -> Self {
        Self { current: Weapon::Salt, cooldown: [0.0; 3], grace: 0.4 }
    }
}

/// 画面下の武器ボタン (UI 側で生成)。押されている間は射撃しない。
#[derive(Component)]
pub struct WeaponButton(pub Weapon);

#[derive(Component)]
pub struct Projectile {
    weapon: Weapon,
    vel: Vec3,
    gravity: f32,
    radius: f32,
    life: f32,
    /// ステッカーは貫通する。既に当たった相手。
    pierced: Vec<Entity>,
    spin: Vec3,
}

#[derive(Component)]
pub struct Particle {
    vel: Vec3,
    life: f32,
    max_life: f32,
    gravity: f32,
    base_scale: f32,
}

#[derive(Resource)]
struct WeaponAssets {
    salt_mesh: Handle<Mesh>,
    salt_mat: Handle<StandardMaterial>,
    water_mesh: Handle<Mesh>,
    water_mat: Handle<StandardMaterial>,
    sticker_mesh: Handle<Mesh>,
    sticker_mat: Handle<StandardMaterial>,
    drop_mesh: Handle<Mesh>,
}

#[derive(Resource)]
pub struct GameRng(pub SmallRng);

impl Default for GameRng {
    fn default() -> Self {
        Self(SmallRng::seed_from_u64(0x5A1E_5A1E))
    }
}

pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Aim>()
            .init_resource::<Arsenal>()
            .init_resource::<GameRng>()
            .add_systems(Startup, setup_assets)
            .add_systems(OnExit(GameState::Title), |mut a: ResMut<Arsenal>| *a = Arsenal::default())
            .add_systems(Update, update_aim)
            .add_systems(
                Update,
                (switch_weapon, fire, move_projectiles)
                    .chain()
                    .after(update_aim)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, update_particles)
            .add_systems(OnEnter(GameState::Title), cleanup);
    }
}

fn setup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(WeaponAssets {
        salt_mesh: meshes.add(Cuboid::from_length(0.07)),
        salt_mat: mats.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::rgb(0.6, 0.6, 0.6),
            ..default()
        }),
        water_mesh: meshes.add(Sphere::new(0.28).mesh().ico(2).unwrap()),
        water_mat: mats.add(StandardMaterial {
            base_color: Color::srgba(0.35, 0.65, 1.0, 0.7),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            ..default()
        }),
        sticker_mesh: meshes.add(Cuboid::new(0.42, 0.02, 0.3)),
        sticker_mat: mats.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.1),
            emissive: LinearRgba::rgb(0.5, 0.35, 0.0),
            ..default()
        }),
        drop_mesh: meshes.add(Sphere::new(0.08).mesh().ico(1).unwrap()),
    });
}

/// マウス / タッチ位置から照準を更新する。
fn update_aim(
    windows: Query<&Window>,
    touches: Res<Touches>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    salesmen: Query<(&Transform, &Salesman)>,
    mut aim: ResMut<Aim>,
) {
    let Ok(window) = windows.single() else { return };
    let pos = touches
        .first_pressed_position()
        .or_else(|| window.cursor_position());
    let Some(pos) = pos else { return };
    let size = window.size();
    aim.screen = pos;
    aim.ndc = Vec2::new(pos.x / size.x * 2.0 - 1.0, 1.0 - pos.y / size.y * 2.0);

    let Ok((camera, cam_tf)) = cam.single() else { return };
    let Ok(ray) = camera.viewport_to_world(cam_tf, pos) else { return };
    aim.valid = true;

    // 営業マン (縦長の円柱として近似) に当たるか
    let mut best: Option<(f32, Vec3)> = None;
    for (tf, s) in &salesmen {
        if s.is_fleeing() {
            continue;
        }
        let base = tf.translation;
        // 光線上で円柱の軸に最も近づく点を粗く探索する
        let d = (base - ray.origin).length();
        let p = ray.origin + *ray.direction * d;
        let horiz = Vec2::new(p.x - base.x, p.z - base.z).length();
        if horiz < s.radius() * 1.3
            && p.y > base.y
            && p.y < base.y + s.height()
            && best.is_none_or(|(bd, _)| d < bd)
        {
            best = Some((d, p));
        }
    }
    if let Some((_, p)) = best {
        aim.target = p;
        aim.on_enemy = true;
        return;
    }
    aim.on_enemy = false;
    // 地面 (y=0) との交点。空を指していたら遠方に置く
    let dir = *ray.direction;
    let t = if dir.y < -0.01 { -ray.origin.y / dir.y } else { 60.0 };
    let mut p = ray.origin + dir * t.min(60.0);
    if p.z < GATE_Z - 12.0 {
        p = ray.origin + dir * ((GATE_Z - 12.0 - ray.origin.z) / dir.z.min(-0.01));
    }
    aim.target = p;
}

fn switch_weapon(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Query<(&Interaction, &WeaponButton), Changed<Interaction>>,
    mut arsenal: ResMut<Arsenal>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let mut next = None;
    for (key, w) in [
        (KeyCode::Digit1, Weapon::Salt),
        (KeyCode::Digit2, Weapon::Water),
        (KeyCode::Digit3, Weapon::Sticker),
    ] {
        if keys.just_pressed(key) {
            next = Some(w);
        }
    }
    if keys.just_pressed(KeyCode::KeyQ) || scroll.delta.y < 0.0 {
        next = Some(Weapon::ALL[(arsenal.current.index() + 1) % 3]);
    } else if scroll.delta.y > 0.0 {
        next = Some(Weapon::ALL[(arsenal.current.index() + 2) % 3]);
    }
    for (interaction, WeaponButton(w)) in &buttons {
        if *interaction == Interaction::Pressed {
            next = Some(*w);
        }
    }
    if let Some(w) = next
        && w != arsenal.current
    {
        arsenal.current = w;
        sfx.write(PlaySfx(Sfx::Click));
    }
}

fn fire(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    buttons: Query<&Interaction, With<WeaponButton>>,
    aim: Res<Aim>,
    assets: Res<WeaponAssets>,
    mut arsenal: ResMut<Arsenal>,
    mut rng: ResMut<GameRng>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let dt = time.delta_secs();
    for c in &mut arsenal.cooldown {
        *c = (*c - dt).max(0.0);
    }
    if arsenal.grace > 0.0 {
        arsenal.grace -= dt;
        return;
    }
    let pressing = mouse.pressed(MouseButton::Left) || touches.iter().next().is_some();
    let over_button = buttons.iter().any(|i| *i != Interaction::None);
    if !pressing || over_button || !aim.valid {
        return;
    }
    let w = arsenal.current;
    if arsenal.cooldown[w.index()] > 0.0 {
        return;
    }
    arsenal.cooldown[w.index()] = w.cooldown();

    // 右手から投げる
    let start = EYE + Vec3::new(0.3, -0.35, -0.4);
    let target = aim.target;
    let dist = start.distance(target);
    let rng = &mut rng.0;

    let (speed, gravity) = match w {
        Weapon::Salt => (26.0, 9.8),
        Weapon::Water => (15.0, 9.8),
        Weapon::Sticker => (38.0, 2.0),
    };
    let launch = |target: Vec3| -> Vec3 {
        let t = (start.distance(target) / speed).clamp(0.12, 1.6);
        (target - start) / t + Vec3::Y * 0.5 * gravity * t
    };

    match w {
        Weapon::Salt => {
            // 塩はひとつまみ = 数粒をばらまく
            let spread = 0.12 + dist * 0.012;
            for _ in 0..4 {
                let off = Vec3::new(
                    rng.random_range(-spread..spread),
                    rng.random_range(-spread..spread),
                    rng.random_range(-spread..spread),
                );
                commands.spawn((
                    Mesh3d(assets.salt_mesh.clone()),
                    MeshMaterial3d(assets.salt_mat.clone()),
                    Transform::from_translation(start),
                    Projectile {
                        weapon: w,
                        vel: launch(target + off),
                        gravity,
                        radius: 0.12,
                        life: 3.0,
                        pierced: Vec::new(),
                        spin: Vec3::new(9.0, 7.0, 5.0),
                    },
                ));
            }
        }
        Weapon::Water => {
            commands.spawn((
                Mesh3d(assets.water_mesh.clone()),
                MeshMaterial3d(assets.water_mat.clone()),
                Transform::from_translation(start),
                Projectile {
                    weapon: w,
                    vel: launch(target),
                    gravity,
                    radius: 0.35,
                    life: 3.0,
                    pierced: Vec::new(),
                    spin: Vec3::ZERO,
                },
            ));
        }
        Weapon::Sticker => {
            commands.spawn((
                Mesh3d(assets.sticker_mesh.clone()),
                MeshMaterial3d(assets.sticker_mat.clone()),
                Transform::from_translation(start),
                Projectile {
                    weapon: w,
                    vel: launch(target),
                    gravity,
                    radius: 0.3,
                    life: 2.0,
                    pierced: Vec::new(),
                    spin: Vec3::new(0.0, 22.0, 0.0),
                },
            ));
        }
    }
    sfx.write(PlaySfx(Sfx::Throw));
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<WeaponAssets>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
    mut salesmen: Query<(Entity, &Transform, &Salesman), Without<Projectile>>,
    shots: Query<(Entity, &Transform), (With<Shot>, Without<Projectile>, Without<Salesman>)>,
    mut hits: MessageWriter<Hit>,
    mut session: ResMut<Session>,
    mut rng: ResMut<GameRng>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut p) in &mut projectiles {
        p.life -= dt;
        let spin = p.spin * dt;
        tf.rotate(Quat::from_euler(EulerRot::XYZ, spin.x, spin.y, spin.z));
        let mut impact: Option<Vec3> = None;
        let mut despawn = p.life <= 0.0;

        // 低フレームレートでもすり抜けないよう、1 ステップ 0.25m 以下に刻んで線分で判定する
        let steps = ((p.vel.length() * dt / 0.25).ceil() as usize).clamp(1, 32);
        'steps: for _ in 0..steps {
            let sdt = dt / steps as f32;
            let prev = tf.translation;
            p.vel.y -= p.gravity * sdt;
            tf.translation += p.vel * sdt;
            let pos = tf.translation;

            // 飛んでくる名刺・パンフレットを撃ち落とす
            for (se, stf) in &shots {
                if stf.translation.distance(pos) < p.radius + 0.35 {
                    commands.entity(se).despawn();
                    session.score += 5;
                    burst(&mut commands, &assets, &mut rng.0, stf.translation, Fx::Salt, 6);
                    sfx.write(PlaySfx(Sfx::SaltHit));
                    if p.weapon != Weapon::Sticker {
                        despawn = true;
                        break 'steps;
                    }
                }
            }

            for (se, stf, s) in &mut salesmen {
                if s.is_fleeing() || p.pierced.contains(&se) {
                    continue;
                }
                let base = stf.translation;
                let (horiz, y) = closest_to_axis(prev, pos, base);
                if horiz < s.radius() + p.radius && y > base.y - 0.1 && y < base.y + s.height() + 0.1 {
                    match p.weapon {
                        Weapon::Salt => {
                            hits.write(Hit { target: se, damage: 7.0, knock: Vec3::ZERO, stun: 0.08, weapon: p.weapon });
                            session.score += 1;
                            burst(&mut commands, &assets, &mut rng.0, pos, Fx::Salt, 4);
                            sfx.write(PlaySfx(Sfx::SaltHit));
                            despawn = true;
                            break 'steps;
                        }
                        Weapon::Water => {
                            impact = Some(pos);
                            despawn = true;
                            break 'steps;
                        }
                        Weapon::Sticker => {
                            let dir = Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or_zero();
                            hits.write(Hit { target: se, damage: 75.0, knock: dir * 2.5, stun: 0.5, weapon: p.weapon });
                            session.score += 10;
                            burst(&mut commands, &assets, &mut rng.0, pos, Fx::Sticker, 10);
                            sfx.write(PlaySfx(Sfx::Sticker));
                            p.pierced.push(se);
                            if p.pierced.len() >= 3 {
                                despawn = true;
                                break 'steps;
                            }
                        }
                    }
                }
            }

            if pos.y <= ground_y(pos) {
                if p.weapon == Weapon::Water {
                    impact = Some(pos);
                }
                despawn = true;
                break 'steps;
            }
        }

        // 水は着弾点の周囲にまとめて効く
        if let Some(at) = impact {
            for (se, stf, s) in &salesmen {
                if s.is_fleeing() {
                    continue;
                }
                let d = Vec2::new(stf.translation.x - at.x, stf.translation.z - at.z);
                let dist = d.length();
                if dist < 2.6 {
                    let falloff = 1.0 - dist / 2.6 * 0.5;
                    let away = if dist > 0.01 { d / dist } else { Vec2::new(0.0, -1.0) };
                    // 基本は門の方 (-z) へ押し返す
                    let dir = (Vec3::new(away.x, 0.0, away.y) * 0.5 + Vec3::NEG_Z).normalize();
                    hits.write(Hit { target: se, damage: 32.0 * falloff, knock: dir * 5.5 * falloff, stun: 0.7, weapon: Weapon::Water });
                    session.score += 3;
                }
            }
            burst(&mut commands, &assets, &mut rng.0, at + Vec3::Y * 0.2, Fx::Water, 22);
            sfx.write(PlaySfx(Sfx::Splash));
        }
        if despawn {
            commands.entity(e).despawn();
        }
    }
}

/// 線分 a→b が、点 base を通る鉛直軸に水平方向で最も近づくときの (水平距離, その点の高さ)。
fn closest_to_axis(a: Vec3, b: Vec3, base: Vec3) -> (f32, f32) {
    let ab = Vec2::new(b.x - a.x, b.z - a.z);
    let ap = Vec2::new(base.x - a.x, base.z - a.z);
    let t = if ab.length_squared() > 1e-6 { (ap.dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 1.0 };
    let p = a.lerp(b, t);
    (Vec2::new(base.x - p.x, base.z - p.z).length(), p.y)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fx {
    Salt,
    Sticker,
    Water,
}

/// 小さな粒を飛び散らせる。
fn burst(commands: &mut Commands, assets: &WeaponAssets, rng: &mut SmallRng, at: Vec3, fx: Fx, n: usize) {
    for _ in 0..n {
        let vel = Vec3::new(
            rng.random_range(-2.5..2.5),
            rng.random_range(1.0..4.5),
            rng.random_range(-2.5..2.5),
        );
        let life = rng.random_range(0.35..0.7);
        let scale = rng.random_range(0.6..1.3);
        let (mesh, mat) = match fx {
            Fx::Salt => (&assets.salt_mesh, &assets.salt_mat),
            Fx::Sticker => (&assets.salt_mesh, &assets.sticker_mat),
            Fx::Water => (&assets.drop_mesh, &assets.water_mat),
        };
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(scale)),
            Particle { vel, life, max_life: life, gravity: 9.8, base_scale: scale },
        ));
    }
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Particle)>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut p) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        p.vel.y -= p.gravity * dt;
        tf.translation += p.vel * dt;
        tf.scale = Vec3::splat(p.base_scale * (p.life / p.max_life).max(0.05));
    }
}

fn cleanup(mut commands: Commands, q: Query<Entity, Or<(With<Projectile>, With<Particle>)>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}
