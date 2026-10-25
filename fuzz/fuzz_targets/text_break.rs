#![no_main]

use ferrovgl_core::misc::text::{self, TextAttributes, TextFlags};
use ferrovgl_fonts::MONTSERRAT_14;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&flags, bytes)) = data.split_first() else {
        return;
    };
    let bytes = &bytes[..bytes.len().min(1024)];
    // Raw-byte UTF-8 decoding is also exercised with malformed input. Bound
    // the walk to prevent a decoder bug from turning into a fuzzer hang.
    let mut offset = 0;
    for _ in 0..=bytes.len() {
        if offset as usize >= bytes.len() || bytes[offset as usize] == 0 {
            break;
        }
        let before = offset;
        text::encoded_next(bytes, Some(&mut offset));
        assert!(offset > before && offset as usize <= bytes.len());
    }
    // Layout takes valid Rust strings; NUL terminates the same way as C.
    let text = String::from_utf8_lossy(bytes);
    let text = text.split('\0').next().unwrap();
    let attributes = TextAttributes {
        max_width: i32::from(bytes.first().copied().unwrap_or(0)),
        letter_space: i32::from(flags >> 5) - 3,
        line_space: i32::from(flags & 3),
        text_flags: TextFlags::from_bits_truncate(flags),
    };
    let mut offset = 0;
    while offset < text.len() {
        let tail = &text[offset..];
        let mut attributes = attributes.clone();
        let mut width = 0;
        let advance = text::get_next_line(
            tail,
            tail.len() as u32,
            &MONTSERRAT_14,
            Some(&mut width),
            &mut attributes,
        ) as usize;
        assert!(advance > 0 && advance <= tail.len());
        assert!(tail.is_char_boundary(advance));
        offset += advance;
    }
    let _size = text::get_size_attributes(text, &MONTSERRAT_14, &attributes);
    let mut edited = text.to_owned();
    let position = u32::from(flags) % (text.chars().count() as u32 + 1);
    text::ins(&mut edited, position, "é한");
    text::cut(&mut edited, position, 2);
    assert_eq!(edited, text);
});
