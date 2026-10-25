use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::{Component, Path, PathBuf};

/// An image normalized to three bytes per pixel in R, G, B order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn image_len(width: u32, height: u32) -> io::Result<usize> {
    if width == 0 || height == 0 {
        return Err(invalid("image dimensions must be positive"));
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(3))
        .ok_or_else(|| invalid("image dimensions overflow"))
}

pub(crate) fn validate_xrgb8888(width: u32, height: u32, stride: u32, byte_len: usize) -> io::Result<usize> {
    let len = image_len(width, height)?;
    let row_bytes = (width as usize)
        .checked_mul(4)
        .ok_or_else(|| invalid("row width overflow"))?;
    let stride = stride as usize;
    let required = stride
        .checked_mul(height as usize - 1)
        .and_then(|n| n.checked_add(row_bytes))
        .ok_or_else(|| invalid("buffer size overflow"))?;
    if stride < row_bytes || byte_len < required {
        return Err(invalid("XRGB8888 buffer or stride is too short"));
    }
    Ok(len)
}

impl RgbImage {
    /// Create an image from packed RGB bytes, validating the dimensions.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> io::Result<Self> {
        if pixels.len() != image_len(width, height)? {
            return Err(invalid("RGB byte count does not match image dimensions"));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Convert LVGL XRGB8888 memory (B, G, R, X), ignoring X and row padding.
    pub fn from_xrgb8888(width: u32, height: u32, stride: u32, bytes: &[u8]) -> io::Result<Self> {
        let len = validate_xrgb8888(width, height, stride, bytes.len())?;
        let row_bytes = width as usize * 4;
        let stride = stride as usize;
        let mut pixels = Vec::with_capacity(len);
        for y in 0..height as usize {
            for p in bytes[y * stride..y * stride + row_bytes].as_chunks::<4>().0 {
                pixels.extend_from_slice(&[p[2], p[1], p[0]]);
            }
        }
        Self::new(width, height, pixels)
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Packed RGB bytes in scanline order.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// RGB at a coordinate; returns `None` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = (y as usize * self.width as usize + x as usize) * 3;
        Some(self.pixels[offset..offset + 3].try_into().expect("RGB pixel"))
    }

    /// Decode PNG to RGB, expanding palettes/grayscale and ignoring alpha.
    /// No gamma or colour-profile correction is applied, matching upstream.
    pub fn read_png(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut decoder = png::Decoder::new(BufReader::new(File::open(path)?));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(io::Error::other)?;
        let mut bytes = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut bytes).map_err(io::Error::other)?;
        let bytes = &bytes[..info.buffer_size()];
        let mut pixels = Vec::with_capacity(image_len(info.width, info.height)?);
        match info.color_type {
            png::ColorType::Rgb => pixels.extend_from_slice(bytes),
            png::ColorType::Rgba => {
                for p in bytes.as_chunks::<4>().0 {
                    pixels.extend_from_slice(&p[..3]);
                }
            }
            png::ColorType::Grayscale => {
                for &v in bytes {
                    pixels.extend_from_slice(&[v; 3]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for p in bytes.as_chunks::<2>().0 {
                    pixels.extend_from_slice(&[p[0]; 3]);
                }
            }
            png::ColorType::Indexed => return Err(invalid("PNG palette was not expanded")),
        }
        Self::new(info.width, info.height, pixels)
    }

    /// Write an eight-bit RGB PNG.
    pub fn write_png(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), self.width, self.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(io::Error::other)?;
        writer.write_image_data(&self.pixels).map_err(io::Error::other)?;
        writer.finish().map_err(io::Error::other)
    }
}

/// Compare RGB bytes exactly against a relative PNG in `reference_dir`.
///
/// Artifact paths preserve subdirectories: `draw/a.png` produces
/// `draw/a_actual.png`, `draw/a_expected.png`, `draw/a_diff.png`, and
/// `draw/a.result` under `artifact_dir`. The result file's first line is
/// `matched`, `mismatch`, or `error`. Missing references fail without being
/// generated or modified. Successful reruns remove obsolete diff artifacts.
pub fn compare_screenshot(
    actual: &RgbImage,
    reference_dir: impl AsRef<Path>,
    artifact_dir: impl AsRef<Path>,
    name: impl AsRef<Path>,
) -> io::Result<()> {
    let name = name.as_ref();
    let base = begin_comparison(artifact_dir.as_ref(), name)?;
    let stem = base.file_stem().expect("PNG stem").to_string_lossy();
    let artifact = |kind: &str| base.with_file_name(format!("{stem}_{kind}.png"));
    let result = base.with_extension("result");
    let expected = match RgbImage::read_png(reference_dir.as_ref().join(name)) {
        Ok(image) => image,
        Err(error) => {
            actual.write_png(artifact("actual"))?;
            fs::write(&result, format!("error\n{error}\n"))?;
            return Err(error);
        }
    };
    if actual == &expected {
        for kind in ["actual", "expected", "diff"] {
            match fs::remove_file(artifact(kind)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        fs::write(result, "matched\n")?;
        return Ok(());
    }

    actual.write_png(artifact("actual"))?;
    expected.write_png(artifact("expected"))?;
    let width = actual.width.max(expected.width);
    let height = actual.height.max(expected.height);
    let mut diff = vec![0; image_len(width, height)?];
    let mut count = 0;
    let mut first = None;
    for y in 0..height {
        for x in 0..width {
            let a = actual.pixel(x, y);
            let e = expected.pixel(x, y);
            if a != e {
                count += 1;
                first.get_or_insert((x, y, a, e));
                let off = (y as usize * width as usize + x as usize) * 3;
                // White marks every differing pixel, including one-bit errors.
                diff[off..off + 3].fill(255);
            }
        }
    }
    RgbImage::new(width, height, diff)?.write_png(artifact("diff"))?;
    let message = format!(
        "{}: {count} differing pixels; actual {}x{}, expected {}x{}; first {first:?}",
        name.display(),
        actual.width,
        actual.height,
        expected.width,
        expected.height
    );
    fs::write(result, format!("mismatch\n{message}\n"))?;
    Err(invalid(message))
}

/// Validate the relative name and mark an attempt before rendering or decoding.
pub(crate) fn begin_comparison(artifact_dir: &Path, name: &Path) -> io::Result<PathBuf> {
    if name.extension().is_none_or(|ext| ext != "png")
        || name.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "screenshot name must be a relative PNG path without parent components",
        ));
    }
    let base = artifact_dir.join(name);
    fs::create_dir_all(base.parent().expect("artifact parent"))?;
    let result = base.with_extension("result");
    // An interrupted attempt cannot retain a previous successful result.
    fs::write(&result, "error\ncomparison did not complete\n")?;
    Ok(base)
}
