use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::{Component, Path, PathBuf};

use ferrovgl_core::ColorFormat;
use ferrovgl_core::draw::draw_buf::DrawBuf;

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

    /// `buf_to_xrgb8888()` of `lv_test_screenshot_compare.c`, keeping the RGB
    /// channels: RGB565 (and RGB565_SWAPPED, read with swapped bytes),
    /// ARGB8888, XRGB8888, premultiplied ARGB8888, RGB888, L8, AL88 and I1.
    ///
    /// Upstream leaves the output uninitialised for any other format; this
    /// returns an error instead, and also when `bytes` cannot hold `height`
    /// rows of `stride` bytes.
    pub fn from_buffer(
        width: u32,
        height: u32,
        stride: u32,
        cf: ColorFormat,
        bytes: &[u8],
    ) -> io::Result<Self> {
        let len = image_len(width, height)?;
        let (w, h, stride) = (width as usize, height as usize, stride as usize);
        let (start, row_bytes) = match cf {
            ColorFormat::Rgb565 | ColorFormat::Rgb565Swapped | ColorFormat::Al88 => (0, w * 2),
            ColorFormat::Argb8888 | ColorFormat::Xrgb8888 | ColorFormat::Argb8888Premultiplied => (0, w * 4),
            ColorFormat::Rgb888 => (0, w * 3),
            ColorFormat::L8 => (0, w),
            /* `buf_in += 8`: skip the two-entry palette. */
            ColorFormat::I1 => (8, w.div_ceil(8)),
            _ => return Err(invalid(format!("screenshots do not support {cf:?}"))),
        };
        let required = stride
            .checked_mul(h - 1)
            .and_then(|n| n.checked_add(start))
            .and_then(|n| n.checked_add(row_bytes))
            .ok_or_else(|| invalid("buffer size overflow"))?;
        if bytes.len() < required {
            return Err(invalid("draw buffer or stride is too short"));
        }
        let mut pixels = Vec::with_capacity(len);
        for y in 0..h {
            let row = start + y * stride;
            let buf_in = &bytes[row..row + row_bytes];
            match cf {
                ColorFormat::Rgb565 | ColorFormat::Rgb565Swapped => {
                    for p in buf_in.as_chunks::<2>().0 {
                        let c16 = if cf == ColorFormat::Rgb565Swapped {
                            u16::from_be_bytes(*p)
                        } else {
                            u16::from_le_bytes(*p)
                        };
                        let red = u32::from(c16 >> 11);
                        let green = u32::from((c16 >> 5) & 0x3f);
                        let blue = u32::from(c16 & 0x1f);
                        /*To make it rounded*/
                        pixels.extend_from_slice(&[
                            ((red * 2106) >> 8) as u8,
                            ((green * 1037) >> 8) as u8,
                            ((blue * 2106) >> 8) as u8,
                        ]);
                    }
                }
                ColorFormat::Argb8888 | ColorFormat::Xrgb8888 | ColorFormat::Argb8888Premultiplied => {
                    for p in buf_in.as_chunks::<4>().0 {
                        pixels.extend_from_slice(&[p[2], p[1], p[0]]);
                    }
                }
                ColorFormat::Rgb888 => {
                    for p in buf_in.as_chunks::<3>().0 {
                        pixels.extend_from_slice(&[p[2], p[1], p[0]]);
                    }
                }
                ColorFormat::L8 => {
                    for &l in buf_in {
                        pixels.extend_from_slice(&[l; 3]);
                    }
                }
                ColorFormat::Al88 => {
                    for p in buf_in.as_chunks::<2>().0 {
                        pixels.extend_from_slice(&[p[0]; 3]);
                    }
                }
                ColorFormat::I1 => {
                    for x in 0..w {
                        let byte = buf_in[x / 8];
                        let bit_pos = x % 8;
                        let pixel = (byte >> (7 - bit_pos)) & 0x01;
                        pixels.extend_from_slice(&[if pixel != 0 { 0xff } else { 0x00 }; 3]);
                    }
                }
                _ => unreachable!("rejected above"),
            }
        }
        Self::new(width, height, pixels)
    }

    /// [`Self::from_buffer`] for a draw buffer's header and data.
    pub fn from_draw_buf(buf: &DrawBuf) -> io::Result<Self> {
        Self::from_buffer(
            buf.header.w.into(),
            buf.header.h.into(),
            buf.header.stride.into(),
            buf.header.cf,
            buf.data(),
        )
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
