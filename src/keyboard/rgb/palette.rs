use rmk_palettefx::palette::{self, Palette};

pub(super) const COLORS: [&Palette; 19] = [
    &palette::AFTERBURN,
    &palette::AMBER,
    &palette::BADWOLF,
    &palette::CARNIVAL,
    &palette::CLASSIC,
    &palette::DRACULA,
    &palette::GROOVY,
    &palette::NOTPINK,
    &palette::PHOSPHOR,
    &palette::POLARIZED,
    &palette::ROSEGOLD,
    &palette::SPORT,
    &palette::SYNTHWAVE,
    &palette::THERMAL,
    &palette::VIRIDIS,
    &palette::WATERMELON,
    // List custom palettes here
    &BEETBIT,
    &CONSOLE,
    &POP,
];

// Colors from `LOSPEC Palettes`

const BEETBIT: Palette = [
    palette::hsv16(36, 76, 255),   // #fff5b5
    palette::hsv16(77, 186, 207),  // #38be77
    palette::hsv16(92, 216, 142),  // #228f55
    palette::hsv16(106, 255, 85),  // #126133
    palette::hsv16(51, 255, 117),  // #74740a
    palette::hsv16(43, 255, 178),  // #b2ad14
    palette::hsv16(29, 216, 255),  // #ffbe3f
    palette::hsv16(15, 255, 219),  // #d16110
    palette::hsv16(10, 255, 142),  // #872506
    palette::hsv16(241, 255, 94),  // #58061e
    palette::hsv16(232, 255, 145), // #910631
    palette::hsv16(235, 255, 242), // #f1164b
    palette::hsv16(199, 85, 163),  // #a98fa3
    palette::hsv16(205, 85, 112),  // #6e4d70
    palette::hsv16(218, 117, 71),  // #471847
    palette::hsv16(223, 117, 36),  // #23060f
];

pub const CONSOLE: Palette = [
    palette::hsv16(254, 255, 255), // #ff032b
    palette::hsv16(243, 255, 128), // #800034
    palette::hsv16(42, 255, 255),  // #ffff0d
    palette::hsv16(24, 255, 255),  // #ff8f00
    palette::hsv16(85, 255, 255),  // #0aff0a
    palette::hsv16(121, 255, 112), // #007062
    palette::hsv16(128, 255, 255), // #0dffff
    palette::hsv16(151, 186, 219), // #3c80db
    palette::hsv16(170, 215, 255), // #2929ff
    palette::hsv16(188, 255, 110), // #2d066e
    palette::hsv16(213, 255, 255), // #ff08ff
    palette::hsv16(205, 255, 133), // #6e0085
    palette::hsv16(194, 215, 58),  // #260a34
    palette::hsv16(0, 0, 0),       // #000000
    palette::hsv16(0, 0, 255),     // #ffffff
    palette::hsv16(161, 110, 218), // #7d7da3
];

const POP: Palette = [
    palette::hsv16(230, 255, 240), // #ff306c
    palette::hsv16(215, 220, 235), // #ebc1
    palette::hsv16(225, 255, 173), // #ad005f
    palette::hsv16(18, 255, 224),  // #e04f0b
    palette::hsv16(25, 255, 255),  // #ffa200
    palette::hsv16(33, 180, 255),  // #ffe74c
    palette::hsv16(50, 45, 255),   // #ebffd4
    palette::hsv16(60, 255, 255),  // #48ff00
    palette::hsv16(75, 255, 207),  // #00cf2d
    palette::hsv16(95, 220, 224),  // #31d4e0
    palette::hsv16(110, 255, 194), // #006ec2
    palette::hsv16(150, 240, 247), // #1629f7
    palette::hsv16(190, 255, 115), // #450073
    palette::hsv16(205, 220, 199), // #8e1cc7
    palette::hsv16(215, 220, 240), // #f018f0
    palette::hsv16(230, 255, 240), // #ff306c
];
