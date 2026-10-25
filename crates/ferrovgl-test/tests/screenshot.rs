//! Screenshot normalization, exact matching, diagnostics and missing-reference checks.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ferrovgl_test::{RgbImage, TestDisplay, compare_screenshot};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ferrovgl-screenshot-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn xrgb_memory_order_padding_and_alpha_are_ignored() {
    let bytes = [
        30, 20, 10, 0, 60, 50, 40, 255, 99, 99, 99, 99, 90, 80, 70, 17, 120, 110, 100, 42,
    ];
    let rgb = RgbImage::from_xrgb8888(2, 2, 12, &bytes).unwrap();
    assert_eq!(rgb.pixels(), &[10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120]);
    assert!(RgbImage::from_xrgb8888(2, 2, 7, &bytes).is_err());
    assert!(RgbImage::from_xrgb8888(2, 2, 12, &bytes[..19]).is_err());
}

#[test]
fn one_byte_difference_fails_and_success_removes_old_artifacts() {
    let dir = TempDir::new();
    let refs = dir.0.join("refs");
    let artifacts = dir.0.join("artifacts");
    fs::create_dir_all(refs.join("draw")).unwrap();
    let expected = RgbImage::new(2, 1, vec![1, 2, 3, 4, 5, 6]).unwrap();
    expected.write_png(refs.join("draw/example.png")).unwrap();
    let actual = RgbImage::new(2, 1, vec![1, 2, 3, 4, 5, 7]).unwrap();
    let error = compare_screenshot(&actual, &refs, &artifacts, "draw/example.png").unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("1 differing pixels"));
    assert!(
        fs::read_to_string(artifacts.join("draw/example.result"))
            .unwrap()
            .starts_with("mismatch\n")
    );
    assert_eq!(
        RgbImage::read_png(artifacts.join("draw/example_actual.png")).unwrap(),
        actual
    );
    assert_eq!(
        RgbImage::read_png(artifacts.join("draw/example_expected.png")).unwrap(),
        expected
    );
    let diff = RgbImage::read_png(artifacts.join("draw/example_diff.png")).unwrap();
    assert_eq!(diff.pixels(), &[0, 0, 0, 255, 255, 255]);
    compare_screenshot(&expected, &refs, &artifacts, "draw/example.png").unwrap();
    assert_eq!(
        fs::read_to_string(artifacts.join("draw/example.result")).unwrap(),
        "matched\n"
    );
    assert!(!artifacts.join("draw/example_diff.png").exists());
}

#[test]
fn missing_references_and_dimension_mismatches_cannot_pass() {
    let dir = TempDir::new();
    let refs = dir.0.join("refs");
    let artifacts = dir.0.join("artifacts");
    fs::create_dir_all(&refs).unwrap();
    let actual = RgbImage::new(1, 2, vec![10; 6]).unwrap();
    assert_eq!(
        compare_screenshot(&actual, &refs, &artifacts, "missing.png")
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    assert!(!refs.join("missing.png").exists());
    assert!(
        fs::read_to_string(artifacts.join("missing.result"))
            .unwrap()
            .starts_with("error\n")
    );
    RgbImage::new(2, 1, vec![10; 6])
        .unwrap()
        .write_png(refs.join("shape.png"))
        .unwrap();
    assert!(compare_screenshot(&actual, &refs, &artifacts, "shape.png").is_err());
    assert_eq!(
        RgbImage::read_png(artifacts.join("shape_diff.png"))
            .unwrap()
            .pixel(1, 0),
        Some([255; 3])
    );
    assert!(compare_screenshot(&actual, &refs, &artifacts, "../escape.png").is_err());
}

#[test]
fn rgba_reference_alpha_does_not_affect_rgb_comparison() {
    let dir = TempDir::new();
    let file = fs::File::create(dir.0.join("rgba.png")).unwrap();
    let mut encoder = png::Encoder::new(file, 2, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&[1, 2, 3, 0, 4, 5, 6, 255]).unwrap();
    writer.finish().unwrap();
    let rgb = RgbImage::new(2, 1, vec![1, 2, 3, 4, 5, 6]).unwrap();
    assert_eq!(RgbImage::read_png(dir.0.join("rgba.png")).unwrap(), rgb);
    compare_screenshot(&rgb, &dir.0, dir.0.join("artifacts"), "rgba.png").unwrap();
}

#[test]
fn render_failure_overwrites_an_earlier_passing_result() {
    let dir = TempDir::new();
    let artifacts = dir.0.join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    fs::write(artifacts.join("scene.result"), "matched\n").unwrap();
    let mut display = TestDisplay::with_resolution(2, 1).unwrap();
    display.ui.display_delete(display.display());
    let error = display
        .screenshot_compare_in(&dir.0, &artifacts, "scene.png")
        .unwrap_err();
    assert!(error.to_string().contains("deleted"));
    let result = fs::read_to_string(artifacts.join("scene.result")).unwrap();
    assert!(result.starts_with("error\n"));
    assert!(result.contains("deleted"));
}
