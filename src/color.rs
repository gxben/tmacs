//! Colors: X11/Emacs color names, hex parsing and 256-color fallback.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

const NAMES: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff),
    ("antiquewhite", 0xfaebd7),
    ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4),
    ("aquamarine1", 0x7fffd4),
    ("aquamarine2", 0x76eec6),
    ("aquamarine3", 0x66cdaa),
    ("aquamarine4", 0x458b74),
    ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4),
    ("black", 0x000000),
    ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff),
    ("blue1", 0x0000ff),
    ("blue2", 0x0000ee),
    ("blue3", 0x0000cd),
    ("blue4", 0x00008b),
    ("blueviolet", 0x8a2be2),
    ("brown", 0xa52a2a),
    ("brown1", 0xff4040),
    ("brown2", 0xee3b3b),
    ("brown3", 0xcd3333),
    ("brown4", 0x8b2323),
    ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0),
    ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e),
    ("coral", 0xff7f50),
    ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c),
    ("cyan", 0x00ffff),
    ("cyan1", 0x00ffff),
    ("cyan2", 0x00eeee),
    ("cyan3", 0x00cdcd),
    ("cyan4", 0x008b8b),
    ("darkblue", 0x00008b),
    ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00),
    ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a),
    ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f),
    ("darkslategrey", 0x2f4f4f),
    ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222),
    ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff),
    ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700),
    ("goldenrod", 0xdaa520),
    ("gray", 0xbebebe),
    ("green", 0x00ff00),
    ("green1", 0x00ff00),
    ("green2", 0x00ee00),
    ("green3", 0x00cd00),
    ("green4", 0x008b00),
    ("greenyellow", 0xadff2f),
    ("grey", 0xbebebe),
    ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4),
    ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082),
    ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd),
    ("lightblue", 0xadd8e6),
    ("lightblue1", 0xbfefff),
    ("lightblue2", 0xb2dfee),
    ("lightblue3", 0x9ac0cd),
    ("lightblue4", 0x68838b),
    ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff),
    ("lightgoldenrod", 0xeedd82),
    ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90),
    ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa),
    ("lightslateblue", 0x8470ff),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0),
    ("lime", 0x00ff00),
    ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff),
    ("maroon", 0xb03060),
    ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd),
    ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db),
    ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a),
    ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead),
    ("navy", 0x000080),
    ("navyblue", 0x000080),
    ("oldlace", 0xfdf5e6),
    ("olive", 0x808000),
    ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500),
    ("orange1", 0xffa500),
    ("orange2", 0xee9a00),
    ("orange3", 0xcd8500),
    ("orange4", 0x8b5a00),
    ("orangered", 0xff4500),
    ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa),
    ("palegreen", 0x98fb98),
    ("palegreen1", 0x9aff9a),
    ("palegreen2", 0x90ee90),
    ("palegreen3", 0x7ccd7c),
    ("palegreen4", 0x548b54),
    ("paleturquoise", 0xafeeee),
    ("paleturquoise1", 0xbbffff),
    ("paleturquoise2", 0xaeeeee),
    ("paleturquoise3", 0x96cdcd),
    ("paleturquoise4", 0x668b8b),
    ("palevioletred", 0xdb7093),
    ("palevioletred1", 0xff82ab),
    ("palevioletred2", 0xee799f),
    ("palevioletred3", 0xcd6889),
    ("palevioletred4", 0x8b475d),
    ("papayawhip", 0xffefd5),
    ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f),
    ("pink", 0xffc0cb),
    ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6),
    ("purple", 0xa020f0),
    ("red", 0xff0000),
    ("red1", 0xff0000),
    ("red2", 0xee0000),
    ("red3", 0xcd0000),
    ("red4", 0x8b0000),
    ("rosybrown", 0xbc8f8f),
    ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57),
    ("seagreen1", 0x54ff9f),
    ("seagreen2", 0x4eee94),
    ("seagreen3", 0x43cd80),
    ("seagreen4", 0x2e8b57),
    ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd),
    ("slateblue1", 0x836fff),
    ("slateblue2", 0x7a67ee),
    ("slateblue3", 0x6959cd),
    ("slateblue4", 0x473c8b),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4),
    ("steelblue1", 0x63b8ff),
    ("steelblue2", 0x5cacee),
    ("steelblue3", 0x4f94cd),
    ("steelblue4", 0x36648b),
    ("tan", 0xd2b48c),
    ("teal", 0x008080),
    ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee),
    ("wheat", 0xf5deb3),
    ("wheat1", 0xffe7ba),
    ("wheat2", 0xeed8ae),
    ("wheat3", 0xcdba96),
    ("wheat4", 0x8b7e66),
    ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00),
    ("yellow1", 0xffff00),
    ("yellow2", 0xeeee00),
    ("yellow3", 0xcdcd00),
    ("yellow4", 0x8b8b00),
    ("yellowgreen", 0x9acd32),
];

pub fn parse(s: &str) -> Option<Rgb> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let v = u32::from_str_radix(hex, 16).ok()?;
            return Some(rgb(v));
        }
        if hex.len() == 3 {
            let v = u32::from_str_radix(hex, 16).ok()?;
            let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
            return Some(Rgb((r * 17) as u8, (g * 17) as u8, (b * 17) as u8));
        }
        return None;
    }
    let name: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    // grayNN / greyNN (0..=100)
    for p in ["gray", "grey"] {
        if let Some(n) = name.strip_prefix(p) {
            if let Ok(n) = n.parse::<u32>() {
                if n <= 100 {
                    let v = ((n as f32) * 2.55).round() as u8;
                    return Some(Rgb(v, v, v));
                }
            }
        }
    }
    NAMES
        .binary_search_by(|(n, _)| (*n).cmp(name.as_str()))
        .ok()
        .map(|i| rgb(NAMES[i].1))
}

fn rgb(v: u32) -> Rgb {
    Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Nearest xterm-256 palette index for terminals without truecolor.
pub fn to_ansi256(c: Rgb) -> u8 {
    let lv = [0u8, 95, 135, 175, 215, 255];
    let idx = |v: u8| -> usize {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            ((v as usize - 35) / 40).min(5)
        }
    };
    let (ri, gi, bi) = (idx(c.0), idx(c.1), idx(c.2));
    let cube = Rgb(lv[ri], lv[gi], lv[bi]);
    let avg = (c.0 as u32 + c.1 as u32 + c.2 as u32) / 3;
    let gray_i = if avg > 238 {
        23
    } else {
        (avg.saturating_sub(3)) / 10
    };
    let gv = (8 + gray_i * 10) as u8;
    let gray = Rgb(gv, gv, gv);
    let d = |a: Rgb, b: Rgb| {
        let dr = a.0 as i32 - b.0 as i32;
        let dg = a.1 as i32 - b.1 as i32;
        let db = a.2 as i32 - b.2 as i32;
        dr * dr + dg * dg + db * db
    };
    if d(c, gray) < d(c, cube) {
        232 + gray_i as u8
    } else {
        (16 + 36 * ri + 6 * gi + bi) as u8
    }
}

pub fn truecolor_supported() -> bool {
    if let Ok(v) = std::env::var("COLORTERM") {
        if v == "truecolor" || v == "24bit" {
            return true;
        }
    }
    if let Ok(t) = std::env::var("TERM_PROGRAM") {
        if t != "Apple_Terminal" {
            return true;
        }
        return false;
    }
    std::env::var("TERM")
        .map(|t| t.contains("direct") || t.contains("kitty") || t.contains("alacritty"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_sorted() {
        for w in NAMES.windows(2) {
            assert!(w[0].0 < w[1].0, "{} >= {}", w[0].0, w[1].0);
        }
    }
    #[test]
    fn parse_names() {
        assert_eq!(parse("DarkSlateGray"), Some(Rgb(0x2f, 0x4f, 0x4f)));
        assert_eq!(parse("wheat"), Some(Rgb(0xf5, 0xde, 0xb3)));
        assert_eq!(parse("gray50"), Some(Rgb(128, 128, 128)));
        assert_eq!(parse("#ff0000"), Some(Rgb(255, 0, 0)));
        assert_eq!(parse("light slate blue"), Some(Rgb(0x84, 0x70, 0xff)));
    }
}
