macro_rules! __ {
    ($($d:tt $name:ident),* $(,)?) => {
        $(
            #[allow(unused_macros)]
            macro_rules! $name {
                ($d ($d tt:tt)*) => {{
                    #[cfg(feature = "defmt")]
                    ::defmt::$name!($d ($d tt)*)
                }};
            }
        )*
    };
}

__! { $trace, $debug, $info, $warn, $error }

#[cfg(feature = "rgb")]
macro_rules! ltable {
    ($vis:vis $name:ident[$ty:ty] => {
        left: [$($litem:expr),* $(,)?],
        right: [$($ritem:expr),* $(,)?] $(,)?
    }) => {
        $vis const $name: [$ty; ltable!(__@keys)] = ltable!(__@{
            left: [$($litem),*],
            right: [$($ritem),*],
        });
    };
    (__@{
        left: [$($litem:expr),* $(,)?],
        right: [$($ritem:expr),* $(,)?] $(,)?
    }) => {{
        #[cfg(not(feature = "halfmoon"))]
        { [ $($litem,)* $($ritem,)* ] }

        #[cfg(feature = "halfmoon")]
        { [ $($litem,)* ] }
    }};
    (__@keys) => {{
        #[cfg(not(feature = "halfmoon"))]
        { $crate::keyboard::KEYS }

        #[cfg(feature = "halfmoon")]
        { $crate::keyboard::KEYS / 2 }
    }}
}

macro_rules! matrix {
    ($vis:vis $name:ident[$ty:ty] => {
        left: [$([$($litem:expr),*]),* $(,)?],
        right: [$([$($ritem:expr),*]),* $(,)?] $(,)?
    }) => {
        $vis const $name: [[$ty; $crate::keyboard::COLS]; matrix!(__@rows)] = matrix!(__@{
            left: [$([$($litem),*]),*],
            right: [$([$($ritem),*]),*],
        });
    };
    (__@{
        left: [$([$($litem:expr),*]),* $(,)?],
        right: [$([$($ritem:expr),*]),* $(,)?] $(,)?
    }) => {{
        #[cfg(not(feature = "halfmoon"))]
        { [ $([$($litem,)*],)* $([$($ritem,)*],)* ] }

        #[cfg(feature = "halfmoon")]
        { [ $([$($litem,)*],)* ] }
    }};
    (__@rows) => {{
        #[cfg(not(feature = "halfmoon"))]
        { $crate::keyboard::ROWS }

        #[cfg(feature = "halfmoon")]
        { $crate::keyboard::ROWS / 2 }
    }}
}

macro_rules! keymaps {
    ($vis:vis $name:ident[$lnum:expr] => [$($lyr:ident => {
        left: [$([$($litem:expr),*]),* $(,)?],
        right: [$([$($ritem:expr),*]),* $(,)?] $(,)?
    }),* $(,)?]) => {
        $vis const $name: [[[::rmk::types::action::KeyAction; $crate::keyboard::COLS]; matrix!(__@rows)] ;$lnum] = [
            $(matrix!(__@{
                left: [$([$($litem),*]),*],
                right: [$([$($ritem),*]),*],
            }),)*
        ];
    };
}
