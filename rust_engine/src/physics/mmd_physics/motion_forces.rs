//! 模型移动速度惯性与尾巴气动力。

use super::super::config::get_config;
use super::is_skirt_body_name;
use super::{MMDPhysics, PhysicsMode};
use glam::{Mat3, Mat4, Vec3};

#[cfg(test)]
mod tests;

impl MMDPhysics {
    /// 同步运动学刚体并传递模型移动速度（实现惯性）
    ///
    /// 原理：物理在模型局部空间运行，角色世界移动对物理不可见。
    /// 第一步：同步 FollowBone 刚体到骨骼位置。
    /// 第二步：计算模型世界速度，转换到物理空间后取反，
    ///        对动态刚体施加 F = m * (-v_local) / dt，
    ///        产生与移动方向相反的惯性力（头发/裙子自然后拽）。
    pub fn sync_bodies_with_model_velocity(
        &mut self,
        bone_transforms: &[Mat4],
        delta_time: f32,
        model_transform: Mat4,
    ) {
        let config = get_config();
        if !delta_time.is_finite() || delta_time <= 0.0 {
            self.reset_motion_history();
            self.sync_bodies(bone_transforms);
            return;
        }

        let dt = delta_time.max(0.001);
        let curr_pos = model_transform.w_axis.truncate();

        if !curr_pos.is_finite() {
            self.reset_motion_history();
            self.sync_bodies(bone_transforms);
            return;
        }

        // 计算模型世界速度，并使用低通滤波平滑以消除逐帧离散渲染跳步
        let raw_velocity = if let Some(prev_pos) = self.prev_model_position {
            (curr_pos - prev_pos) / dt
        } else {
            Vec3::ZERO
        };
        self.prev_model_position = Some(curr_pos);

        let smooth_factor = (dt / 0.08).clamp(0.0, 1.0);
        self.smoothed_model_velocity = self
            .smoothed_model_velocity
            .lerp(raw_velocity, smooth_factor);
        let model_velocity = self.smoothed_model_velocity;

        // 第一步：同步运动学刚体位置
        if config.debug_log {
            self.update_debug_body_targets(bone_transforms, delta_time);
        }
        self.sync_bodies(bone_transforms);

        // 第二步：给动态刚体施加惯性力或独立的尾巴轻抬力。
        // 钳制世界速度（20 blocks/s = 200 MMD units/s 覆盖疾跑和速度药水，超出视为传送）。
        let speed_sq = model_velocity.length_squared();
        let max_speed = 200.0_f32;
        let world_vel = if speed_sq > max_speed * max_speed {
            model_velocity * (max_speed / speed_sq.sqrt())
        } else {
            model_velocity
        };

        // 世界速度 → 模型局部空间（R^T * v_world）。
        let rot_inv = Mat3::from_mat4(model_transform).transpose();
        let local_vel = rot_inv * world_vel;
        let movement_active =
            config.inertia_strength > 0.0 && model_velocity.length_squared() > 1e-6;
        let idle_lift = self.tail_forces.idle_lift_acceleration(
            dt,
            local_vel,
            config.gravity_y.abs(),
            config.inertia_strength,
        );
        if movement_active || idle_lift > 0.0 {
            // 在 MMD Bullet 物理空间中：+X 为右侧，+Y 为上方，+Z 为后方（尾巴方向），-Z 为前方。
            // 当角色向前移动 (local_vel.z > 0) 时，惯性拖拽力应将头发和尾巴向后拉拽 (+Z 方向)；
            // 当角色向右侧移 (local_vel.x > 0) 时，惯性力向左拉拽 (-X 方向)；
            // 当角色向上跳跃 (local_vel.y > 0) 时，惯性力向下压 (-Y 方向)。
            let inertia_accel = if movement_active {
                Vec3::new(
                    -local_vel.x * config.inertia_strength,
                    -local_vel.y * config.inertia_strength,
                    local_vel.z * config.inertia_strength,
                )
            } else {
                Vec3::ZERO
            };

            // 最大加速度 = max_linear_velocity * physics_fps
            let max_accel = config.max_linear_velocity * self.fps;

            // F = m * a_drag（空气阻力与平滑加速度，不除以 dt，避免帧率抖动和过度爆炸）
            for rb_data in &self.rigid_bodies {
                if rb_data.physics_mode == PhysicsMode::FollowBone {
                    continue;
                }
                if let Some(ref body) = rb_data.bullet_body {
                    let mass = body.get_mass();
                    if mass > 0.0 {
                        let is_skirt = is_skirt_body_name(&rb_data.name);
                        let use_new_tail_rules =
                            self.tail_forces.idle_lift || self.tail_forces.movement_boost;
                        let is_tail = if use_new_tail_rules {
                            rb_data.is_tail_dynamic
                        } else {
                            rb_data.is_tail_dynamic_legacy
                        };

                        let accel = if is_tail {
                            // 尾巴作为柔性长摆锤，在跑动时需要明显的阻尼滞后（Damped Tracking）和迎风向后扬起效果。
                            // 线性风阻 + 迎风动压阻力 + 空气升力，使尾巴在疾跑时自然向后上方扬起（~45°），并在停步时平滑摆回。
                            let mut tail_accel = if movement_active {
                                let forward_speed = local_vel.z.max(0.0);
                                let quad_drag =
                                    0.025 * forward_speed * forward_speed * config.inertia_strength;
                                let lift_accel =
                                    0.012 * forward_speed * forward_speed * config.inertia_strength;
                                // 全向倍率作用于合成受力，平方风阻也统一增强一次。
                                self.tail_forces.boost_movement_acceleration(Vec3::new(
                                    -local_vel.x * (2.8 * config.inertia_strength),
                                    -local_vel.y * (1.8 * config.inertia_strength) + lift_accel,
                                    (local_vel.z * 2.8 + quad_drag) * config.inertia_strength,
                                ))
                            } else {
                                Vec3::ZERO
                            };
                            // 近竖直尾链需要后向分量形成抬起力矩，纯向上力只会卸载重力。
                            tail_accel += Vec3::new(0.0, idle_lift, idle_lift);
                            tail_accel
                        } else if is_skirt {
                            // 裙摆是环绕身体的环状结构，惯性响应系数降低为 0.15，保持裙摆优雅形态，防止跑动时向上翻起
                            inertia_accel * 0.15
                        } else {
                            // 头发与饰品等常规动态部位
                            inertia_accel
                        };

                        if !movement_active && !is_tail {
                            continue;
                        }
                        let mut force = accel * mass;
                        let max_force = max_accel * mass;
                        let force_sq = force.length_squared();
                        if force_sq > max_force * max_force {
                            force *= max_force / force_sq.sqrt();
                        }
                        body.apply_central_force(force.x, force.y, force.z);
                    }
                }
            }
        }
    }
}
