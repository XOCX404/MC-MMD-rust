use super::super::MMDPhysics;
use crate::physics::config::get_config;
use glam::{Mat4, Vec3};
use mmd::pmx::rigid_body::{RigidBody as PmxRigidBody, RigidBodyMode, RigidBodyShape};

const DT: f32 = 1.0 / 60.0;

fn dynamic_body(name: &str, position: [f32; 3]) -> PmxRigidBody {
    PmxRigidBody {
        local_name: name.to_owned(),
        universal_name: String::new(),
        bone_index: -1,
        group: 0,
        un_collision_group_flag: 0,
        shape: RigidBodyShape::Sphere,
        size: [0.08, 0.0, 0.0],
        position,
        rotation: [0.0; 3],
        mass: 1.0,
        move_attenuation: 0.0,
        rotation_attenuation: 0.0,
        repulsion: 0.0,
        friction: 0.0,
        mode: RigidBodyMode::Dynamic,
    }
}

fn run_motion(velocity: Vec3, movement_boost: bool) -> (Vec3, Vec3) {
    let config = get_config();
    assert!(config.inertia_strength > 0.0, "该回归需使用默认非零惯性");

    let mut physics = MMDPhysics::new().expect("Bullet 世界应可创建");
    physics.world.set_gravity(0.0, 0.0, 0.0);
    let bodies = [
        dynamic_body("Tail_01", [0.0, 0.0, 0.0]),
        dynamic_body("Accessory_01", [10.0, 0.0, 0.0]),
    ];
    physics.build_physics(&bodies, &[], &[]);
    physics.set_tail_physics_options(false, movement_boost);

    // 模拟已收敛的速度，只施力一步，避免未步进时积累多帧力。
    physics.prev_model_position = Some(Vec3::ZERO);
    physics.smoothed_model_velocity = velocity;
    physics.sync_bodies_with_model_velocity(&[], DT, Mat4::from_translation(velocity * DT));
    physics.step_simulation(DT);

    let tail_velocity = physics.rigid_bodies[0]
        .bullet_body
        .as_ref()
        .expect("尾巴 Bullet 刚体已创建")
        .get_linear_velocity();
    let other_velocity = physics.rigid_bodies[1]
        .bullet_body
        .as_ref()
        .expect("普通 Bullet 刚体已创建")
        .get_linear_velocity();
    (tail_velocity, other_velocity)
}

fn assert_vec3_close(actual: Vec3, expected: Vec3, tolerance: f32) {
    assert!(
        actual.abs_diff_eq(expected, tolerance),
        "实际速度 {actual:?}，期望 {expected:?}"
    );
}

#[test]
fn bullet_tail_movement_force_boosts_all_axes_and_diagonal_by_one_point_five() {
    let directions = [
        Vec3::X,
        -Vec3::X,
        Vec3::Y,
        -Vec3::Y,
        Vec3::Z,
        -Vec3::Z,
        Vec3::new(1.0, -2.0, 3.0).normalize(),
    ];

    for direction in directions {
        let (boosted_tail, boosted_other) = run_motion(direction * 2.0, true);
        let (original_tail, original_other) = run_motion(direction * 2.0, false);
        assert!(
            original_tail.length() > 1e-4,
            "方向 {direction:?} 未产生尾巴受力"
        );
        assert_vec3_close(boosted_tail, original_tail * 1.5, 1e-5);
        assert_vec3_close(boosted_other, original_other, 1e-4);
    }
}

#[test]
fn bullet_tail_disabled_boost_keeps_original_formula_and_run_exceeds_walk() {
    let (walk_tail, _) = run_motion(Vec3::Z * 43.0, false);
    let (run_tail, _) = run_motion(Vec3::Z * 56.0, false);
    assert!(walk_tail.length() > 1e-4);
    assert!(run_tail.length() > walk_tail.length() * 1.2);

    let (boosted_tail, _) = run_motion(Vec3::Z * 43.0, true);
    assert_vec3_close(boosted_tail, walk_tail * 1.5, 1e-5);
}

#[test]
fn idle_lift_generates_backward_torque_and_is_independent_of_movement_boost() {
    use crate::physics::tail_forces::TailForceState;

    fn run_idle(mut state: TailForceState, idle: bool, movement: bool) -> (Vec3, Vec3) {
        let mut physics = MMDPhysics::new().expect("Bullet world");
        physics.world.set_gravity(0.0, 0.0, 0.0);
        physics.build_physics(
            &[
                dynamic_body("Tail_01", [0.0, 0.0, 0.0]),
                dynamic_body("Accessory_01", [10.0, 0.0, 0.0]),
            ],
            &[],
            &[],
        );
        state.set_options(idle, movement);
        physics.tail_forces = state;
        for _ in 0..180 {
            physics.sync_bodies_with_model_velocity(&[], DT, Mat4::IDENTITY);
            physics.step_simulation(DT);
        }
        let velocity = |index: usize| {
            physics.rigid_bodies[index]
                .bullet_body
                .as_ref()
                .unwrap()
                .get_linear_velocity()
        };
        (velocity(0), velocity(1))
    }

    // 相同随机状态对比独立开关，避免随机脉冲时刻掩盖叠加错误。
    let state = TailForceState::default();
    let (idle_only, other) = run_idle(state.clone(), true, false);
    let (combined, _) = run_idle(state.clone(), true, true);
    let (disabled, _) = run_idle(state, false, false);
    assert!(idle_only.y > 0.0 && idle_only.z > 0.0);
    assert_vec3_close(combined, idle_only, 1e-5);
    assert_vec3_close(other, Vec3::ZERO, 1e-5);
    assert_vec3_close(disabled, Vec3::ZERO, 1e-5);
}
