//! Widget themes. Each module documents its implemented widget coverage.

/// Declare a theme's `my_theme_styles_t` as a generic struct of named fields.
///
/// `Styles<Style>` holds the values built by `style_init()`, `Styles<StyleId>`
/// the shared styles registered in `Ui`. Fields keep the C struct order; a
/// field may carry `#[cfg(...)]` for its upstream `#if LV_USE_<WIDGET>`. New
/// widgets add fields here without renumbering any existing style.
#[cfg(any(feature = "theme-default", feature = "theme-simple"))]
macro_rules! theme_styles {
    ($(#[$attr:meta])* struct $name:ident { $( $(#[cfg($cfg:meta)])? $field:ident, )* }) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        struct $name<T> {
            $( $(#[cfg($cfg)])? $field: T, )*
        }

        impl<T> $name<T> {
            /// One value per style, in field (C struct) order.
            fn from_fn(mut f: impl FnMut() -> T) -> Self {
                Self { $( $(#[cfg($cfg)])? $field: f(), )* }
            }

            /// Transform every style, in field (C struct) order.
            fn map<U>(self, mut f: impl FnMut(T) -> U) -> $name<U> {
                $name { $( $(#[cfg($cfg)])? $field: f(self.$field), )* }
            }

            /// Pair the styles of two sets field by field.
            fn zip<U>(self, other: $name<U>) -> $name<(T, U)> {
                $name { $( $(#[cfg($cfg)])? $field: (self.$field, other.$field), )* }
            }
        }
    };
}

#[cfg(feature = "theme-default")]
pub mod default;

#[cfg(feature = "theme-simple")]
pub mod simple;
