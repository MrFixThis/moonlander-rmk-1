#[cfg(feature = "defmt")]
use defmt_rtt as _;

macro_rules! __ {
    ($($d:tt $name:ident),* $(,)?) => {
        $(
            #[allow(unused_macros)]
            macro_rules! $name {
                ($d ($d tt:tt)*) => {
                    #[cfg(feature = "defmt")]
                    ::defmt::$name!($d ($d tt)*)
                };
            }
        )*
    };
}

__! { $trace, $debug, $info, $warn, $error }
