//! Observer bindings of the widgets (`LV_USE_OBSERVER`).
//!
//! Ported from LVGL v9.6.0: the `#if LV_USE_OBSERVER` parts of
//! `src/widgets/arc/lv_arc.c`, `src/widgets/bar/lv_bar.c`,
//! `src/widgets/image/lv_image.c`, `src/widgets/label/lv_label.c` and
//! `src/widgets/slider/lv_slider.c` (`lv_<widget>_bind_*()` and their static
//! observer / event callbacks).
//!
//! Also ports the formatting core of
//! `src/stdlib/builtin/lv_sprintf_builtin.c` (`lv_vsnprintf_inner()` and its
//! helpers, the `LV_USE_STDLIB_SPRINTF == LV_STDLIB_BUILTIN` default of the
//! parity config), because `label_text_observer_cb()` formats the subject's
//! value with the run-time `fmt` given to `lv_label_bind_text()`.
//!
//! They are gathered here, next to the core subject/observer API
//! (`ferrovgl_core::core::observer`), so that the observer port does not edit
//! the widget modules; each binding is an `impl` block of its widget handle.
//!
//! SKELETON: `Arc::bind_value()` stays `todo!()` until the arc widget is
//! ported.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use alloc::rc::Rc;
#[allow(unused_imports)]
use alloc::string::String;
#[allow(unused_imports)]
use core::any::Any;
#[allow(unused_imports)]
use ferrovgl_core::Ui;
#[allow(unused_imports)]
use ferrovgl_core::core::observer::{ObserverId, SubjectId, SubjectType, USE_FLOAT, log_warn};

#[cfg(feature = "arc")]
impl crate::arc::Arc {
    /// `lv_arc_bind_value()`: bind an integer subject to the arc's value.
    pub fn bind_value(self, ui: &mut Ui, subject: SubjectId) -> Option<ObserverId> {
        todo!()
    }
}

#[cfg(feature = "bar")]
impl crate::bar::Bar {
    /// `lv_bar_bind_value()`: bind an integer or float subject to the bar's
    /// value.
    pub fn bind_value(self, ui: &mut Ui, subject: SubjectId) -> Option<ObserverId> {
        crate::bar::bar_bind_value(ui, self.obj(), subject)
    }
}

#[cfg(feature = "slider")]
impl crate::slider::Slider {
    /// `lv_slider_bind_value()`: bind an integer or float subject to the
    /// slider's value.
    pub fn bind_value(self, ui: &mut Ui, subject: SubjectId) -> Option<ObserverId> {
        crate::slider::slider_bind_value(ui, self.obj(), subject)
    }
}

#[cfg(feature = "label")]
impl crate::label::Label {
    /// `lv_label_bind_text()`: bind an integer, float, string or pointer
    /// subject to the label's text. `fmt` is an optional printf-like format
    /// with one specifier (e.g. `"%d °C"`); with `None` strings are shown as
    /// they are, integers as `%d` and floats as `%0.1f`.
    ///
    /// A pointer subject's value is shown when it is a `&'static str` or a
    /// `String` (the `const char *` of upstream); any other payload counts
    /// as `NULL`.
    pub fn bind_text(self, ui: &mut Ui, subject: SubjectId, fmt: Option<&'static str>) -> Option<ObserverId> {
        if !ui.obj_is_valid(self.obj()) {
            return None; /*LV_CHECK_OBJ*/
        }
        let Some(type_) = ui.observer.subjects.get(subject.0).map(|s| s.type_) else {
            ferrovgl_core::misc::check_arg_failed("lv_label_bind_text: subject != NULL");
            return None;
        };

        let mut fmt = fmt;
        if fmt.is_none() {
            if type_ == SubjectType::Int {
                fmt = Some("%d");
            } else if USE_FLOAT && type_ == SubjectType::Float {
                fmt = Some("%0.1f");
            } else if type_ != SubjectType::String && type_ != SubjectType::Pointer {
                log_warn("Incompatible subject type");
                return None;
            }
        } else if type_ != SubjectType::String
            && type_ != SubjectType::Pointer
            && type_ != SubjectType::Int
            && type_ != SubjectType::Float
        {
            log_warn("Incompatible subject type");
            return None;
        }

        let user_data = fmt.map(|f| Rc::new(f) as Rc<dyn Any>);
        ui.subject_add_observer_obj(
            subject,
            Rc::new(label_text_observer_cb),
            Some(self.obj()),
            user_data,
        )
    }
}

/// The `const char *` a pointer subject's value stands for: a `&'static str`
/// or a `String` payload (`NULL` otherwise).
#[cfg(feature = "label")]
fn pointer_as_string(ptr: Option<Rc<dyn Any>>) -> Option<String> {
    let ptr = ptr?;
    if let Some(s) = (*ptr).downcast_ref::<&'static str>() {
        return Some(String::from(*s));
    }
    (*ptr).downcast_ref::<String>().cloned()
}

/// `label_text_observer_cb()`
#[cfg(feature = "label")]
fn label_text_observer_cb(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some(target) = ui.observer_get_target_obj(observer) else {
        return; /*LV_ASSERT(observer->target != NULL)*/
    };
    let Some(label) = crate::label::Label::from_obj(ui, target) else {
        return; /*LV_CHECK_OBJ of lv_label_set_text()*/
    };
    let fmt: Option<&'static str> = ui
        .observer_get_user_data(observer)
        .and_then(|d| (*d).downcast_ref::<&'static str>().copied());
    let Some(s) = ui.observer.subjects.get(subject.0) else {
        return; /*LV_ASSERT(subject != NULL)*/
    };
    let type_ = s.type_;
    /*`subject->value.pointer`: the string buffer or the pointer*/
    let pointer = match type_ {
        SubjectType::String => s.value.str().map(String::from),
        SubjectType::Pointer => pointer_as_string(s.value.pointer()),
        _ => None,
    };
    let (num, float_v) = (s.value.num(), s.value.float_v_as_f64());

    match fmt {
        None => match pointer {
            Some(text) => label.set_text(ui, &text),
            None => label.refresh_text(ui), /*lv_label_set_text(obj, NULL)*/
        },
        Some(fmt) => {
            let arg = match type_ {
                SubjectType::Int => sprintf::VaArg::Int(num),
                /*LV_USE_FLOAT*/
                SubjectType::Float if USE_FLOAT => sprintf::VaArg::Double(float_v),
                SubjectType::String | SubjectType::Pointer => sprintf::VaArg::Ptr(pointer.as_deref()),
                _ => return,
            };
            let text = sprintf::vsnprintf(fmt, &[arg]);
            label.set_text_fmt(ui, format_args!("{text}"));
        }
    }
}

#[cfg(feature = "image")]
impl crate::image::Image {
    /// `lv_image_bind_src()`: bind a pointer subject to the image's source.
    /// The subject's value is an [`ImageSrc`](ferrovgl_core::misc::style::ImageSrc)
    /// payload; `NULL` or any other payload clears the source.
    pub fn bind_src(self, ui: &mut Ui, subject: SubjectId) -> Option<ObserverId> {
        if !ui.obj_is_valid(self.obj()) {
            return None; /*LV_CHECK_OBJ*/
        }
        let Some(type_) = ui.observer.subjects.get(subject.0).map(|s| s.type_) else {
            ferrovgl_core::misc::check_arg_failed("lv_image_bind_src: subject != NULL");
            return None;
        };
        if type_ != SubjectType::Pointer {
            ferrovgl_core::misc::check_arg_failed("Incompatible subject type");
            return None;
        }

        ui.subject_add_observer_obj(subject, Rc::new(image_src_observer_cb), Some(self.obj()), None)
    }
}

/// `image_src_observer_cb()`
#[cfg(feature = "image")]
fn image_src_observer_cb(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some(target) = ui.observer_get_target_obj(observer) else {
        return; /*LV_ASSERT(observer->target != NULL)*/
    };
    let Some(s) = ui.observer.subjects.get(subject.0) else {
        return; /*LV_ASSERT(subject != NULL)*/
    };
    if s.type_ == SubjectType::Pointer {
        let src = s
            .value
            .pointer()
            .and_then(|p| {
                (*p).downcast_ref::<ferrovgl_core::misc::style::ImageSrc>()
                    .cloned()
            })
            .unwrap_or_default();
        let Some(image) = crate::image::Image::from_obj(ui, target) else {
            return; /*LV_CHECK_OBJ of lv_image_set_src()*/
        };
        image.set_src(ui, src);
    }
}

/// The formatting core of `src/stdlib/builtin/lv_sprintf_builtin.c`
/// (`PRINTF_SUPPORT_FLOAT` follows `LV_USE_FLOAT`; exponential, long long
/// and `ptrdiff_t` support are on, like upstream's defaults).
///
/// The `va_list` is modelled by [`VaArg`]s. Reading an argument of another
/// type than the one passed (or past the last one) is undefined behaviour in
/// C; here it yields 0 / `NULL`. `%s` with `NULL` (an `LV_ASSERT` in
/// `lv_strnlen()`) prints nothing, and the `%pV` nested format has no
/// `lv_vaformat_t` to read and prints nothing.
#[cfg(feature = "label")]
mod sprintf {
    use alloc::string::String;
    use alloc::vec::Vec;

    // 'ntoa' conversion buffer size, this must be big enough to hold one converted
    // numeric number including padded zeros (dynamically created on stack)
    const PRINTF_NTOA_BUFFER_SIZE: usize = 32;
    // 'ftoa' conversion buffer size, this must be big enough to hold one converted
    // float number including padded zeros (dynamically created on stack)
    const PRINTF_FTOA_BUFFER_SIZE: usize = 32;
    // define the default floating point precision
    const PRINTF_DEFAULT_FLOAT_PRECISION: u32 = 6;
    // define the largest float suitable to print with %f
    const PRINTF_MAX_FLOAT: f64 = 1e9;
    // support for the floating point type (%f): `!LV_USE_FLOAT` disables it
    const PRINTF_SUPPORT_FLOAT: bool = ferrovgl_core::core::observer::USE_FLOAT;

    // internal flag definitions
    const FLAGS_ZEROPAD: u32 = 1 << 0;
    const FLAGS_LEFT: u32 = 1 << 1;
    const FLAGS_PLUS: u32 = 1 << 2;
    const FLAGS_SPACE: u32 = 1 << 3;
    const FLAGS_HASH: u32 = 1 << 4;
    const FLAGS_UPPERCASE: u32 = 1 << 5;
    const FLAGS_CHAR: u32 = 1 << 6;
    const FLAGS_SHORT: u32 = 1 << 7;
    const FLAGS_LONG: u32 = 1 << 8;
    const FLAGS_LONG_LONG: u32 = 1 << 9;
    const FLAGS_PRECISION: u32 = 1 << 10;
    const FLAGS_ADAPT_EXP: u32 = 1 << 11;

    /// One variadic argument of the `lv_snprintf()` call.
    #[derive(Clone, Copy)]
    pub(super) enum VaArg<'a> {
        /// An `int` (`int32_t`).
        Int(i32),
        /// A `double` (a `float` after the default argument promotion).
        Double(f64),
        /// A `const char *` (`NULL` = `None`).
        Ptr(Option<&'a str>),
    }

    /// `va_list`
    struct VaList<'a> {
        args: &'a [VaArg<'a>],
        next: usize,
    }

    impl<'a> VaList<'a> {
        fn arg(&mut self) -> Option<VaArg<'a>> {
            let a = self.args.get(self.next).copied();
            self.next += 1;
            a
        }
        /// `va_arg(va, int)`
        fn int(&mut self) -> i32 {
            match self.arg() {
                Some(VaArg::Int(v)) => v,
                _ => 0,
            }
        }
        /// `va_arg(va, long)` / `va_arg(va, long long)`
        fn long(&mut self) -> i64 {
            self.int() as i64
        }
        /// `va_arg(va, unsigned int)`
        fn uint(&mut self) -> u32 {
            self.int() as u32
        }
        /// `va_arg(va, unsigned long)` / `va_arg(va, unsigned long long)`
        fn ulong(&mut self) -> u64 {
            match self.arg() {
                Some(VaArg::Int(v)) => v as u32 as u64,
                Some(VaArg::Ptr(Some(p))) => p.as_ptr() as usize as u64,
                _ => 0,
            }
        }
        /// `va_arg(va, double)`
        fn double(&mut self) -> f64 {
            match self.arg() {
                Some(VaArg::Double(v)) => v,
                _ => 0.0,
            }
        }
        /// `va_arg(va, char *)`
        fn ptr(&mut self) -> Option<&'a str> {
            match self.arg() {
                Some(VaArg::Ptr(p)) => p,
                _ => None,
            }
        }
    }

    // internal test if char is a digit (0-9)
    fn is_digit(ch: u8) -> bool {
        ch.is_ascii_digit()
    }

    // internal ASCII string to unsigned int conversion
    fn atoi(fmt: &[u8], pos: &mut usize) -> u32 {
        let mut i: u32 = 0;
        while is_digit(at(fmt, *pos)) {
            i = i.wrapping_mul(10).wrapping_add((at(fmt, *pos) - b'0') as u32);
            *pos += 1;
        }
        i
    }

    /// `*format`, with the terminating `\0` past the end.
    fn at(fmt: &[u8], pos: usize) -> u8 {
        fmt.get(pos).copied().unwrap_or(0)
    }

    // output the specified string in reverse, taking care of any zero-padding
    fn out_rev(out: &mut Vec<u8>, buf: &[u8], mut len: usize, width: u32, flags: u32) {
        let start_idx = out.len();
        let width = width as usize;

        // pad spaces up to given width
        if flags & FLAGS_LEFT == 0 && flags & FLAGS_ZEROPAD == 0 {
            for _ in len..width {
                out.push(b' ');
            }
        }

        // reverse string
        while len != 0 {
            len -= 1;
            out.push(buf[len]);
        }

        // append pad spaces up to given width
        if flags & FLAGS_LEFT != 0 {
            while out.len() - start_idx < width {
                out.push(b' ');
            }
        }
    }

    // internal itoa format
    #[allow(clippy::too_many_arguments)]
    fn ntoa_format(
        out: &mut Vec<u8>,
        buf: &mut [u8; PRINTF_NTOA_BUFFER_SIZE],
        mut len: usize,
        negative: bool,
        base: u32,
        prec: u32,
        mut width: u32,
        flags: u32,
    ) {
        // pad leading zeros
        if flags & FLAGS_LEFT == 0 {
            if width != 0
                && flags & FLAGS_ZEROPAD != 0
                && (negative || flags & (FLAGS_PLUS | FLAGS_SPACE) != 0)
            {
                width -= 1;
            }
            while len < prec as usize && len < PRINTF_NTOA_BUFFER_SIZE {
                buf[len] = b'0';
                len += 1;
            }
            while flags & FLAGS_ZEROPAD != 0 && len < width as usize && len < PRINTF_NTOA_BUFFER_SIZE {
                buf[len] = b'0';
                len += 1;
            }
        }

        // handle hash
        if flags & FLAGS_HASH != 0 {
            if flags & FLAGS_PRECISION == 0 && len != 0 && (len == prec as usize || len == width as usize) {
                len -= 1;
                if len != 0 && base == 16 {
                    len -= 1;
                }
            }
            if base == 16 && flags & FLAGS_UPPERCASE == 0 && len < PRINTF_NTOA_BUFFER_SIZE {
                buf[len] = b'x';
                len += 1;
            } else if base == 16 && flags & FLAGS_UPPERCASE != 0 && len < PRINTF_NTOA_BUFFER_SIZE {
                buf[len] = b'X';
                len += 1;
            } else if base == 2 && len < PRINTF_NTOA_BUFFER_SIZE {
                buf[len] = b'b';
                len += 1;
            }
            if len < PRINTF_NTOA_BUFFER_SIZE {
                buf[len] = b'0';
                len += 1;
            }
        }

        if len < PRINTF_NTOA_BUFFER_SIZE {
            if negative {
                buf[len] = b'-';
                len += 1;
            } else if flags & FLAGS_PLUS != 0 {
                buf[len] = b'+'; // ignore the space if the '+' exists
                len += 1;
            } else if flags & FLAGS_SPACE != 0 {
                buf[len] = b' ';
                len += 1;
            }
        }

        out_rev(out, &buf[..], len, width, flags);
    }

    // internal itoa for 'long' and 'long long' type (both 64 bits wide)
    fn ntoa_long(
        out: &mut Vec<u8>,
        mut value: u64,
        negative: bool,
        base: u64,
        prec: u32,
        width: u32,
        mut flags: u32,
    ) {
        let mut buf = [0u8; PRINTF_NTOA_BUFFER_SIZE];
        let mut len = 0usize;

        // no hash for 0 values
        if value == 0 {
            flags &= !FLAGS_HASH;
        }

        // write if precision != 0 and value is != 0
        if flags & FLAGS_PRECISION == 0 || value != 0 {
            loop {
                let digit = (value % base) as u8;
                buf[len] = if digit < 10 {
                    b'0' + digit
                } else {
                    (if flags & FLAGS_UPPERCASE != 0 { b'A' } else { b'a' }) + digit - 10
                };
                len += 1;
                value /= base;
                if !(value != 0 && len < PRINTF_NTOA_BUFFER_SIZE) {
                    break;
                }
            }
        }

        ntoa_format(out, &mut buf, len, negative, base as u32, prec, width, flags);
    }

    // internal ftoa for fixed decimal floating point
    #[allow(clippy::nonminimal_bool, clippy::neg_cmp_op_on_partial_ord)] // upstream's NaN-aware test
    fn ftoa(out: &mut Vec<u8>, mut value: f64, mut prec: u32, mut width: u32, flags: u32) {
        let mut buf = [0u8; PRINTF_FTOA_BUFFER_SIZE];
        let mut len = 0usize;

        // powers of 10
        const POW10: [f64; 10] = [
            1.0,
            10.0,
            100.0,
            1000.0,
            10000.0,
            100000.0,
            1000000.0,
            10000000.0,
            100000000.0,
            1000000000.0,
        ];

        // test for special values
        if value.is_nan() {
            return out_rev(out, b"nan", 3, width, flags);
        }
        if value < -f64::MAX {
            return out_rev(out, b"fni-", 4, width, flags);
        }
        if value > f64::MAX {
            let plus = flags & FLAGS_PLUS != 0;
            return out_rev(
                out,
                if plus { b"fni+" } else { b"fni" },
                if plus { 4 } else { 3 },
                width,
                flags,
            );
        }

        // test for very large values
        // standard printf behavior is to print EVERY whole number digit -- which could be 100s of characters overflowing your buffers == bad
        if value > PRINTF_MAX_FLOAT || value < -PRINTF_MAX_FLOAT {
            return etoa(out, value, prec, width, flags);
        }

        // test for negative
        let mut negative = false;
        if value < 0.0 {
            negative = true;
            value = 0.0 - value;
        }

        // set default precision, if not set explicitly
        if flags & FLAGS_PRECISION == 0 {
            prec = PRINTF_DEFAULT_FLOAT_PRECISION;
        }
        // limit precision to 9, cause a prec >= 10 can lead to overflow errors
        while len < PRINTF_FTOA_BUFFER_SIZE && prec > 9 {
            buf[len] = b'0';
            len += 1;
            prec -= 1;
        }

        let mut whole = value as i32;
        let tmp = (value - whole as f64) * POW10[prec as usize];
        let mut frac = tmp as u64;
        let mut diff = tmp - frac as f64;

        if diff > 0.5 {
            frac += 1;
            // handle rollover, e.g. case 0.99 with prec 1 is 1.0
            if frac as f64 >= POW10[prec as usize] {
                frac = 0;
                whole += 1;
            }
        } else if diff < 0.5 {
        } else if frac == 0 || frac & 1 != 0 {
            // if halfway, round up if odd OR if last digit is 0
            frac += 1;
        }

        if prec == 0 {
            diff = value - whole as f64;
            if (!(diff < 0.5) || (diff > 0.5)) && (whole & 1) != 0 {
                // exactly 0.5 and ODD, then round up
                // 1.5 -> 2, but 2.5 -> 2
                whole += 1;
            }
        } else {
            let mut count = prec;
            // now do fractional part, as an unsigned number
            while len < PRINTF_FTOA_BUFFER_SIZE {
                count = count.wrapping_sub(1);
                buf[len] = 48 + (frac % 10) as u8;
                len += 1;
                frac /= 10;
                if frac == 0 {
                    break;
                }
            }
            // add extra 0s
            while len < PRINTF_FTOA_BUFFER_SIZE && {
                let c = count > 0;
                count = count.wrapping_sub(1);
                c
            } {
                buf[len] = b'0';
                len += 1;
            }
            if len < PRINTF_FTOA_BUFFER_SIZE {
                // add decimal
                buf[len] = b'.';
                len += 1;
            }
        }

        // do whole part, number is reversed
        while len < PRINTF_FTOA_BUFFER_SIZE {
            buf[len] = (48 + (whole % 10)) as u8;
            len += 1;
            whole /= 10;
            if whole == 0 {
                break;
            }
        }

        // pad leading zeros
        if flags & FLAGS_LEFT == 0 && flags & FLAGS_ZEROPAD != 0 {
            if width != 0 && (negative || flags & (FLAGS_PLUS | FLAGS_SPACE) != 0) {
                width -= 1;
            }
            while len < width as usize && len < PRINTF_FTOA_BUFFER_SIZE {
                buf[len] = b'0';
                len += 1;
            }
        }

        if len < PRINTF_FTOA_BUFFER_SIZE {
            if negative {
                buf[len] = b'-';
                len += 1;
            } else if flags & FLAGS_PLUS != 0 {
                buf[len] = b'+'; // ignore the space if the '+' exists
                len += 1;
            } else if flags & FLAGS_SPACE != 0 {
                buf[len] = b' ';
                len += 1;
            }
        }

        out_rev(out, &buf, len, width, flags);
    }

    // internal ftoa variant for exponential floating-point type, contributed by Martijn Jasperse <m.jasperse@gmail.com>
    #[allow(clippy::approx_constant)] // upstream's literals, not the exact constants
    fn etoa(out: &mut Vec<u8>, mut value: f64, mut prec: u32, width: u32, mut flags: u32) {
        // check for NaN and special values
        if value.is_nan() || value > f64::MAX || value < -f64::MAX {
            return ftoa(out, value, prec, width, flags);
        }

        // determine the sign
        let negative = value < 0.0;
        if negative {
            value = -value;
        }

        // default precision
        if flags & FLAGS_PRECISION == 0 {
            prec = PRINTF_DEFAULT_FLOAT_PRECISION;
        }

        // determine the decimal exponent
        // based on the algorithm by David Gay (https://www.ampl.com/netlib/fp/dtoa.c)
        let mut conv_u: u64 = value.to_bits();
        let mut exp2 = ((conv_u >> 52) & 0x07FF) as i32 - 1023; // effectively log2
        conv_u = (conv_u & ((1u64 << 52) - 1)) | (1023u64 << 52); // drop the exponent so conv.F is now in [1,2)
        let conv_f = f64::from_bits(conv_u);
        // now approximate log10 from the log2 integer part and an expansion of ln around 1.5
        let mut expval =
            (0.1760912590558 + exp2 as f64 * 0.301029995663981 + (conv_f - 1.5) * 0.289529654602168) as i32;
        // now we want to compute 10^expval but we want to be sure it won't overflow
        exp2 = (expval as f64 * 3.321928094887362 + 0.5) as i32;
        let z = expval as f64 * 2.302585092994046 - exp2 as f64 * 0.6931471805599453;
        let z2 = z * z;
        conv_u = ((exp2 + 1023) as u64) << 52;
        let mut conv_f = f64::from_bits(conv_u);
        // compute exp(z) using continued fractions, see https://en.wikipedia.org/wiki/Exponential_function#Continued_fractions_for_ex
        conv_f *= 1.0 + 2.0 * z / (2.0 - z + (z2 / (6.0 + (z2 / (10.0 + z2 / 14.0)))));
        // correct for rounding errors
        if value < conv_f {
            expval -= 1;
            conv_f /= 10.0;
        }

        // the exponent format is "%+03d" and largest value is "307", so set aside 4-5 characters
        let mut minwidth: u32 = if expval < 100 && expval > -100 { 4 } else { 5 };

        // in "%g" mode, "prec" is the number of *significant figures* not decimals
        if flags & FLAGS_ADAPT_EXP != 0 {
            // do we want to fall-back to "%f" mode?
            if value >= 1e-4 && value < 1e6 {
                if prec as i32 > expval {
                    prec = (prec as i32 - expval - 1) as u32;
                } else {
                    prec = 0;
                }
                flags |= FLAGS_PRECISION; // make sure _ftoa respects precision
                // no characters in exponent
                minwidth = 0;
                expval = 0;
            } else {
                // we use one sigfig for the whole part
                if prec > 0 && flags & FLAGS_PRECISION != 0 {
                    prec -= 1;
                }
            }
        }

        // will everything fit?
        let mut fwidth = width;
        if width > minwidth {
            // we didn't fall-back so subtract the characters required for the exponent
            fwidth -= minwidth;
        } else {
            // not enough characters, so go back to default sizing
            fwidth = 0;
        }
        if flags & FLAGS_LEFT != 0 && minwidth != 0 {
            // if we're padding on the right, DON'T pad the floating part
            fwidth = 0;
        }

        // rescale the float value
        if expval != 0 {
            value /= conv_f;
        }

        // output the floating part
        let start_idx = out.len();
        ftoa(
            out,
            if negative { -value } else { value },
            prec,
            fwidth,
            flags & !FLAGS_ADAPT_EXP,
        );

        // output the exponent part
        if minwidth != 0 {
            // output the exponential symbol
            out.push(if flags & FLAGS_UPPERCASE != 0 { b'E' } else { b'e' });
            // output the exponent value
            ntoa_long(
                out,
                (if expval < 0 { -expval } else { expval }) as u64,
                expval < 0,
                10,
                0,
                minwidth - 1,
                FLAGS_ZEROPAD | FLAGS_PLUS,
            );
            // might need to right-pad spaces
            if flags & FLAGS_LEFT != 0 {
                while out.len() - start_idx < width as usize {
                    out.push(b' ');
                }
            }
        }
    }

    // internal vsnprintf
    fn vsnprintf_inner(out: &mut Vec<u8>, format: &[u8], va: &mut VaList<'_>) {
        let mut f = 0usize;

        while at(format, f) != 0 {
            // format specifier?  %[flags][width][.precision][length]
            if at(format, f) != b'%' {
                // no
                out.push(at(format, f));
                f += 1;
                continue;
            } else {
                // yes, evaluate it
                f += 1;
            }

            // evaluate flags
            let mut flags: u32 = 0;
            loop {
                match at(format, f) {
                    b'0' => flags |= FLAGS_ZEROPAD,
                    b'-' => flags |= FLAGS_LEFT,
                    b'+' => flags |= FLAGS_PLUS,
                    b' ' => flags |= FLAGS_SPACE,
                    b'#' => flags |= FLAGS_HASH,
                    _ => break,
                }
                f += 1;
            }

            // evaluate width field
            let mut width: u32 = 0;
            if is_digit(at(format, f)) {
                width = atoi(format, &mut f);
            } else if at(format, f) == b'*' {
                let w = va.int();
                if w < 0 {
                    flags |= FLAGS_LEFT; // reverse padding
                    width = w.wrapping_neg() as u32;
                } else {
                    width = w as u32;
                }
                f += 1;
            }

            // evaluate precision field
            let mut precision: u32 = 0;
            if at(format, f) == b'.' {
                flags |= FLAGS_PRECISION;
                f += 1;
                if is_digit(at(format, f)) {
                    precision = atoi(format, &mut f);
                } else if at(format, f) == b'*' {
                    let prec = va.int();
                    precision = if prec > 0 { prec as u32 } else { 0 };
                    f += 1;
                }
            }

            // evaluate length field (`ptrdiff_t`, `intmax_t` and `size_t` are as wide as `long`)
            match at(format, f) {
                b'l' => {
                    flags |= FLAGS_LONG;
                    f += 1;
                    if at(format, f) == b'l' {
                        flags |= FLAGS_LONG_LONG;
                        f += 1;
                    }
                }
                b'h' => {
                    flags |= FLAGS_SHORT;
                    f += 1;
                    if at(format, f) == b'h' {
                        flags |= FLAGS_CHAR;
                        f += 1;
                    }
                }
                b't' | b'j' | b'z' => {
                    flags |= FLAGS_LONG;
                    f += 1;
                }
                _ => {}
            }

            // evaluate specifier
            match at(format, f) {
                b'd' | b'i' | b'u' | b'x' | b'X' | b'p' | b'P' | b'o' | b'b' => {
                    // set the base
                    let base: u32;
                    let c = at(format, f);
                    if c == b'x' || c == b'X' {
                        base = 16;
                    } else if c == b'p' || c == b'P' {
                        base = 16;
                        flags |= FLAGS_HASH; // always hash for pointer format
                        // sizeof(uintptr_t) == sizeof(long long)
                        flags |= FLAGS_LONG_LONG;

                        if at(format, f + 1) == b'V' {
                            f += 1;
                        }
                    } else if c == b'o' {
                        base = 8;
                    } else if c == b'b' {
                        base = 2;
                    } else {
                        base = 10;
                        flags &= !FLAGS_HASH; // no hash for dec format
                    }
                    let c = at(format, f);
                    // uppercase
                    if c == b'X' || c == b'P' {
                        flags |= FLAGS_UPPERCASE;
                    }

                    // no plus or space flag for u, x, X, o, b
                    if c != b'i' && c != b'd' {
                        flags &= !(FLAGS_PLUS | FLAGS_SPACE);
                    }

                    // ignore '0' flag when precision is given
                    if flags & FLAGS_PRECISION != 0 {
                        flags &= !FLAGS_ZEROPAD;
                    }

                    // convert the integer
                    if c == b'i' || c == b'd' {
                        // signed
                        if flags & (FLAGS_LONG_LONG | FLAGS_LONG) != 0 {
                            let value = va.long();
                            ntoa_long(
                                out,
                                if value > 0 {
                                    value as u64
                                } else {
                                    0u64.wrapping_sub(value as u64)
                                },
                                value < 0,
                                base as u64,
                                precision,
                                width,
                                flags,
                            );
                        } else {
                            let v = va.int();
                            let value: i32 = if flags & FLAGS_CHAR != 0 {
                                v as i8 as i32
                            } else if flags & FLAGS_SHORT != 0 {
                                v as i16 as i32
                            } else {
                                v
                            };
                            ntoa_long(
                                out,
                                (if value > 0 {
                                    value
                                } else {
                                    0i32.wrapping_sub(value)
                                }) as u32 as u64,
                                value < 0,
                                base as u64,
                                precision,
                                width,
                                flags,
                            );
                        }
                    } else if c == b'V' {
                        /*`lv_vaformat_t *`: not representable, see the module doc*/
                        let _ = va.arg();
                    } else {
                        // unsigned
                        if flags & (FLAGS_LONG_LONG | FLAGS_LONG) != 0 {
                            let v = va.ulong();
                            ntoa_long(out, v, false, base as u64, precision, width, flags);
                        } else {
                            let v = va.uint();
                            let value: u32 = if flags & FLAGS_CHAR != 0 {
                                v as u8 as u32
                            } else if flags & FLAGS_SHORT != 0 {
                                v as u16 as u32
                            } else {
                                v
                            };
                            ntoa_long(out, value as u64, false, base as u64, precision, width, flags);
                        }
                    }
                    f += 1;
                }
                b'f' | b'F' if PRINTF_SUPPORT_FLOAT => {
                    if at(format, f) == b'F' {
                        flags |= FLAGS_UPPERCASE;
                    }
                    let v = va.double();
                    ftoa(out, v, precision, width, flags);
                    f += 1;
                }
                b'e' | b'E' | b'g' | b'G' if PRINTF_SUPPORT_FLOAT => {
                    let c = at(format, f);
                    if c == b'g' || c == b'G' {
                        flags |= FLAGS_ADAPT_EXP;
                    }
                    if c == b'E' || c == b'G' {
                        flags |= FLAGS_UPPERCASE;
                    }
                    let v = va.double();
                    etoa(out, v, precision, width, flags);
                    f += 1;
                }
                b'c' => {
                    let mut l: u32 = 1;
                    // pre padding
                    if flags & FLAGS_LEFT == 0 {
                        while {
                            let c = l < width;
                            l = l.wrapping_add(1);
                            c
                        } {
                            out.push(b' ');
                        }
                    }
                    // char output
                    out.push(va.int() as u8);
                    // post padding
                    if flags & FLAGS_LEFT != 0 {
                        while {
                            let c = l < width;
                            l = l.wrapping_add(1);
                            c
                        } {
                            out.push(b' ');
                        }
                    }
                    f += 1;
                }
                b's' => {
                    let p = va.ptr().unwrap_or("").as_bytes();
                    let max_len = if precision != 0 {
                        precision as usize
                    } else {
                        usize::MAX
                    };
                    let mut l = p.iter().take(max_len).take_while(|&&b| b != 0).count() as u32;
                    // pre padding
                    if flags & FLAGS_PRECISION != 0 {
                        l = if l < precision { l } else { precision };
                    }
                    if flags & FLAGS_LEFT == 0 {
                        while {
                            let c = l < width;
                            l = l.wrapping_add(1);
                            c
                        } {
                            out.push(b' ');
                        }
                    }
                    // string output
                    let mut i = 0usize;
                    while i < p.len()
                        && p[i] != 0
                        && (flags & FLAGS_PRECISION == 0 || {
                            let c = precision != 0;
                            precision = precision.wrapping_sub(1);
                            c
                        })
                    {
                        out.push(p[i]);
                        i += 1;
                    }
                    // post padding
                    if flags & FLAGS_LEFT != 0 {
                        while {
                            let c = l < width;
                            l = l.wrapping_add(1);
                            c
                        } {
                            out.push(b' ');
                        }
                    }
                    f += 1;
                }
                b'%' => {
                    out.push(b'%');
                    f += 1;
                }
                0 => {
                    /*`%` at the end: upstream writes the `\0` and reads past the
                     * terminator (undefined behaviour); stop here instead*/
                    out.push(0);
                    break;
                }
                c => {
                    out.push(c);
                    f += 1;
                }
            }
        }
    }

    /// `lv_text_set_text_vfmt()` with the builtin `lv_vsnprintf()`: the
    /// whole formatted text (measured with a `NULL` buffer, then written into
    /// a `len + 1` buffer, so nothing is truncated).
    pub(super) fn vsnprintf(format: &str, args: &[VaArg<'_>]) -> String {
        let mut out = Vec::new();
        let mut va = VaList { args, next: 0 };
        vsnprintf_inner(&mut out, format.as_bytes(), &mut va);
        String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn fmt1(f: &str, a: VaArg<'_>) -> String {
            vsnprintf(f, &[a])
        }

        #[test]
        fn integers() {
            assert_eq!(fmt1("%d", VaArg::Int(10)), "10");
            assert_eq!(fmt1("value: %d", VaArg::Int(-20)), "value: -20");
            assert_eq!(fmt1("%5d|", VaArg::Int(42)), "   42|");
            assert_eq!(fmt1("%-5d|", VaArg::Int(42)), "42   |");
            assert_eq!(fmt1("%05d", VaArg::Int(-42)), "-0042");
            assert_eq!(fmt1("%+d", VaArg::Int(7)), "+7");
            assert_eq!(fmt1("%x", VaArg::Int(255)), "ff");
            assert_eq!(fmt1("%#X", VaArg::Int(255)), "0XFF");
            assert_eq!(fmt1("%u", VaArg::Int(-1)), "4294967295");
            assert_eq!(fmt1("%d", VaArg::Int(i32::MIN)), "-2147483648");
            assert_eq!(fmt1("%.0d", VaArg::Int(0)), "");
            assert_eq!(fmt1("%hhd", VaArg::Int(300)), "44");
            assert_eq!(fmt1("100%%", VaArg::Int(0)), "100%");
        }

        #[test]
        fn floats() {
            if !PRINTF_SUPPORT_FLOAT {
                return;
            }
            assert_eq!(fmt1("%0.1f", VaArg::Double(10.5)), "10.5");
            assert_eq!(fmt1("Value: %0.2f", VaArg::Double(81.5)), "Value: 81.50");
            assert_eq!(fmt1("%f", VaArg::Double(1.0)), "1.000000");
            assert_eq!(fmt1("%.0f", VaArg::Double(2.5)), "2");
            assert_eq!(fmt1("%.0f", VaArg::Double(1.5)), "2");
            assert_eq!(fmt1("%8.3f", VaArg::Double(-3.14259)), "  -3.143");
            assert_eq!(fmt1("%e", VaArg::Double(12345.678)), "1.234568e+04");
            // no trailing-zero removal in the builtin `%g`
            assert_eq!(fmt1("%g", VaArg::Double(0.5)), "0.500000");
            assert_eq!(fmt1("%f", VaArg::Double(f64::NAN)), "nan");
            assert_eq!(fmt1("%f", VaArg::Double(f64::NEG_INFINITY)), "-inf");
        }

        #[test]
        fn strings() {
            assert_eq!(fmt1("text: %s", VaArg::Ptr(Some("hello"))), "text: hello");
            assert_eq!(fmt1("%.3s", VaArg::Ptr(Some("hello"))), "hel");
            assert_eq!(fmt1("%7s|", VaArg::Ptr(Some("abc"))), "    abc|");
            assert_eq!(fmt1("%-7s|", VaArg::Ptr(Some("abc"))), "abc    |");
            assert_eq!(fmt1("%c%c", VaArg::Int(b'A' as i32)), "A\0");
        }
    }
}
