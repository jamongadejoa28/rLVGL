//! Byte-level ports of the `lv_canvas` buffer operations used by the canvas
//! parity scenes. The canvas widget API is not ported; a "canvas" is a
//! [`DrawBuf`] written exactly like `lv_canvas.c` writes its draw buffer.

// Each test target includes this module but uses only part of it.
#![allow(dead_code)]

use ferrovgl_core::draw::LayerId;
use ferrovgl_core::draw::draw_buf::{DrawBuf, STRIDE_AUTO};
use ferrovgl_core::{Area, Color, ColorFormat, Opa, Ui};

/// A zero-initialised buffer, like the `static` arrays behind
/// `LV_DRAW_BUF_DEFINE_STATIC` / `lv_canvas_set_buffer()` in the C tests.
pub fn canvas_buf(w: u32, h: u32, cf: ColorFormat) -> DrawBuf {
    DrawBuf::create(w, h, cf, STRIDE_AUTO).expect("canvas buffer")
}

/// `lv_canvas_set_px()` (without the `lv_obj_invalidate()`).
pub fn canvas_set_px(buf: &mut DrawBuf, x: i32, y: i32, color: Color, opa: Opa) {
    let cf = buf.header.cf;
    /*lv_draw_buf_goto_xy() returns NULL for invalid coordinates*/
    let (Ok(ux), Ok(uy)) = (u32::try_from(x), u32::try_from(y)) else {
        return;
    };
    let Some(at) = buf.goto_xy_checked(ux, uy) else {
        return;
    };
    let data = &mut buf.data_mut().expect("writable canvas buffer")[at..];

    if cf.is_indexed() {
        let mut c_int: u8 = color.blue;
        let shift: u8 = match cf {
            ColorFormat::I1 => 7 - (x & 0x7) as u8,
            ColorFormat::I2 => 6 - 2 * (x & 0x3) as u8,
            ColorFormat::I4 => 4 - 4 * (x & 0x1) as u8,
            ColorFormat::I8 => {
                /*Indexed8 format is a easy case, process and return.*/
                data[0] = c_int;
                /*falls through to `default: return;`*/
                return;
            }
            _ => return,
        };

        let bpp = cf.bpp();
        let mask: u8 = ((1u32 << bpp) - 1) as u8;
        c_int &= mask;
        data[0] = (data[0] & !(mask << shift)) | (c_int << shift);
    } else if cf == ColorFormat::L8 {
        data[0] = color.luminance();
    } else if cf == ColorFormat::A8 {
        data[0] = opa;
    } else if cf == ColorFormat::Rgb565 {
        /*lv_color16_t bitfields: blue:5, green:6, red:5*/
        let mut c16 = u16::from_le_bytes([data[0], data[1]]);
        c16 = (c16 & !0xf800) | (u16::from(color.red >> 3) << 11);
        c16 = (c16 & !0x07e0) | (u16::from(color.green >> 2) << 5);
        c16 = (c16 & !0x001f) | u16::from(color.blue >> 3);
        data[..2].copy_from_slice(&c16.to_le_bytes());
    } else if cf == ColorFormat::Rgb888 {
        data[2] = color.red;
        data[1] = color.green;
        data[0] = color.blue;
    } else if cf == ColorFormat::Xrgb8888 {
        data[2] = color.red;
        data[1] = color.green;
        data[0] = color.blue;
        data[3] = 0xFF;
    } else if cf == ColorFormat::Argb8888 {
        /*lv_color32_t: blue, green, red, alpha*/
        data[2] = color.red;
        data[1] = color.green;
        data[0] = color.blue;
        data[3] = opa;
    } else if cf == ColorFormat::Al88 {
        /*lv_color16a_t: lumi, alpha*/
        data[0] = color.luminance();
        data[1] = 255;
    }
}

/// `lv_canvas_fill_bg()` (without the cache flush and invalidation).
pub fn canvas_fill_bg(buf: &mut DrawBuf, color: Color, opa: Opa) {
    let cf = buf.header.cf;
    let (w, h) = (buf.header.w as usize, buf.header.h as usize);
    let stride = buf.header.stride as usize;

    if matches!(
        cf,
        ColorFormat::Rgb565
            | ColorFormat::Xrgb8888
            | ColorFormat::Argb8888
            | ColorFormat::Rgb888
            | ColorFormat::L8
            | ColorFormat::Al88
    ) {
        let data = buf.data_mut().expect("writable canvas buffer");
        for y in 0..h {
            let row = &mut data[y * stride..];
            match cf {
                ColorFormat::Rgb565 => {
                    let c16 = color.to_u16().to_le_bytes();
                    for px in row[..w * 2].as_chunks_mut::<2>().0 {
                        px.copy_from_slice(&c16);
                    }
                }
                ColorFormat::Xrgb8888 | ColorFormat::Argb8888 => {
                    let mut c32 = color.to_u32();
                    if cf == ColorFormat::Argb8888 {
                        c32 &= 0x00ff_ffff;
                        c32 |= u32::from(opa) << 24;
                    }
                    for px in row[..w * 4].as_chunks_mut::<4>().0 {
                        px.copy_from_slice(&c32.to_le_bytes());
                    }
                }
                ColorFormat::Rgb888 => {
                    for px in row[..w * 3].as_chunks_mut::<3>().0 {
                        px.copy_from_slice(&[color.blue, color.green, color.red]);
                    }
                }
                ColorFormat::L8 => {
                    let c8 = color.luminance();
                    row[..w].fill(c8);
                }
                ColorFormat::Al88 => {
                    let c = [color.luminance(), 255];
                    for px in row[..w * 2].as_chunks_mut::<2>().0 {
                        px.copy_from_slice(&c);
                    }
                }
                _ => unreachable!(),
            }
        }
    } else {
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                canvas_set_px(buf, x, y, color, opa);
            }
        }
    }
}

/// `lv_canvas_init_layer()`: a layer drawing directly into the canvas buffer.
pub fn canvas_init_layer(ui: &mut Ui, buf: DrawBuf) -> LayerId {
    let area = Area::new(0, 0, i32::from(buf.header.w) - 1, i32::from(buf.header.h) - 1);
    let layer = ui.draw_layer_create(None, buf.header.cf, &area);
    ui.draw.layer_mut(layer).unwrap().draw_buf = Some(buf);
    layer
}

/// `lv_canvas_finish_layer()`: dispatch until the queue is empty and hand the
/// canvas buffer back.
pub fn canvas_finish_layer(ui: &mut Ui, layer: LayerId) -> DrawBuf {
    for _ in 0..64 {
        if ui.draw.layer(layer).unwrap().tasks.is_empty() {
            break;
        }
        ui.draw_dispatch();
    }
    assert!(
        ui.draw.layer(layer).unwrap().tasks.is_empty(),
        "draw queue stalled"
    );
    let buf = ui.draw.layer_mut(layer).unwrap().draw_buf.take().unwrap();
    ui.draw_layer_delete(layer);
    buf
}
