//! 尾巴闲置轻抬与全向移动增强。

use glam::Vec3;
use std::sync::atomic::{AtomicU64, Ordering};

const LIFT_GRAVITY_MIN: f32 = 0.08;
const LIFT_GRAVITY_MAX: f32 = 0.14;
const IDLE_LIFT_MULTIPLIER: f32 = 2.0;
const MOVEMENT_FORCE_MULTIPLIER: f32 = 1.5;
const RESTORE_SECONDS: f32 = 0.55;
static NEXT_TAIL_SEED: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);

#[derive(Debug, Clone)]
pub(crate) struct TailForceState {
    pub idle_lift: bool,
    pub movement_boost: bool,
    rng: u64,
    time_to_next_lift: f32,
    lift_age: Option<f32>,
    lift_duration: f32,
    lift_accel: f32,
    movement_fade: f32,
}

impl Default for TailForceState {
    fn default() -> Self {
        Self::with_seed(NEXT_TAIL_SEED.fetch_add(0xA076_1D64_78BD_642F, Ordering::Relaxed))
    }
}

impl TailForceState {
    fn with_seed(seed: u64) -> Self {
        Self {
            idle_lift: true,
            movement_boost: true,
            rng: seed.max(1),
            time_to_next_lift: 1.2,
            lift_age: None,
            lift_duration: 0.7,
            lift_accel: 0.0,
            movement_fade: 0.0,
        }
    }

    pub fn set_options(&mut self, idle_lift: bool, movement_boost: bool) {
        self.idle_lift = idle_lift;
        self.movement_boost = movement_boost;
        if !idle_lift {
            self.lift_age = None;
            self.time_to_next_lift = 1.2;
        }
    }

    pub fn update(&mut self, delta_time: f32, local_velocity: Vec3, gravity_magnitude: f32) -> f32 {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            self.reset_envelope();
            return 0.0;
        }

        let dt = delta_time;
        let speed = local_velocity.length().min(200.0);
        let target_fade = (speed / 2.0).clamp(0.0, 1.0);
        let fade_step = (dt
            / if target_fade > self.movement_fade {
                0.2
            } else {
                RESTORE_SECONDS
            })
        .clamp(0.0, 1.0);
        self.movement_fade += (target_fade - self.movement_fade) * fade_step;

        if !self.idle_lift {
            return 0.0;
        }

        let mut remaining = dt;
        while remaining > 0.0 {
            if let Some(age) = self.lift_age {
                let until_end = (self.lift_duration - age).max(0.0);
                let consumed = remaining.min(until_end);
                self.lift_age = Some(age + consumed);
                remaining -= consumed;
                if consumed < until_end {
                    break;
                }
                self.lift_age = None;
                self.time_to_next_lift = self.next_wait();
            } else if remaining >= self.time_to_next_lift {
                remaining -= self.time_to_next_lift;
                self.lift_duration = 0.45 + self.random_unit() * 0.4;
                self.lift_accel = gravity_magnitude.abs()
                    * (LIFT_GRAVITY_MIN
                        + self.random_unit() * (LIFT_GRAVITY_MAX - LIFT_GRAVITY_MIN));
                self.lift_age = Some(0.0);
            } else {
                self.time_to_next_lift -= remaining;
                break;
            }
        }

        let Some(age) = self.lift_age else {
            return 0.0;
        };
        let phase = (age / self.lift_duration).clamp(0.0, 1.0);
        let envelope = (std::f32::consts::PI * phase).sin().max(0.0);
        self.lift_accel * envelope * (1.0 - self.movement_fade)
    }

    pub fn idle_lift_acceleration(
        &mut self,
        delta_time: f32,
        local_velocity: Vec3,
        gravity_magnitude: f32,
        inertia_strength: f32,
    ) -> f32 {
        self.update(delta_time, local_velocity, gravity_magnitude)
            * inertia_strength.max(0.0)
            * IDLE_LIFT_MULTIPLIER
    }

    pub fn boost_movement_acceleration(&self, acceleration: Vec3) -> Vec3 {
        if self.movement_boost {
            acceleration * MOVEMENT_FORCE_MULTIPLIER
        } else {
            acceleration
        }
    }

    pub fn reset(&mut self) {
        self.rng = self.rng.rotate_left(17) ^ 0xA076_1D64_78BD_642F;
        self.reset_envelope();
    }

    fn reset_envelope(&mut self) {
        self.time_to_next_lift = 1.2;
        self.lift_age = None;
        self.movement_fade = 0.0;
    }

    fn next_wait(&mut self) -> f32 {
        1.6 + self.random_unit() * 2.2
    }

    fn random_unit(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng as u32) as f32 / u32::MAX as f32
    }
}

#[cfg(test)]
mod tests {
    use super::TailForceState;
    use glam::Mat4;
    use glam::Vec3;

    #[test]
    fn movement_boost_scales_all_axes_and_keeps_disabled_output() {
        let mut state = TailForceState::default();
        assert!(state.idle_lift && state.movement_boost);
        state.set_options(false, false);
        let acceleration = Vec3::new(2.0, -3.0, 4.0);
        assert_eq!(
            state.boost_movement_acceleration(acceleration),
            acceleration
        );

        state.set_options(false, true);
        assert_eq!(
            state.boost_movement_acceleration(acceleration),
            Vec3::new(3.0, -4.5, 6.0)
        );
        assert_eq!(
            state.boost_movement_acceleration(-acceleration),
            Vec3::new(-3.0, 4.5, -6.0)
        );
        state.set_options(false, false);
        assert_eq!(
            state.boost_movement_acceleration(acceleration),
            acceleration
        );
    }

    #[test]
    fn idle_lift_is_twice_the_previous_envelope_and_independent_of_movement_boost() {
        let mut envelope = TailForceState::with_seed(17);
        let mut idle = TailForceState::with_seed(17);
        let mut combined = TailForceState::with_seed(17);
        envelope.set_options(true, false);
        idle.set_options(true, false);
        combined.set_options(true, true);
        let dt = 1.0 / 60.0;
        let mut saw_pulse = false;
        for _ in 0..720 {
            let previous = envelope.update(dt, Vec3::ZERO, 98.0) * 0.5;
            let raised = idle.idle_lift_acceleration(dt, Vec3::ZERO, 98.0, 0.5);
            let both = combined.idle_lift_acceleration(dt, Vec3::ZERO, 98.0, 0.5);
            assert_eq!(raised, previous * 2.0);
            assert_eq!(both, raised);
            saw_pulse |= raised > 0.0;
        }
        assert!(saw_pulse);
    }

    #[test]
    fn idle_lift_is_shared_deterministic_and_stops_cleanly() {
        let mut a = TailForceState::with_seed(17);
        let mut b = TailForceState::with_seed(17);
        a.set_options(true, false);
        b.set_options(true, false);
        let samples_a: Vec<_> = (0..240)
            .map(|_| a.update(1.0 / 60.0, Vec3::ZERO, 98.0))
            .collect();
        let samples_b: Vec<_> = (0..240)
            .map(|_| b.update(1.0 / 60.0, Vec3::ZERO, 98.0))
            .collect();
        assert_eq!(samples_a, samples_b);
        assert!(samples_a.iter().any(|v| *v > 0.0));
        assert!(samples_a.iter().any(|v| *v == 0.0));
    }

    #[test]
    fn movement_fades_idle_lift_and_reset_clears_the_envelope() {
        let mut state = TailForceState::with_seed(31);
        state.set_options(true, false);
        state.lift_age = Some(0.2);
        state.lift_duration = 0.8;
        state.lift_accel = 10.0;
        let idle = state.update(0.01, Vec3::ZERO, 98.0);
        let moving = state.update(0.25, Vec3::new(0.0, 0.0, 20.0), 98.0);
        assert!(idle > 0.0);
        assert_eq!(moving, 0.0);

        state.reset();
        assert!(state.lift_age.is_none());
        assert_eq!(state.movement_fade, 0.0);
    }

    #[test]
    fn pulse_integral_is_stable_across_simulation_rates() {
        fn integrated_lift(fps: u32) -> f32 {
            let mut state = TailForceState::with_seed(47);
            state.set_options(true, false);
            (0..fps * 12)
                .map(|_| state.update(1.0 / fps as f32, Vec3::ZERO, 98.0) / fps as f32)
                .sum()
        }

        let at_30 = integrated_lift(30);
        let at_60 = integrated_lift(60);
        let at_120 = integrated_lift(120);
        assert!(at_30 > 0.0);
        assert!((at_30 - at_60).abs() < at_60 * 0.06);
        assert!((at_60 - at_120).abs() < at_120 * 0.03);
    }

    #[test]
    fn zero_inertia_strength_disables_idle_force() {
        let mut state = TailForceState::with_seed(53);
        state.set_options(true, false);
        state.lift_age = Some(0.2);
        state.lift_accel = 10.0;
        assert_eq!(
            state.idle_lift_acceleration(1.0 / 60.0, Vec3::ZERO, 98.0, 0.0),
            0.0
        );
    }

    #[test]
    fn idle_pulse_moves_a_bullet_suspended_tail_segment() {
        use crate::physics::bullet_ffi::{
            BulletConstraint, BulletRigidBody, BulletShape, BulletWorld, RigidBodyInfo,
        };

        fn run(apply_lift: bool) -> Vec<Vec3> {
            let world = BulletWorld::new(0.0, -98.0, 0.0).expect("Bullet world");
            let anchor_shape = BulletShape::sphere(0.12).expect("anchor shape");
            let upper_shape = BulletShape::sphere(0.16).expect("tail shape");
            let upper_body = BulletRigidBody::new(
                &RigidBodyInfo {
                    mass: 1.0,
                    linear_damping: 0.05,
                    angular_damping: 0.05,
                    friction: 0.4,
                    restitution: 0.0,
                    additional_damping: false,
                    is_kinematic: false,
                    disable_deactivation: true,
                    no_contact_response: false,
                    initial_transform: Mat4::from_translation(Vec3::new(1.0, 1.0, 0.0)),
                },
                &upper_shape,
            )
            .expect("tail body");
            let anchor_body = BulletRigidBody::new(
                &RigidBodyInfo {
                    mass: 0.0,
                    linear_damping: 0.0,
                    angular_damping: 0.0,
                    friction: 0.4,
                    restitution: 0.0,
                    additional_damping: false,
                    is_kinematic: false,
                    disable_deactivation: true,
                    no_contact_response: false,
                    initial_transform: Mat4::from_translation(Vec3::new(0.0, 2.0, 0.0)),
                },
                &anchor_shape,
            )
            .expect("anchor body");
            world.add_rigid_body(&anchor_body, 1, -1);
            world.add_rigid_body(&upper_body, 2, -1);
            let constraint = BulletConstraint::new_6dof_spring(
                &anchor_body,
                &upper_body,
                Mat4::IDENTITY,
                Mat4::from_translation(Vec3::new(-1.0, 1.0, 0.0)),
                true,
            )
            .expect("tail joint");
            constraint.set_linear_lower_limit(0.0, 0.0, 0.0);
            constraint.set_linear_upper_limit(0.0, 0.0, 0.0);
            constraint.set_angular_lower_limit(1.0, 1.0, 1.0);
            constraint.set_angular_upper_limit(-1.0, -1.0, -1.0);
            world.add_constraint(&constraint, true);

            let mut state = TailForceState::with_seed(61);
            state.set_options(apply_lift, false);
            let dt = 1.0 / 60.0;
            let mut positions = Vec::with_capacity(6 * 60);
            for _ in 0..(6 * 60) {
                let lift = state.idle_lift_acceleration(dt, Vec3::ZERO, 98.0, 0.5);
                if lift > 0.0 {
                    upper_body.apply_central_force(0.0, lift, 0.0);
                }
                world.step(dt, 1, dt);
                positions.push(upper_body.get_position());
            }
            positions
        }

        let baseline = run(false);
        let lifted = run(true);
        assert!(baseline
            .iter()
            .chain(&lifted)
            .all(|position| position.is_finite()));
        let max_trajectory_delta = baseline
            .iter()
            .zip(&lifted)
            .map(|(a, b)| a.distance(*b))
            .fold(0.0_f32, f32::max);
        let max_tail_displacement = lifted
            .iter()
            .map(|position| position.distance(Vec3::new(1.0, 1.0, 0.0)))
            .fold(0.0_f32, f32::max);
        assert!(max_tail_displacement > 0.05, "悬挂尾段应实际摆动");
        assert!(
            max_trajectory_delta > 0.01,
            "闲置脉冲应改变 Bullet 实际位置轨迹"
        );
    }
}
