use super::*;
use crate::skeleton::BoneLink;
use crate::vr::{VrTrackedPose, XR_TO_MODEL_SCALE};
use crate::vrm_runtime::{ArmIkHandCalibration, BodyTrackingCalibration};

#[test]
fn hand_matrix_should_prefer_explicit_attachment_over_dummy_and_wrist() {
    let mut model = MmdModel::new();
    add_test_bone(&mut model, "右手首", Vec3::new(1.0, 0.0, 0.0));
    add_test_bone(&mut model, "ダミー.R", Vec3::new(2.0, 0.0, 0.0));
    add_test_bone(&mut model, "Hand_Attach_R", Vec3::new(3.0, 0.0, 0.0));
    model.bone_manager.build_hierarchy();

    assert_eq!(
        model.get_right_hand_matrix().transform_point3(Vec3::ZERO),
        Vec3::new(3.0, 0.0, 0.0)
    );
}

#[test]
fn hand_matrix_should_fall_back_to_dummy_then_wrist() {
    let mut dummy_model = MmdModel::new();
    add_test_bone(&mut dummy_model, "右手首", Vec3::new(1.0, 0.0, 0.0));
    add_test_bone(&mut dummy_model, "ダミー.R", Vec3::new(2.0, 0.0, 0.0));
    dummy_model.bone_manager.build_hierarchy();

    let mut wrist_model = MmdModel::new();
    add_test_bone(&mut wrist_model, "右手首", Vec3::new(1.0, 0.0, 0.0));
    wrist_model.bone_manager.build_hierarchy();

    assert_eq!(
        dummy_model
            .get_right_hand_matrix()
            .transform_point3(Vec3::ZERO),
        Vec3::new(2.0, 0.0, 0.0)
    );
    assert_eq!(
        wrist_model
            .get_right_hand_matrix()
            .transform_point3(Vec3::ZERO),
        Vec3::new(1.0, 0.0, 0.0)
    );
}

#[test]
fn hand_matrix_should_accept_common_dummy_separators() {
    for name in ["ダミー_R", "ダミー.R", "ダミー R"] {
        let mut model = MmdModel::new();
        add_test_bone(&mut model, "右手首", Vec3::new(1.0, 0.0, 0.0));
        add_test_bone(&mut model, name, Vec3::new(2.0, 0.0, 0.0));
        model.bone_manager.build_hierarchy();

        assert_eq!(
            model.get_right_hand_matrix().transform_point3(Vec3::ZERO),
            Vec3::new(2.0, 0.0, 0.0),
            "右手挂点应兼容 {name}"
        );
    }
}

#[test]
fn hand_matrix_should_not_use_the_opposite_dummy_side() {
    let mut model = MmdModel::new();
    add_test_bone(&mut model, "右手首", Vec3::new(1.0, 0.0, 0.0));
    add_test_bone(&mut model, "ダミー_L", Vec3::new(2.0, 0.0, 0.0));
    model.bone_manager.build_hierarchy();

    assert_eq!(
        model.get_right_hand_matrix().transform_point3(Vec3::ZERO),
        Vec3::new(1.0, 0.0, 0.0)
    );
}

fn add_test_bone(model: &mut MmdModel, name: &str, position: Vec3) {
    let mut bone = BoneLink::new(name.to_string());
    bone.initial_position = position;
    model.bone_manager.add_bone(bone);
}

#[test]
fn set_first_person_mode_should_restore_user_material_visibility() {
    let mut model = make_material_visibility_test_model();
    model.set_material_visible(2, false);

    model.set_first_person_mode(true);

    assert!(model.is_material_visible(0));
    assert!(!model.is_material_visible(1));
    assert!(!model.is_material_visible(2));
    assert_eq!(
        model.user_material_visibility_snapshot(),
        vec![true, true, false]
    );

    model.set_first_person_mode(false);

    assert!(model.is_material_visible(0));
    assert!(model.is_material_visible(1));
    assert!(!model.is_material_visible(2));
    assert_eq!(
        model.user_material_visibility_snapshot(),
        vec![true, true, false]
    );
}

#[test]
fn first_person_mesh_should_replace_heuristic_material_mask() {
    let mut model = make_material_visibility_test_model();
    model.first_person_mesh = Some(FirstPersonMesh {
        indices: vec![0, 1, 2],
        submeshes: vec![SubMesh::new(0, 3, 1)],
        triangle_classes: vec![],
        dynamic_vertices: vec![],
    });

    model.set_first_person_mode(true);

    // 头部材质必须保留，实际可见三角形由第一人称 EBO 控制。
    assert!(model.is_material_visible(1));
}

#[test]
fn camera_anchor_should_prefer_animated_eye_pair_over_combined_eye_bone() {
    let mut model = make_camera_anchor_test_model(true, true, true);
    model
        .bone_manager
        .get_bone_mut(2)
        .unwrap()
        .animation_translate = Vec3::new(0.2, -0.1, 0.5);
    model
        .bone_manager
        .get_bone_mut(3)
        .unwrap()
        .animation_translate = Vec3::new(0.4, 0.3, 0.1);
    model.bone_manager.update_transforms(false);
    model.bone_manager.update_skinning_matrices();

    let anchor = model.get_first_person_camera_anchor_position();
    let legacy_eye = model.get_eye_bone_animated_position();

    assert_eq!(anchor, Vec3::new(0.3, 17.7, 0.6));
    assert_eq!(legacy_eye, Vec3::new(4.0, 18.0, -2.0));
}

#[test]
fn camera_anchor_should_follow_rendered_skinning_transition() {
    let mut model = make_camera_anchor_test_model(false, true, true);
    model
        .bone_manager
        .get_bone_mut(1)
        .unwrap()
        .animation_translate = Vec3::new(0.0, 0.0, -1.0);
    model
        .bone_manager
        .get_bone_mut(2)
        .unwrap()
        .animation_translate = Vec3::new(0.0, 0.0, -0.6);
    model.bone_manager.update_transforms(false);

    // 模拟 Sprint -> Idle 过渡中，网格仍只完成部分回正。
    model
        .bone_manager
        .set_skinning_matrix(1, Mat4::from_translation(Vec3::new(0.0, 0.0, -0.25)));
    model
        .bone_manager
        .set_skinning_matrix(2, Mat4::from_translation(Vec3::new(0.0, 0.0, -0.15)));

    let anchor = model.get_first_person_camera_anchor_position();
    let unblended_eye = model.get_eye_bone_animated_position();

    assert_eq!(anchor, Vec3::new(0.0, 17.6, 0.1));
    assert_eq!(unblended_eye, Vec3::new(0.0, 17.6, -0.5));
}
#[test]
fn camera_anchor_should_keep_single_eye_and_missing_eye_fallbacks() {
    let mut single_eye_model = make_camera_anchor_test_model(false, true, false);
    let mut missing_eye_model = make_camera_anchor_test_model(false, false, false);

    assert_eq!(
        single_eye_model.get_first_person_camera_anchor_position(),
        Vec3::new(-0.3, 17.5, 0.2)
    );
    assert_eq!(
        missing_eye_model.get_first_person_camera_anchor_position(),
        Vec3::ZERO
    );
}

#[test]
fn set_vr_tracking_data_should_preserve_arm_and_body_calibration() {
    let mut model = MmdModel::new();
    let arm_ik_calibration = ArmIkCalibration {
        left: ArmIkHandCalibration {
            wrist_offset_model: Vec3::new(1.0, 2.0, 3.0),
            wrist_rotation_offset_model: Quat::from_rotation_z(0.1),
        },
        right: ArmIkHandCalibration {
            wrist_offset_model: Vec3::new(-1.0, -2.0, -3.0),
            wrist_rotation_offset_model: Quat::from_rotation_z(-0.2),
        },
        forearm_twist_ratio: 0.75,
        hand_face_flip: false,
    };
    let body_calibration = BodyTrackingCalibration {
        head_rest_anchor_model: Vec3::new(0.0, 17.0, 0.0),
        shoulder_width_model: XR_TO_MODEL_SCALE * 0.3,
        shoulder_depth_model: XR_TO_MODEL_SCALE * 0.08,
        body_yaw_follow_gain: 0.65,
        horizontal_translation_follow_gain: 0.9,
        vertical_translation_follow_gain: 0.95,
        body_translation_clamp_model: 2.0,
        shoulder_follow_gain: 0.45,
    };
    model.set_vr_tracking_frame(Some(VrTrackingFrame {
        head: VrTrackedPose::default(),
        right_palm: VrTrackedPose::default(),
        left_palm: VrTrackedPose::default(),
        arm_ik_calibration,
        body_calibration,
    }));

    model.set_vr_tracking_data(&[0.0; 21]);

    let frame = model
        .vr_tracking_frame
        .expect("tracking frame should exist");
    assert_eq!(
        frame.arm_ik_calibration.left.wrist_offset_model,
        arm_ik_calibration.left.wrist_offset_model
    );
    assert_eq!(
        frame.arm_ik_calibration.right.wrist_offset_model,
        arm_ik_calibration.right.wrist_offset_model
    );
    let left_similarity = frame
        .arm_ik_calibration
        .left
        .wrist_rotation_offset_model
        .dot(arm_ik_calibration.left.wrist_rotation_offset_model)
        .abs();
    assert!(left_similarity > 1.0 - 1e-6);
    let right_similarity = frame
        .arm_ik_calibration
        .right
        .wrist_rotation_offset_model
        .dot(arm_ik_calibration.right.wrist_rotation_offset_model)
        .abs();
    assert!(right_similarity > 1.0 - 1e-6);
    assert_eq!(
        frame.arm_ik_calibration.forearm_twist_ratio,
        arm_ik_calibration.forearm_twist_ratio
    );
    assert_eq!(
        frame.body_calibration.head_rest_anchor_model,
        body_calibration.head_rest_anchor_model
    );
    assert_eq!(
        frame.body_calibration.shoulder_width_model,
        body_calibration.shoulder_width_model
    );
}

fn make_material_visibility_test_model() -> MmdModel {
    let mut model = MmdModel::new();
    model.materials = vec![
        MmdMaterial::default(),
        MmdMaterial::default(),
        MmdMaterial::default(),
    ];
    model.submeshes = vec![
        SubMesh::new(0, 3, 0),
        SubMesh::new(3, 3, 1),
        SubMesh::new(6, 3, 2),
    ];
    model.head_submesh_flags = vec![false, true, false];
    model.head_detection_initialized = true;
    model
        .bone_manager
        .add_bone(BoneLink::new("Head".to_string()));
    model.init_material_visibility();
    model
}

fn make_camera_anchor_test_model(
    has_combined_eye: bool,
    has_left_eye: bool,
    has_right_eye: bool,
) -> MmdModel {
    let mut model = MmdModel::new();

    let mut head = BoneLink::new("Head".to_string());
    head.initial_position = Vec3::new(0.0, 16.0, 0.0);
    model.bone_manager.add_bone(head);

    if has_combined_eye {
        let mut eye = BoneLink::new("両目".to_string());
        eye.initial_position = Vec3::new(4.0, 18.0, -2.0);
        model.bone_manager.add_bone(eye);
    }
    if has_left_eye {
        let mut left_eye = BoneLink::new("左目".to_string());
        left_eye.initial_position = Vec3::new(-0.3, 17.5, 0.2);
        model.bone_manager.add_bone(left_eye);
    }
    if has_right_eye {
        let mut right_eye = BoneLink::new("右目".to_string());
        right_eye.initial_position = Vec3::new(0.3, 17.7, 0.4);
        model.bone_manager.add_bone(right_eye);
    }

    model.bone_manager.build_hierarchy();
    model
}
