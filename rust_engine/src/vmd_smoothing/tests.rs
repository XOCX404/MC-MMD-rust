use super::{smooth_bytes, BoneSelection, SmoothingOptions, VmdDocument};
use crate::animation::VmdFile;
use glam::Vec3;

const SPRINT: &[u8] =
    include_bytes!("../../../common/src/main/resources/assets/mmdskin/default_anim/sprint.vmd");
const WALK: &[u8] =
    include_bytes!("../../../common/src/main/resources/assets/mmdskin/default_anim/walk.vmd");

fn options(selection: BoneSelection) -> SmoothingOptions {
    SmoothingOptions {
        strength: 1.0,
        radius: 1,
        looped: false,
        selection,
    }
}

fn record(name: &str, frame: u32, x: f32, q: [f32; 4]) -> Vec<u8> {
    let mut bytes = vec![0_u8; 111];
    let (name, _, _) = encoding_rs::SHIFT_JIS.encode(name);
    bytes[..name.len().min(15)].copy_from_slice(&name[..name.len().min(15)]);
    bytes[15..19].copy_from_slice(&frame.to_le_bytes());
    bytes[19..23].copy_from_slice(&x.to_le_bytes());
    bytes[43..47].copy_from_slice(&q[3].to_le_bytes());
    bytes[31..35].copy_from_slice(&q[0].to_le_bytes());
    bytes[35..39].copy_from_slice(&q[1].to_le_bytes());
    bytes[39..43].copy_from_slice(&q[2].to_le_bytes());
    for channel in 0..4 {
        for (index, value) in [20, 20, 107, 107].iter().enumerate() {
            bytes[47 + index * 4 + channel] = *value;
        }
    }
    bytes
}

fn vmd(records: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![0; 50];
    bytes[..25].copy_from_slice(b"Vocaloid Motion Data 0002");
    bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for record in records {
        bytes.extend_from_slice(record);
    }
    for _ in 0..5 {
        bytes.extend_from_slice(&0_u32.to_le_bytes());
    }
    bytes
}

fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn output_records(bytes: &[u8]) -> Vec<Vec<u8>> {
    let count = u32::from_le_bytes(bytes[50..54].try_into().unwrap()) as usize;
    (0..count)
        .map(|index| {
            let start = 54 + index * 111;
            bytes[start..start + 111].to_vec()
        })
        .collect()
}

fn records_for(bytes: &[u8], name: &str) -> Vec<Vec<u8>> {
    output_records(bytes)
        .into_iter()
        .filter(|record| {
            let raw = &record[..15];
            let end = raw.iter().position(|byte| *byte == 0).unwrap_or(15);
            let (decoded, _, _) = encoding_rs::SHIFT_JIS.decode(&raw[..end]);
            decoded == name
        })
        .collect()
}

fn raw_position(record: &[u8]) -> Vec3 {
    Vec3::new(f32_at(record, 19), f32_at(record, 23), -f32_at(record, 27))
}

fn second_difference_energy(positions: &[Vec3]) -> f32 {
    positions
        .windows(3)
        .map(|triple| (triple[2] - triple[1] * 2.0 + triple[0]).length())
        .sum()
}

fn assert_extrema_and_range_preserved(original: &[Vec3], output: &[Vec3]) {
    assert_eq!(original.len(), output.len());
    for axis in 0..3 {
        let minimum = original
            .iter()
            .map(|position| position[axis])
            .fold(f32::INFINITY, f32::min);
        let maximum = original
            .iter()
            .map(|position| position[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        let tolerance = minimum.abs().max(maximum.abs()).max(1.0) * 1.0e-5;
        let output_min = output
            .iter()
            .map(|position| position[axis])
            .fold(f32::INFINITY, f32::min);
        let output_max = output
            .iter()
            .map(|position| position[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((output_min - minimum).abs() <= tolerance);
        assert!((output_max - maximum).abs() <= tolerance);
        for (frame, position) in original.iter().enumerate() {
            if (position[axis] - minimum).abs() <= tolerance
                || (position[axis] - maximum).abs() <= tolerance
            {
                assert!((output[frame][axis] - position[axis]).abs() <= tolerance);
            }
        }
    }
}

#[test]
fn zero_strength_returns_exact_original_bytes() {
    let mut source = vmd(&[record("spine", 0, 3.0, [0.0, 0.0, 0.0, -1.0])]);
    source.extend_from_slice(b"opaque tail");
    let mut opts = options(BoneSelection::AllExceptIk);
    opts.strength = 0.0;
    let result = smooth_bytes(&source, &opts).unwrap();
    assert_eq!(result.bytes, source);
    assert_eq!(result.report.original_keys, result.report.output_keys);
}

#[test]
fn zero_strength_for_foot_ik_returns_exact_original_bytes() {
    let source = vmd(&[
        record("左足ＩＫ", 0, -3.0, [0.0, 0.0, 0.0, -1.0]),
        record("左足ＩＫ", 1, 8.0, [0.0, 0.0, 0.0, -1.0]),
    ]);
    let mut opts = options(BoneSelection::Named(vec!["左足ＩＫ".into()]));
    opts.strength = 0.0;
    assert_eq!(smooth_bytes(&source, &opts).unwrap().bytes, source);
}

#[test]
fn constant_track_does_not_drift() {
    let records = (0..5)
        .map(|frame| record("spine", frame, 2.5, [0.0, 0.0, 0.0, -1.0]))
        .collect::<Vec<_>>();
    let result = smooth_bytes(&vmd(&records), &options(BoneSelection::AllExceptIk)).unwrap();
    let poses = output_records(&result.bytes);
    assert_eq!(poses.len(), 5);
    for pose in poses {
        assert!((f32_at(&pose, 19) - 2.5).abs() < 1e-6);
    }
}

#[test]
fn long_static_track_keeps_sparse_original_records() {
    let source = vmd(&[
        record("spine", 0, 2.5, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 9000, 2.5, [0.0, 0.0, 0.0, -1.0]),
    ]);
    let result = smooth_bytes(&source, &options(BoneSelection::AllExceptIk)).unwrap();
    assert_eq!(result.bytes, source);
    assert_eq!(result.report.original_keys, 2);
    assert_eq!(result.report.output_keys, 2);
}

#[test]
fn symmetric_filter_reduces_a_translation_spike() {
    let xs = [0.0, 0.0, 10.0, 0.0, 0.0];
    let records = xs
        .iter()
        .enumerate()
        .map(|(frame, x)| record("spine", frame as u32, *x, [0.0, 0.0, 0.0, -1.0]))
        .collect::<Vec<_>>();
    let result = smooth_bytes(&vmd(&records), &options(BoneSelection::AllExceptIk)).unwrap();
    let poses = output_records(&result.bytes);
    assert!(f32_at(&poses[2], 19) < 10.0);
    assert!(f32_at(&poses[2], 19) > 0.0);
}

#[test]
fn non_looped_processing_keeps_endpoint_poses() {
    let records = [
        record("spine", 0, -2.0, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 1, 0.0, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 2, 5.0, [0.0, 0.0, 0.0, -1.0]),
    ];
    let result = smooth_bytes(&vmd(&records), &options(BoneSelection::AllExceptIk)).unwrap();
    let poses = output_records(&result.bytes);
    assert_eq!(f32_at(&poses[0], 19), -2.0);
    assert_eq!(f32_at(&poses[2], 19), 5.0);
}

#[test]
fn non_looped_processing_keeps_the_track_start_frame() {
    let records = [
        record("spine", 3, 1.0, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 4, 2.0, [0.0, 0.0, 0.0, -1.0]),
    ];
    let result = smooth_bytes(&vmd(&records), &options(BoneSelection::AllExceptIk)).unwrap();
    let poses = output_records(&result.bytes);
    assert_eq!(u32::from_le_bytes(poses[0][15..19].try_into().unwrap()), 3);
    assert_eq!(poses.len(), 2);
}

#[test]
fn two_frame_track_keeps_both_endpoints_in_both_modes() {
    let records = [
        record("spine", 0, -3.0, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 1, 8.0, [0.0, 0.0, 0.0, -1.0]),
    ];
    for looped in [false, true] {
        let mut opts = options(BoneSelection::Named(vec!["spine".into()]));
        opts.looped = looped;
        let poses = records_for(&smooth_bytes(&vmd(&records), &opts).unwrap().bytes, "spine");
        assert_eq!(poses.len(), 2);
        assert_eq!(raw_position(&poses[0]), raw_position(&records[0]));
        assert_eq!(raw_position(&poses[1]), raw_position(&records[1]));
    }
}

#[test]
fn upper_body_preset_leaves_root_tracks_for_explicit_selection() {
    assert!(!super::filter::selected(
        "センター",
        &BoneSelection::UpperBody
    ));
    assert!(!super::filter::selected(
        "pelvis",
        &BoneSelection::UpperBody
    ));
    assert!(super::filter::selected("上半身", &BoneSelection::UpperBody));
}

#[test]
fn foot_ik_classifier_covers_english_japanese_and_chinese_names() {
    for name in [
        "left foot IK",
        "right toe ik",
        "ankle_ik_l",
        "左足ＩＫ",
        "右つま先ＩＫ",
        "左足首IK",
        "右趾ＩＫ",
        "左脚IK",
        "左脚趾IK",
    ] {
        assert!(super::filter::is_foot_ik(name), "{name}");
    }
    for name in ["左ひじＩＫ", "right wrist ik", "左足", "leg IK"] {
        assert!(!super::filter::is_foot_ik(name), "{name}");
    }
}

#[test]
fn looped_foot_compensation_preserves_seam_and_stride_extrema() {
    let values = [0.0, 2.0, 5.0, 2.0, 0.0];
    let records = values
        .iter()
        .enumerate()
        .map(|(frame, value)| record("左足ＩＫ", frame as u32, *value, [0.0, 0.0, 0.0, -1.0]))
        .collect::<Vec<_>>();
    let mut opts = options(BoneSelection::Named(vec!["左足ＩＫ".into()]));
    opts.looped = true;
    let result = smooth_bytes(&vmd(&records), &opts).unwrap();
    let poses = records_for(&result.bytes, "左足ＩＫ");
    let original = records
        .iter()
        .map(|record| raw_position(record))
        .collect::<Vec<_>>();
    let output = poses
        .iter()
        .map(|record| raw_position(record))
        .collect::<Vec<_>>();
    assert_extrema_and_range_preserved(&original, &output);
    assert!(
        (output[1].x - output[3].x).abs() < 1e-5,
        "循环接缝中央差分应为零"
    );
}

#[test]
fn looped_processing_closes_a_cyclic_pose() {
    let xs = [12.0, 13.0, 12.0, 11.0, 12.0];
    let records = xs
        .iter()
        .enumerate()
        .map(|(frame, x)| record("spine", frame as u32, *x, [0.0, 0.0, 0.0, -1.0]))
        .collect::<Vec<_>>();
    let mut opts = options(BoneSelection::AllExceptIk);
    opts.looped = true;
    let result = smooth_bytes(&vmd(&records), &opts).unwrap();
    let poses = output_records(&result.bytes);
    assert_eq!(poses.len(), 5);
    let mean = poses.iter().map(|pose| f32_at(pose, 19)).sum::<f32>() / poses.len() as f32;
    assert!((mean - 12.0).abs() < 1e-5);
    assert!((f32_at(&poses[0], 19) - f32_at(&poses[4], 19)).abs() < 1e-5);
    let first_step = f32_at(&poses[1], 19) - f32_at(&poses[0], 19);
    let seam_step = f32_at(&poses[0], 19) - f32_at(&poses[3], 19);
    assert!((first_step - seam_step).abs() < 1e-5);
}

#[test]
fn looped_filter_keeps_asymmetric_track_endpoint_transforms() {
    let q = [0.0, 0.0, (0.6_f32 / 2.0).sin(), -(0.6_f32 / 2.0).cos()];
    let records = [
        record("spine", 0, -1.0, q),
        record("spine", 1, 3.0, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 2, 5.0, [0.2, 0.0, 0.0, -0.98]),
        record("spine", 3, -4.0, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 4, -1.0, q),
    ];
    let mut opts = options(BoneSelection::Named(vec!["spine".into()]));
    opts.looped = true;
    opts.radius = 2;
    let result = smooth_bytes(&vmd(&records), &opts).unwrap();
    let poses = records_for(&result.bytes, "spine");
    assert_eq!(poses.len(), 5);
    assert_eq!(raw_position(&poses[0]), raw_position(&records[0]));
    assert_eq!(raw_position(&poses[4]), raw_position(&records[4]));
    for (pose, source) in [(&poses[0], &records[0]), (&poses[4], &records[4])] {
        let rotation_fields = [31, 35, 39, 43];
        for offset in rotation_fields {
            assert!((f32_at(pose, offset) - f32_at(source, offset)).abs() < 1e-5);
        }
    }
}

#[test]
fn incomplete_track_in_loop_mode_keeps_its_original_frame_range() {
    let records = [
        record("spine", 5, 0.0, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 7, 4.0, [0.0, 0.0, 0.0, -1.0]),
        record("other", 12, 0.0, [0.0, 0.0, 0.0, -1.0]),
    ];
    let mut opts = options(BoneSelection::Named(vec!["spine".into()]));
    opts.looped = true;
    let result = smooth_bytes(&vmd(&records), &opts).unwrap();
    let poses = records_for(&result.bytes, "spine");
    assert_eq!(poses.len(), 3);
    assert_eq!(u32::from_le_bytes(poses[0][15..19].try_into().unwrap()), 5);
    assert_eq!(u32::from_le_bytes(poses[2][15..19].try_into().unwrap()), 7);
    assert!(result
        .report
        .warnings
        .iter()
        .any(|warning| warning.contains("未覆盖完整动作范围")));
}

#[test]
fn looped_nonzero_constant_translation_does_not_drift() {
    let records = (0..5)
        .map(|frame| record("センター", frame, 12.0, [0.0, 0.0, 0.0, -1.0]))
        .collect::<Vec<_>>();
    let mut opts = options(BoneSelection::Named(vec!["センター".into()]));
    opts.looped = true;
    let result = smooth_bytes(&vmd(&records), &opts).unwrap();
    let poses = output_records(&result.bytes);
    for pose in poses {
        assert!((f32_at(&pose, 19) - 12.0).abs() < 1e-5);
    }
}

#[test]
fn looped_root_track_preserves_linear_displacement() {
    let xs = [0.0, 2.0, 4.0, 6.0];
    let records = xs
        .iter()
        .enumerate()
        .map(|(frame, x)| record("センター", frame as u32, *x, [0.0, 0.0, 0.0, -1.0]))
        .collect::<Vec<_>>();
    let mut opts = options(BoneSelection::Named(vec!["センター".into()]));
    opts.looped = true;
    let result = smooth_bytes(&vmd(&records), &opts).unwrap();
    let poses = output_records(&result.bytes);
    for (pose, expected) in poses.iter().zip(xs) {
        assert!((f32_at(pose, 19) - expected).abs() < 1e-4);
    }
    assert!((f32_at(poses.last().unwrap(), 19) - f32_at(&poses[0], 19) - 6.0).abs() < 1e-4);
    assert!(result
        .report
        .warnings
        .iter()
        .any(|warning| warning.contains("持续位移")));
}

#[test]
fn quaternion_sign_equivalence_stays_normalized() {
    let records = [
        record("spine", 0, 0.0, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 1, 0.0, [0.0, 0.0, 0.0, 1.0]),
        record("spine", 2, 0.0, [0.0, 0.0, 0.0, -1.0]),
    ];
    let result = smooth_bytes(&vmd(&records), &options(BoneSelection::AllExceptIk)).unwrap();
    for pose in output_records(&result.bytes) {
        let norm = [31, 35, 39, 43]
            .map(|at| f32_at(&pose, at))
            .iter()
            .map(|v| v * v)
            .sum::<f32>()
            .sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
        assert!(f32_at(&pose, 31).abs() < 1e-5);
        assert!(f32_at(&pose, 35).abs() < 1e-5);
        assert!(f32_at(&pose, 39).abs() < 1e-5);
        assert!((f32_at(&pose, 43).abs() - 1.0).abs() < 1e-5);
    }
}

#[test]
fn angular_spike_is_reduced_without_changing_rotation_axis() {
    let angle = 1.2_f32;
    let records = (0..5)
        .map(|frame| {
            let rotation = if frame == 2 {
                [(angle / 2.0).sin(), 0.0, 0.0, -(angle / 2.0).cos()]
            } else {
                [0.0, 0.0, 0.0, -1.0]
            };
            record("spine", frame, 0.0, rotation)
        })
        .collect::<Vec<_>>();
    let result = smooth_bytes(&vmd(&records), &options(BoneSelection::AllExceptIk)).unwrap();
    let poses = output_records(&result.bytes);
    let reduced = 2.0 * f32_at(&poses[2], 43).abs().clamp(0.0, 1.0).acos();
    assert!(reduced > 0.0 && reduced < angle * 0.8);
    assert!(f32_at(&poses[2], 35).abs() < 1e-5);
    assert!(f32_at(&poses[2], 39).abs() < 1e-5);
}

#[test]
fn nonempty_morph_camera_light_shadow_and_hidden_ik_are_preserved() {
    let records = [
        record("spine", 0, 0.0, [0.0, 0.0, 0.0, -1.0]),
        record("spine", 2, 3.0, [0.0, 0.0, 0.0, -1.0]),
    ];
    let mut source = vmd(&records);
    source.truncate(54 + records.len() * 111);
    let mut tail = Vec::new();
    // 各区段使用非空记录，包含关闭显示的 IK 帧。
    let mut morph = vec![0_u8; 23];
    morph[..5].copy_from_slice(b"blink");
    morph[15..19].copy_from_slice(&8_u32.to_le_bytes());
    morph[19..23].copy_from_slice(&0.7_f32.to_le_bytes());
    let mut camera = vec![0_u8; 61];
    camera[..4].copy_from_slice(&8_u32.to_le_bytes());
    camera[4..8].copy_from_slice(&(-12.0_f32).to_le_bytes());
    camera[56..60].copy_from_slice(&45_u32.to_le_bytes());
    camera[60] = 1;
    let mut light = vec![0_u8; 28];
    light[..4].copy_from_slice(&8_u32.to_le_bytes());
    light[4..8].copy_from_slice(&0.5_f32.to_le_bytes());
    let mut shadow = vec![0_u8; 9];
    shadow[..4].copy_from_slice(&8_u32.to_le_bytes());
    shadow[4] = 1;
    shadow[5..9].copy_from_slice(&4.0_f32.to_le_bytes());
    for section in [morph, camera, light, shadow] {
        tail.extend_from_slice(&1_u32.to_le_bytes());
        tail.extend_from_slice(&section);
    }
    tail.extend_from_slice(&1_u32.to_le_bytes());
    tail.extend_from_slice(&8_u32.to_le_bytes());
    tail.push(0);
    tail.extend_from_slice(&1_u32.to_le_bytes());
    let mut ik_name = [0_u8; 20];
    ik_name[..12].copy_from_slice(b"left foot ik");
    tail.extend_from_slice(&ik_name);
    tail.push(0);
    tail.extend_from_slice(b"extension data");
    source.extend_from_slice(&tail);
    let result = smooth_bytes(&source, &options(BoneSelection::AllExceptIk)).unwrap();
    assert_eq!(output_records(&result.bytes).len(), 3);
    assert_eq!(
        &result.bytes[result.bytes.len() - tail.len()..],
        tail.as_slice()
    );
    VmdDocument::from_bytes(&result.bytes).unwrap();
}

#[test]
fn unselected_records_and_opaque_tail_are_preserved() {
    let unselected = record("left foot", 1, 7.0, [0.0, 0.0, 0.0, -1.0]);
    let selected = record("spine", 1, 2.0, [0.0, 0.0, 0.0, -1.0]);
    let source = vmd(&[unselected.clone(), selected]);
    let tail = &source[54 + 222..];
    let result = smooth_bytes(
        &source,
        &options(BoneSelection::Named(vec!["spine".into()])),
    )
    .unwrap();
    let records = output_records(&result.bytes);
    assert_eq!(records[0], unselected);
    let out_tail = &result.bytes[result.bytes.len() - tail.len()..];
    assert_eq!(out_tail, tail);
}

#[test]
fn malformed_sections_non_finite_values_and_bad_quaternions_are_rejected() {
    let mut truncated = vmd(&[record("spine", 0, 0.0, [0.0, 0.0, 0.0, -1.0])]);
    truncated.pop();
    assert!(VmdDocument::from_bytes(&truncated).is_err());

    let mut non_finite = vmd(&[record("spine", 0, f32::NAN, [0.0, 0.0, 0.0, -1.0])]);
    assert!(VmdDocument::from_bytes(&non_finite).is_err());

    let zero_q = vmd(&[record("spine", 0, 0.0, [0.0; 4])]);
    assert!(VmdDocument::from_bytes(&zero_q).is_err());

    non_finite[..25].copy_from_slice(b"Vocaloid Motion Data file");
    assert!(VmdDocument::from_bytes(&non_finite)
        .unwrap_err()
        .to_string()
        .contains("VMD1"));
}

#[test]
fn project_walk_and_sprint_motions_can_be_processed() {
    for bytes in [SPRINT, WALK] {
        let document = VmdDocument::from_bytes(bytes).unwrap();
        let result = document
            .smooth(&options(BoneSelection::AllExceptIk))
            .unwrap();
        assert!(result.report.selected_tracks > 0);
        assert!(result.report.output_keys >= result.report.original_keys);
        VmdDocument::from_bytes(&result.bytes).unwrap();
    }
}

#[test]
fn sprint_ik_smoothing_preserves_endpoints_and_reduces_jitter() {
    let left = "左足ＩＫ";
    let right = "右足ＩＫ";
    let opts = SmoothingOptions {
        strength: 1.0,
        radius: 6,
        looped: true,
        selection: BoneSelection::Named(vec![left.into(), right.into()]),
    };
    opts.validate().unwrap();
    let source_motion = VmdFile::load_from_bytes(SPRINT).unwrap().motion;
    let result = smooth_bytes(SPRINT, &opts).unwrap();
    let output_left = records_for(&result.bytes, left);
    let output_right = records_for(&result.bytes, right);
    assert_eq!(output_left.first().unwrap()[15..19], 0_u32.to_le_bytes());
    assert_eq!(output_left.last().unwrap()[15..19], 20_u32.to_le_bytes());
    assert_eq!(output_right.first().unwrap()[15..19], 0_u32.to_le_bytes());
    assert_eq!(output_right.last().unwrap()[15..19], 20_u32.to_le_bytes());

    for (name, records) in [(left, &output_left), (right, &output_right)] {
        let source_first = source_motion.find_bone_transform(name, 0, 0.0);
        let source_last = source_motion.find_bone_transform(name, 20, 0.0);
        for (record, source) in [
            (records.first().unwrap(), source_first),
            (records.last().unwrap(), source_last),
        ] {
            let position = raw_position(record);
            assert!((position - source.translation).length() < 1e-5);
            let q = glam::Quat::from_xyzw(
                f32_at(record, 31),
                f32_at(record, 35),
                -f32_at(record, 39),
                -f32_at(record, 43),
            );
            assert!(q.normalize().dot(source.orientation.normalize()).abs() > 0.99999);
        }
        assert!(records.iter().skip(1).take(19).any(|record| {
            let frame = u32::from_le_bytes(record[15..19].try_into().unwrap());
            let original = source_motion.find_bone_transform(name, frame, 0.0);
            (raw_position(record) - original.translation).length() > 1e-4
        }));
        let original = (0..=20)
            .map(|frame| {
                source_motion
                    .find_bone_transform(name, frame, 0.0)
                    .translation
            })
            .collect::<Vec<_>>();
        let smoothed = records
            .iter()
            .map(|record| raw_position(record))
            .collect::<Vec<_>>();
        assert_extrema_and_range_preserved(&original, &smoothed);
        let original_energy = second_difference_energy(&original);
        let smoothed_energy = second_difference_energy(&smoothed);
        assert!(
            smoothed_energy < original_energy,
            "{name}: 原 {original_energy}，平滑 {smoothed_energy}"
        );
    }
    assert!(output_left.iter().all(|record| f32_at(record, 23) >= -1e-6));

    for (name, frame, output) in [
        (left, 0, output_left.first().unwrap()),
        (left, 20, output_left.last().unwrap()),
        (right, 0, output_right.first().unwrap()),
        (right, 20, output_right.last().unwrap()),
    ] {
        let source = records_for(SPRINT, name);
        let expected = source
            .iter()
            .find(|record| u32::from_le_bytes(record[15..19].try_into().unwrap()) == frame)
            .unwrap();
        for offset in [19, 23, 27, 31, 35, 39, 43] {
            assert!((f32_at(output, offset) - f32_at(expected, offset)).abs() < 1e-5);
        }
    }
    let source_count = u32::from_le_bytes(SPRINT[50..54].try_into().unwrap()) as usize;
    let output_count = u32::from_le_bytes(result.bytes[50..54].try_into().unwrap()) as usize;
    let source_tail = &SPRINT[54 + source_count * 111..];
    let output_tail = &result.bytes[54 + output_count * 111..];
    assert_eq!(output_tail, source_tail);

    let document = VmdDocument::from_bytes(SPRINT).unwrap();
    let unselected = document
        .bone_names()
        .into_iter()
        .find(|name| name != left && name != right)
        .unwrap();
    assert_eq!(
        records_for(&result.bytes, &unselected),
        records_for(SPRINT, &unselected)
    );
}

#[test]
fn sprint_user_settings_preserve_foot_ranges_and_still_smooth_elbow_rotation() {
    let names = [
        "左足ＩＫ",
        "右足ＩＫ",
        "左つま先ＩＫ",
        "右つま先ＩＫ",
        "左ひじ",
        "右ひじ",
    ];
    let document = VmdDocument::from_bytes(SPRINT).unwrap();
    let present = document.bone_names();
    for name in names {
        assert!(present.iter().any(|candidate| candidate == name), "{name}");
    }
    let source_motion = VmdFile::load_from_bytes(SPRINT).unwrap().motion;
    let opts = SmoothingOptions {
        strength: 0.85,
        radius: 5,
        looped: true,
        selection: BoneSelection::Named(names.iter().map(|name| (*name).to_string()).collect()),
    };
    let result = smooth_bytes(SPRINT, &opts).unwrap();

    for name in &names[..4] {
        let source = (0..=20)
            .map(|frame| {
                source_motion
                    .find_bone_transform(name, frame, 0.0)
                    .translation
            })
            .collect::<Vec<_>>();
        let output = records_for(&result.bytes, name)
            .iter()
            .map(|record| raw_position(record))
            .collect::<Vec<_>>();
        assert_extrema_and_range_preserved(&source, &output);
        assert!(output
            .iter()
            .zip(&source)
            .any(|(filtered, original)| (*filtered - *original).length() > 1e-4));
    }

    let left_elbow = records_for(&result.bytes, "左ひじ");
    assert!(left_elbow.iter().any(|record| {
        let frame = u32::from_le_bytes(record[15..19].try_into().unwrap());
        let original = source_motion
            .find_bone_transform("左ひじ", frame, 0.0)
            .orientation;
        let output = glam::Quat::from_xyzw(
            f32_at(record, 31),
            f32_at(record, 35),
            -f32_at(record, 39),
            -f32_at(record, 43),
        );
        2.0 * original.dot(output).abs().clamp(0.0, 1.0).acos() > 1e-4
    }));
}
