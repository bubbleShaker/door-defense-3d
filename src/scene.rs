//! 舞台: 自宅の玄関ポーチから見た前庭・門・道路と、一人称カメラ。

use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    light::{CascadeShadowConfigBuilder, GlobalAmbientLight},
    prelude::*,
};

use crate::{GameState, Session, weapon::Aim};

/// プレイヤー (カメラ) の目の位置。
pub const EYE: Vec3 = Vec3::new(0.0, 1.62, 1.1);
/// 門の中心 z 座標。
pub const GATE_Z: f32 = -22.0;
/// 門の開口部の半幅。
pub const GATE_HALF: f32 = 1.35;
/// 庭の塀の x 座標 (±)。
pub const YARD_HALF: f32 = 9.5;
/// 営業マンがインターホンを押す位置の z。
pub const DOOR_Z: f32 = -3.1;
/// ポーチ (玄関前の床) の高さ。
pub const PORCH_H: f32 = 0.12;

#[derive(Component)]
pub struct MainCamera;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.62, 0.78, 0.93)))
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.85, 0.9, 1.0),
                brightness: 600.0,
                ..default()
            })
            .add_systems(Startup, (spawn_camera, spawn_world))
            .add_systems(Update, drive_camera);
    }
}

/// 地面の高さ (ポーチの上だけ少し高い)。
pub fn ground_y(p: Vec3) -> f32 {
    if p.z > -2.6 && p.x.abs() < 3.2 { PORCH_H } else { 0.0 }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        MainCamera,
        Tonemapping::TonyMcMapface,
        Projection::Perspective(PerspectiveProjection {
            fov: 62f32.to_radians(),
            far: 400.0,
            ..default()
        }),
        DistanceFog {
            color: Color::srgb(0.66, 0.78, 0.9),
            falloff: FogFalloff::Linear { start: 45.0, end: 160.0 },
            ..default()
        },
        Transform::from_translation(EYE).looking_at(Vec3::new(0.0, 1.0, GATE_Z), Vec3::Y),
    ));
}

/// タイトル中は庭を見下ろし、プレイ中は玄関から照準方向へ少し首を振る。
fn drive_camera(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    aim: Res<Aim>,
    mut session: ResMut<Session>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(mut tf) = cam.single_mut() else { return };
    let dt = time.delta_secs().min(0.05);
    let el = time.elapsed_secs();

    let (pos, look) = match state.get() {
        GameState::Title => (
            Vec3::new((el * 0.15).sin() * 2.0, 3.2, 3.5),
            Vec3::new(0.0, 0.8, -12.0),
        ),
        _ => {
            // 照準 (画面上の位置 -1..1) に合わせて視線をわずかにずらす
            let n = aim.ndc;
            (
                EYE + Vec3::new(n.x * 0.15, (el * 1.3).sin() * 0.01, 0.0),
                Vec3::new(n.x * 4.0, 1.0 + n.y * 2.0, GATE_Z * 0.8),
            )
        }
    };

    let k = 1.0 - (-dt * 5.0).exp();
    tf.translation = tf.translation.lerp(pos, k);
    let target = tf.looking_at(look, Vec3::Y).rotation;
    tf.rotation = tf.rotation.slerp(target, k);

    if session.shake > 0.0 {
        let s = session.shake * session.shake * 0.08;
        tf.translation += Vec3::new((el * 71.0).sin() * s, (el * 53.0).cos() * s, 0.0);
        session.shake = (session.shake - dt * 2.5).max(0.0);
    }
}

/// メッシュとマテリアルを使い回しつつ静的な箱・円柱などを置くためのヘルパー。
struct Kit<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    mats: &'a mut Assets<StandardMaterial>,
}

impl Kit<'_, '_, '_> {
    fn mat(&mut self, color: Color, rough: f32) -> Handle<StandardMaterial> {
        self.mats.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: rough,
            ..default()
        })
    }

    fn cube(&mut self, size: Vec3, pos: Vec3, mat: &Handle<StandardMaterial>) -> Entity {
        let mesh = self.meshes.add(Cuboid::from_size(size));
        self.commands
            .spawn((Mesh3d(mesh), MeshMaterial3d(mat.clone()), Transform::from_translation(pos)))
            .id()
    }

    fn cube_rot(&mut self, size: Vec3, tf: Transform, mat: &Handle<StandardMaterial>) {
        let mesh = self.meshes.add(Cuboid::from_size(size));
        self.commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat.clone()), tf));
    }

    fn cylinder(&mut self, r: f32, h: f32, pos: Vec3, mat: &Handle<StandardMaterial>) {
        let mesh = self.meshes.add(Cylinder::new(r, h));
        self.commands
            .spawn((Mesh3d(mesh), MeshMaterial3d(mat.clone()), Transform::from_translation(pos)));
    }

    fn sphere(&mut self, r: f32, pos: Vec3, scale: Vec3, mat: &Handle<StandardMaterial>) {
        let mesh = self.meshes.add(Sphere::new(r).mesh().ico(3).unwrap());
        self.commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pos).with_scale(scale),
        ));
    }

    fn tree(&mut self, pos: Vec3, h: f32, trunk: &Handle<StandardMaterial>, leaf: &Handle<StandardMaterial>) {
        self.cylinder(0.14, h * 0.55, pos + Vec3::Y * h * 0.275, trunk);
        self.sphere(h * 0.32, pos + Vec3::Y * h * 0.72, Vec3::new(1.0, 1.1, 1.0), leaf);
        self.sphere(h * 0.22, pos + Vec3::new(0.3, h * 0.92, 0.1), Vec3::ONE, leaf);
    }

    /// 切妻屋根の家。
    fn house(&mut self, center: Vec3, w: f32, d: f32, h: f32, wall: Color, roof: Color) {
        let wall_m = self.mat(wall, 0.9);
        let roof_m = self.mat(roof, 0.7);
        let win_m = self.mat(Color::srgb(0.35, 0.5, 0.62), 0.15);
        self.cube(Vec3::new(w, h, d), center + Vec3::Y * h * 0.5, &wall_m);
        let slope = 0.5f32;
        let half = d * 0.5 + 0.4;
        let len = half / slope.cos();
        let rise = half * slope.tan();
        for s in [-1.0f32, 1.0] {
            let tf = Transform::from_translation(center + Vec3::new(0.0, h + rise * 0.5, s * half * 0.5))
                .with_rotation(Quat::from_rotation_x(s * slope));
            self.cube_rot(Vec3::new(w + 0.6, 0.18, len), tf, &roof_m);
        }
        // 窓 (道路側 = +z 面)
        for i in 0..3 {
            let x = (i as f32 - 1.0) * w * 0.3;
            for y in [h * 0.3, h * 0.72] {
                self.cube(Vec3::new(1.0, 0.8, 0.05), center + Vec3::new(x, y, d * 0.5 + 0.03), &win_m);
            }
        }
    }
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // 太陽
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            maximum_distance: 60.0,
            first_cascade_far_bound: 12.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(-12.0, 20.0, 8.0).looking_at(Vec3::new(0.0, 0.0, -10.0), Vec3::Y),
    ));

    let mut k = Kit { commands: &mut commands, meshes: &mut meshes, mats: &mut mats };

    let lawn = k.mat(Color::srgb(0.36, 0.55, 0.25), 1.0);
    let soil = k.mat(Color::srgb(0.42, 0.52, 0.3), 1.0);
    let asphalt = k.mat(Color::srgb(0.25, 0.26, 0.28), 0.95);
    let white = k.mat(Color::srgb(0.92, 0.92, 0.9), 0.8);
    let block = k.mat(Color::srgb(0.72, 0.7, 0.66), 0.95);
    let block_cap = k.mat(Color::srgb(0.58, 0.56, 0.53), 0.9);
    let stone = k.mat(Color::srgb(0.66, 0.64, 0.6), 0.9);
    let tile = k.mat(Color::srgb(0.55, 0.47, 0.42), 0.8);
    let wood = k.mat(Color::srgb(0.45, 0.3, 0.18), 0.8);
    let dark_wood = k.mat(Color::srgb(0.3, 0.2, 0.12), 0.8);
    let trunk = k.mat(Color::srgb(0.38, 0.27, 0.18), 1.0);
    let leaf = k.mat(Color::srgb(0.22, 0.45, 0.2), 1.0);
    let hedge = k.mat(Color::srgb(0.2, 0.4, 0.18), 1.0);
    let pole = k.mat(Color::srgb(0.6, 0.6, 0.58), 0.8);
    let red = k.mat(Color::srgb(0.75, 0.15, 0.12), 0.5);
    let pot = k.mat(Color::srgb(0.7, 0.38, 0.22), 0.9);
    let flower = k.mat(Color::srgb(0.95, 0.5, 0.65), 0.8);
    let yellow = k.mat(Color::srgb(0.95, 0.8, 0.2), 0.8);

    // 地面
    k.cube(Vec3::new(2.0 * YARD_HALF, 0.1, 24.0), Vec3::new(0.0, -0.05, GATE_Z / 2.0 - 0.5), &lawn);
    k.cube(Vec3::new(200.0, 0.1, 200.0), Vec3::new(0.0, -0.12, -40.0), &soil);
    // 道路
    k.cube(Vec3::new(200.0, 0.1, 9.0), Vec3::new(0.0, -0.04, GATE_Z - 5.5), &asphalt);
    for i in -20..20 {
        k.cube(Vec3::new(2.0, 0.02, 0.15), Vec3::new(i as f32 * 5.0, 0.02, GATE_Z - 5.5), &white);
    }
    // 歩道の縁石
    k.cube(Vec3::new(200.0, 0.14, 0.3), Vec3::new(0.0, 0.03, GATE_Z - 1.0), &stone);

    // 玄関ポーチ
    k.cube(Vec3::new(6.4, PORCH_H, 4.4), Vec3::new(0.0, PORCH_H * 0.5, -0.4), &tile);
    // ポーチの柱と庇 (画面の上端・左右に見えるフレーム)
    for s in [-1.0f32, 1.0] {
        k.cube(Vec3::new(0.22, 3.3, 0.22), Vec3::new(s * 2.9, 1.65, -2.3), &white);
    }
    k.cube(Vec3::new(6.8, 0.2, 4.2), Vec3::new(0.0, 3.35, 0.1), &white);
    k.cube(Vec3::new(6.8, 0.3, 0.12), Vec3::new(0.0, 3.2, -2.0), &dark_wood);
    // 右の柱に付いたインターホンと植木鉢
    k.cube(Vec3::new(0.14, 0.2, 0.06), Vec3::new(2.9, 1.35, -2.44), &dark_wood);
    for x in [-2.3f32, 2.3] {
        k.cylinder(0.28, 0.5, Vec3::new(x, PORCH_H + 0.25, -2.0), &pot);
        k.sphere(0.35, Vec3::new(x, PORCH_H + 0.75, -2.0), Vec3::ONE, &leaf);
        k.sphere(0.1, Vec3::new(x + 0.15, PORCH_H + 1.0, -2.2), Vec3::ONE, &flower);
    }

    // 飛び石
    let mut z = -3.4;
    let mut i = 0;
    while z > GATE_Z + 0.8 {
        let x = if i % 2 == 0 { 0.18 } else { -0.18 };
        k.cylinder(0.5, 0.06, Vec3::new(x, 0.02, z), &stone);
        z -= 1.25;
        i += 1;
    }

    // ブロック塀 (左右と門の両側)
    let wall_h = 1.3;
    for s in [-1.0f32, 1.0] {
        let len = 1.5 - GATE_Z;
        k.cube(Vec3::new(0.2, wall_h, len), Vec3::new(s * YARD_HALF, wall_h / 2.0, GATE_Z + len / 2.0), &block);
        k.cube(Vec3::new(0.26, 0.08, len), Vec3::new(s * YARD_HALF, wall_h, GATE_Z + len / 2.0), &block_cap);
        let seg = YARD_HALF - GATE_HALF - 0.3;
        let cx = s * (GATE_HALF + 0.3 + seg / 2.0);
        k.cube(Vec3::new(seg, wall_h, 0.2), Vec3::new(cx, wall_h / 2.0, GATE_Z), &block);
        k.cube(Vec3::new(seg, 0.08, 0.26), Vec3::new(cx, wall_h, GATE_Z), &block_cap);
        // 門柱
        k.cube(Vec3::new(0.55, 1.75, 0.55), Vec3::new(s * (GATE_HALF + 0.15), 0.875, GATE_Z), &block_cap);
        // 開いた門扉
        let tf = Transform::from_translation(Vec3::new(s * (GATE_HALF - 0.1), 0.6, GATE_Z + 0.6))
            .with_rotation(Quat::from_rotation_y(s * 1.3));
        k.cube_rot(Vec3::new(1.2, 1.0, 0.05), tf, &dark_wood);
    }
    // ポスト
    k.cube(Vec3::new(0.35, 0.45, 0.25), Vec3::new(GATE_HALF + 0.15, 1.95, GATE_Z), &red);

    // 庭木・生垣・花壇
    k.tree(Vec3::new(-7.0, 0.0, -17.0), 4.2, &trunk, &leaf);
    k.tree(Vec3::new(7.2, 0.0, -8.0), 3.6, &trunk, &leaf);
    k.tree(Vec3::new(-6.5, 0.0, -5.0), 3.0, &trunk, &leaf);
    for j in 0..6 {
        let z = GATE_Z + 2.0 + j as f32 * 3.2;
        k.sphere(0.7, Vec3::new(YARD_HALF - 0.7, 0.45, z), Vec3::new(1.0, 0.7, 1.3), &hedge);
    }
    for j in 0..5 {
        let x = -8.0 + j as f32 * 0.8;
        k.sphere(0.18, Vec3::new(x, 0.2, -10.5), Vec3::ONE, if j % 2 == 0 { &flower } else { &yellow });
    }
    k.cube(Vec3::new(4.2, 0.15, 1.0), Vec3::new(-6.4, 0.07, -10.5), &wood);

    // 電柱
    for x in [-14.0f32, 16.0] {
        k.cylinder(0.18, 9.0, Vec3::new(x, 4.5, GATE_Z - 1.6), &pole);
        k.cube(Vec3::new(1.6, 0.12, 0.12), Vec3::new(x, 8.0, GATE_Z - 1.6), &pole);
    }

    // 向かいの家並み
    let walls = [
        Color::srgb(0.9, 0.86, 0.78),
        Color::srgb(0.78, 0.82, 0.86),
        Color::srgb(0.88, 0.8, 0.7),
        Color::srgb(0.84, 0.84, 0.8),
    ];
    let roofs = [
        Color::srgb(0.35, 0.3, 0.3),
        Color::srgb(0.55, 0.25, 0.2),
        Color::srgb(0.25, 0.32, 0.42),
        Color::srgb(0.4, 0.35, 0.28),
    ];
    for (n, x) in [-26.0f32, -12.0, 2.0, 16.0, 30.0].into_iter().enumerate() {
        k.house(Vec3::new(x, 0.0, GATE_Z - 17.0), 10.0, 8.0, 5.5, walls[n % 4], roofs[n % 4]);
        k.cube(Vec3::new(12.0, 1.2, 0.2), Vec3::new(x, 0.6, GATE_Z - 10.5), &block);
    }
    // 両隣の家 (塀の外)
    for s in [-1.0f32, 1.0] {
        k.house(Vec3::new(s * 17.0, 0.0, -8.0), 9.0, 10.0, 5.5, walls[(s as i32 + 2) as usize], roofs[3]);
    }
}
