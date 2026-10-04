use super::*;

use glam::{Mat4, Vec3};
use mmd::pmx::rigid_body::{RigidBody as PmxRigidBody, RigidBodyMode, RigidBodyShape};

fn pmx_body(
    name: &str,
    universal_name: &str,
    bone_index: i32,
    group: u8,
    un_collision_group_flag: u16,
    mode: RigidBodyMode,
    radius: f32,
    position: Vec3,
) -> PmxRigidBody {
    PmxRigidBody {
        local_name: name.to_owned(),
        universal_name: universal_name.to_owned(),
        bone_index,
        group,
        un_collision_group_flag,
        shape: RigidBodyShape::Sphere,
        size: [radius, 0.0, 0.0],
        position: position.to_array(),
        rotation: [0.0; 3],
        mass: 1.0,
        move_attenuation: 0.0,
        rotation_attenuation: 0.0,
        repulsion: 0.0,
        friction: 0.0,
        mode,
    }
}

fn chest_physics(dynamic_position: Vec3) -> MMDPhysics {
    let mut physics = MMDPhysics::new().expect("Bullet world should be available");
    let bodies = [
        pmx_body(
            "上半身2躯干壳",
            "",
            0,
            0,
            0,
            RigidBodyMode::Static,
            1.0,
            Vec3::ZERO,
        ),
        pmx_body(
            "左胸下",
            "",
            1,
            3,
            0,
            RigidBodyMode::Dynamic,
            0.1,
            dynamic_position,
        ),
    ];
    physics.build_physics(&bodies, &[], &[Mat4::IDENTITY; 2]);
    physics.world.set_gravity(0.0, 0.0, 0.0);
    physics
}

fn chest_bones() -> (Vec<String>, Vec<i32>) {
    (vec!["上半身2".into(), "左胸下".into()], vec![-1, 0])
}

#[test]
fn stable_and_relaxed_refresh_embedded_contacts_before_they_can_push() {
    let (bone_names, bone_parents) = chest_bones();
    for mode in [
        CollisionStabilityMode::Stable,
        CollisionStabilityMode::Relaxed,
    ] {
        let mut physics = chest_physics(Vec3::ZERO);
        physics.collision_stability_mode = mode;
        physics.world.detect_collisions();
        assert_eq!(physics.world.contact_manifolds().len(), 1);

        assert_eq!(
            physics.configure_embedded_body_contacts(&bone_names, &bone_parents),
            1
        );
        assert!(physics.world.contact_manifolds().is_empty());

        let body = physics.rigid_bodies[1]
            .bullet_body
            .as_ref()
            .expect("dynamic chest body should exist");
        let before = body.get_simulation_transform().w_axis.truncate();
        physics.world.step(1.0 / 60.0, 1, 1.0 / 60.0);
        let after = body.get_simulation_transform().w_axis.truncate();
        assert!(
            (after - before).length() < 1e-4,
            "mode {mode:?} retained a stale contact"
        );
    }
}

#[test]
fn strict_mode_keeps_the_authored_body_contact() {
    let (bone_names, bone_parents) = chest_bones();
    let mut physics = chest_physics(Vec3::ZERO);
    physics.collision_stability_mode = CollisionStabilityMode::Strict;
    physics.world.detect_collisions();
    assert_eq!(physics.world.contact_manifolds().len(), 1);

    assert_eq!(
        physics.configure_embedded_body_contacts(&bone_names, &bone_parents),
        0
    );
    assert_eq!(physics.world.contact_manifolds().len(), 1);
    let shell = physics.rigid_bodies[0].bullet_body.as_ref().unwrap();
    let chest = physics.rigid_bodies[1].bullet_body.as_ref().unwrap();
    assert!(shell.check_collide_with(chest));
}

#[test]
fn nonembedded_chest_remains_collidable() {
    let (bone_names, bone_parents) = chest_bones();
    let mut physics = chest_physics(Vec3::new(2.0, 0.0, 0.0));
    physics.world.detect_collisions();
    assert!(physics.world.contact_manifolds().is_empty());

    assert_eq!(
        physics.configure_embedded_body_contacts(&bone_names, &bone_parents),
        0
    );
    let shell = physics.rigid_bodies[0].bullet_body.as_ref().unwrap();
    let chest = physics.rigid_bodies[1].bullet_body.as_ref().unwrap();
    assert!(shell.check_collide_with(chest));
}

#[test]
fn english_skirt_tassel_keeps_pmx_exclusion_with_hip_shell_present() {
    let mut physics = MMDPhysics::new().expect("Bullet world should be available");
    let bodies = [
        pmx_body("腰壳", "", 0, 0, 0, RigidBodyMode::Static, 1.0, Vec3::ZERO),
        pmx_body(
            "胸前穗_0_1",
            "Skirt_0_1",
            1,
            14,
            0xBFFF,
            RigidBodyMode::Dynamic,
            0.1,
            Vec3::ZERO,
        ),
    ];
    let (bone_names, bone_parents) = chest_bones();
    physics.build_physics(&bodies, &[], &[Mat4::IDENTITY; 2]);
    physics.world.set_gravity(0.0, 0.0, 0.0);
    physics.collision_stability_mode = CollisionStabilityMode::Stable;
    physics.world.detect_collisions();

    assert!(physics.world.contact_manifolds().is_empty());
    assert_eq!(physics.rigid_bodies[1].collision_mask, 0x4000);
    assert_eq!(
        physics.configure_embedded_body_contacts(&bone_names, &bone_parents),
        0
    );
    assert!(physics.world.contact_manifolds().is_empty());
    assert_eq!(physics.rigid_bodies[1].collision_mask, 0x4000);
}
