#![allow(
    unused,
    reason = "items in the right-side keymaps may be unused when the `halfmoon` feature is enabled"
)]

use embassy_time::Duration;
use rmk::{
    a,
    config::{BehaviorConfig, Hand},
    k, kbctrl, light, lt, mo, mt, shifted, tg,
    types::{action::KeyAction, modifier::ModifierCombination as Mc},
    wm,
};

use alias::*;

#[allow(non_upper_case_globals)]
mod alias {
    use super::*;

    pub(super) const xxxxxx: KeyAction = k!(No);
    pub(super) const ______: KeyAction = a!(Transparent);

    pub(super) const Hyper: KeyAction = wm!(No, Mc::new_from(false, true, true, true, true));
    pub(super) const Meh: KeyAction = wm!(No, Mc::new_from(true, false, true, true, true));
}

#[cfg(feature = "vial")]
pub(super) mod vial {
    include!(concat!(env!("OUT_DIR"), "/vial_gen_config.rs"));
}

const LAYER_NUM: usize = 3;

keymaps!(pub(super) KEYMAPS[LAYER_NUM] => [
    Base => {
        left: [
            [k!(Equal),     k!(Kc1),           k!(Kc2),  k!(Kc3),                    k!(Kc4),   k!(Kc5), k!(Left)],
            [k!(Delete),    k!(Q),             k!(W),    k!(E),                      k!(R),     k!(T),   tg!(1)],
            [k!(Backspace), k!(A),             k!(S),    k!(D),                      k!(F),     k!(G),   Hyper],
            [k!(LShift),    mt!(Z, Mc::LCTRL), k!(X),    k!(C),                      k!(V),     k!(B),   xxxxxx],
            [lt!(1, Grave), a!(No),            k!(X),    k!(Left),                   k!(Right), xxxxxx,  xxxxxx],
            [k!(Space),     k!(Backspace),     k!(LGui), mt!(Application, Mc::LALT), xxxxxx,    xxxxxx,  xxxxxx],
        ],
        right: [
            [k!(Right), k!(Kc6), k!(Kc7), k!(Kc8),                k!(Kc9),         k!(Kc0),               k!(Minus)],
            [tg!(1),    k!(Y),   k!(U),   k!(I),                  k!(O),           k!(P),                 k!(Backslash)],
            [Meh,       k!(H),   k!(J),   k!(K),                  k!(L),           lt!(2, Semicolon),     mt!(Quote, Mc::LGUI)],
            [xxxxxx,    k!(N),   k!(M),   k!(Comma),              k!(Dot),         mt!(Slash, Mc::RCTRL), k!(RShift)],
            [xxxxxx,    xxxxxx,  k!(Up),  k!(Down),               k!(LeftBracket), k!(RightBracket),      mo!(2)],
            [xxxxxx,    xxxxxx,  xxxxxx,  mt!(Escape, Mc::RCTRL), k!(LAlt),        k!(Tab),               k!(Enter)]
        ]
    },
    Symbols => {
        left: [
            [______,               k!(F1),         k!(F2),         k!(F3),                k!(F4),                 k!(F5),              k!(F6)],
            [______,               shifted!(Kc1),  shifted!(Kc2),  shifted!(LeftBracket), shifted!(RightBracket), shifted!(Backslash), ______],
            [______,               shifted!(Kc3),  shifted!(Kc4),  shifted!(Kc9),         shifted!(Kc0),          k!(Grave),           ______],
            [______,               shifted!(Kc5),  shifted!(Kc6),  k!(LeftBracket),       k!(RightBracket),       shifted!(Grave),     xxxxxx],
            [kbctrl!(ClearEeprom), ______,         ______,         ______,                ______,                 xxxxxx,              xxxxxx],
            [light!(RgbHud),       light!(RgbVad), light!(RgbHui), light!(RgbVai),        xxxxxx,                 xxxxxx,              xxxxxx],
        ],
        right: [
            [k!(F7), k!(F8),        k!(F9),  k!(F10),        k!(F11),                k!(F12),        ______],
            [______, k!(Up),        k!(Kc7), k!(Kc8),        k!(Kc9),                k!(KpAsterisk), ______],
            [______, k!(Down),      k!(Kc4), k!(Kc5),        k!(Kc6),                k!(KpPlus),     ______],
            [xxxxxx, shifted!(Kc7), k!(Kc1), k!(Kc2),        k!(Kc3),                k!(Backslash),  ______],
            [xxxxxx, xxxxxx,        ______,  k!(Dot),        k!(Kc0),                k!(Equal),      ______],
            [xxxxxx, xxxxxx,        xxxxxx,  light!(RgbTog), light!(RgbModeForward), ______,         ______]
        ],
    },
    Media => {
        left: [
            [______, ______, ______,        ______,        ______,         ______, ______],
            [______, ______, ______,        k!(MouseUp),   ______,         ______, ______],
            [______, ______, k!(MouseLeft), k!(MouseDown), k!(MouseRight), ______, ______],
            [______, ______, ______,        ______,        ______,         ______, xxxxxx],
            [______, ______, ______,        k!(MouseBtn1), k!(MouseBtn2),  xxxxxx, xxxxxx],
            [______, ______, ______,        ______,        xxxxxx,         xxxxxx, xxxxxx],
        ],
        right: [
            [______, ______, ______,             ______,             ______,             ______, kbctrl!(Bootloader)],
            [______, ______, ______,             ______,             ______,             ______, ______],
            [______, ______, k!(MediaPrevTrack), k!(MediaPlayPause), k!(MediaNextTrack), ______, ______],
            [xxxxxx, ______, k!(KbVolumeUp),     k!(KbVolumeDown),   k!(KbMute),         ______, ______],
            [xxxxxx, xxxxxx, ______,             ______,             ______,             ______, ______],
            [xxxxxx, xxxxxx, xxxxxx,             ______,             ______,             ______, ______]
        ]
    },
]);

matrix!(pub(super) CHORDAL_HOLD_LAYOUT[Hand] => {
    left: [
        [Hand::Left, Hand::Left, Hand::Left, Hand::Left, Hand::Left,    Hand::Left,    Hand::Left],
        [Hand::Left, Hand::Left, Hand::Left, Hand::Left, Hand::Left,    Hand::Left,    Hand::Left],
        [Hand::Left, Hand::Left, Hand::Left, Hand::Left, Hand::Left,    Hand::Left,    Hand::Left],
        [Hand::Left, Hand::Left, Hand::Left, Hand::Left, Hand::Left,    Hand::Left,    Hand::Unknown],
        [Hand::Left, Hand::Left, Hand::Left, Hand::Left, Hand::Left,    Hand::Unknown, Hand::Unknown],
        [Hand::Left, Hand::Left, Hand::Left, Hand::Left, Hand::Unknown, Hand::Unknown, Hand::Unknown],
    ],
    right: [
        [Hand::Right,   Hand::Right,   Hand::Right,   Hand::Right, Hand::Right, Hand::Right, Hand::Right],
        [Hand::Right,   Hand::Right,   Hand::Right,   Hand::Right, Hand::Right, Hand::Right, Hand::Right],
        [Hand::Right,   Hand::Right,   Hand::Right,   Hand::Right, Hand::Right, Hand::Right, Hand::Right],
        [Hand::Unknown, Hand::Right,   Hand::Right,   Hand::Right, Hand::Right, Hand::Right, Hand::Right],
        [Hand::Unknown, Hand::Unknown, Hand::Right,   Hand::Right, Hand::Right, Hand::Right, Hand::Right],
        [Hand::Unknown, Hand::Unknown, Hand::Unknown, Hand::Right, Hand::Right, Hand::Right, Hand::Right],
    ],
});

#[inline(always)]
pub(super) fn behavior_config() -> BehaviorConfig {
    let mut config = BehaviorConfig::default();
    config.one_shot.timeout = Duration::from_millis(500);
    config
}
