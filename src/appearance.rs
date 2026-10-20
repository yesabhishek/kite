use iced::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    Dark,
    Eucalyptus,
    Midnight,
    Graphite,
    Porcelain,
}
impl std::fmt::Display for Appearance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Dark => "Dark",
            Self::Eucalyptus => "Eucalyptus",
            Self::Midnight => "Midnight",
            Self::Graphite => "Graphite",
            Self::Porcelain => "Paper",
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Colors {
    /// The editor and terminal surface.
    pub bg: Color,
    /// Sidebar, title bar, and other persistent chrome.
    pub panel: Color,
    /// Menus, dialogs, and controls above the workspace.
    pub raised: Color,
    pub hover: Color,
    pub edge: Color,
    pub muted: Color,
    pub subtle: Color,
    pub ink: Color,
    pub accent: Color,
    pub accent_ink: Color,
    pub selection: Color,
    pub danger: Color,
    pub success: Color,
    pub overlay: Color,
}
impl Appearance {
    pub const ALL: [Self; 5] = [
        Self::Dark,
        Self::Graphite,
        Self::Midnight,
        Self::Eucalyptus,
        Self::Porcelain,
    ];
    pub fn colors(self) -> Colors {
        let values = match self {
            Self::Dark => [
                0x141414, 0x1b1b1b, 0x232323, 0x2d2d2d, 0x343434, 0xaaa8a3, 0x878580, 0xf4f2ec,
                0xf07858, 0x141414, 0x37332f, 0xf19b9b, 0x9bc59f,
            ],
            Self::Eucalyptus => [
                0x1b2220, 0x222b28, 0x2a3430, 0x303d37, 0x35423c, 0xa1b0a7, 0x71867a, 0xe6ece8,
                0x86bba8, 0x152c23, 0x304c40, 0xe38b8b, 0x8ac49c,
            ],
            Self::Midnight => [
                0x191e2a, 0x202633, 0x293141, 0x30394b, 0x343f52, 0xa0acc1, 0x707e98, 0xe5eaf3,
                0x8daafa, 0x17223d, 0x304465, 0xef8e99, 0x88bdab,
            ],
            Self::Graphite => [
                0x1b1d21, 0x222429, 0x2a2d33, 0x30343b, 0x353941, 0xa4aab6, 0x747d8d, 0xe6e9ef,
                0x87a9ed, 0x17243c, 0x2e4263, 0xec8a8e, 0x8bbda2,
            ],
            Self::Porcelain => [
                0xf4f2ec, 0xece9e1, 0xfaf9f6, 0xe3dfd5, 0xd4cfc4, 0x706d66, 0x79756e, 0x232323,
                0xb83e23, 0xffffff, 0xe4d9cd, 0xb13d4b, 0x28764f,
            ],
        };
        let [
            bg,
            panel,
            raised,
            hover,
            edge,
            muted,
            subtle,
            ink,
            accent,
            accent_ink,
            selection,
            danger,
            success,
        ] = values.map(rgb);
        Colors {
            bg,
            panel,
            raised,
            hover,
            edge,
            muted,
            subtle,
            ink,
            accent,
            accent_ink,
            selection,
            danger,
            success,
            overlay: if self == Self::Dark {
                Color::from_rgba8(0, 0, 0, 0.72)
            } else {
                Color::from_rgba8(10, 12, 17, 0.68)
            },
        }
    }
    pub fn syntax(self) -> iced::highlighter::Theme {
        if self == Self::Porcelain {
            iced::highlighter::Theme::InspiredGitHub
        } else {
            iced::highlighter::Theme::Base16Ocean
        }
    }
    pub fn terminal(self) -> iced_term::ColorPalette {
        let c = self.colors();
        let (normal, bright) = match self {
            Self::Porcelain => (
                [
                    0x293444, 0xb33847, 0x2c7352, 0x8a641f, 0x3169bd, 0x8b4bae, 0x247b87, 0x5e6876,
                ],
                [
                    0x667386, 0xbe3e4f, 0x217548, 0x8b661b, 0x285fc0, 0x914bb4, 0x147482, 0x3a4657,
                ],
            ),
            Self::Dark => (
                [
                    0x262626, 0xe99999, 0x9bc59f, 0xd9be8b, 0x99b4e7, 0xbba5d9, 0x93c3ca, 0xd1d1d1,
                ],
                [
                    0x7a7a7a, 0xf0b1b1, 0xb2d4b4, 0xe6ceaa, 0xb1c7ef, 0xcfbae7, 0xb0d6dc, 0xf5f5f5,
                ],
            ),
            _ => (
                [
                    0x272c35, 0xe38a8e, 0x91bd9e, 0xd6ba89, 0x87a9ed, 0xb69bdb, 0x83bdc8, 0xc8ced8,
                ],
                [
                    0x798292, 0xf2a0a4, 0xa4cfae, 0xe3c99e, 0xa0bcf6, 0xc7b0e8, 0x9bd0d8, 0xebeff5,
                ],
            ),
        };
        let [black, red, green, yellow, blue, magenta, cyan, white] = normal.map(rgb);
        let [
            bright_black,
            bright_red,
            bright_green,
            bright_yellow,
            bright_blue,
            bright_magenta,
            bright_cyan,
            bright_white,
        ] = bright.map(rgb);
        let dim = |color: Color| {
            hex(Color::from_rgb(
                color.r * 0.7 + c.bg.r * 0.3,
                color.g * 0.7 + c.bg.g * 0.3,
                color.b * 0.7 + c.bg.b * 0.3,
            ))
        };
        iced_term::ColorPalette {
            background: hex(c.bg),
            foreground: hex(c.ink),
            black: hex(black),
            red: hex(red),
            green: hex(green),
            yellow: hex(yellow),
            blue: hex(blue),
            magenta: hex(magenta),
            cyan: hex(cyan),
            white: hex(white),
            bright_black: hex(bright_black),
            bright_red: hex(bright_red),
            bright_green: hex(bright_green),
            bright_yellow: hex(bright_yellow),
            bright_blue: hex(bright_blue),
            bright_magenta: hex(bright_magenta),
            bright_cyan: hex(bright_cyan),
            bright_white: hex(bright_white),
            bright_foreground: Some(hex(c.ink)),
            dim_foreground: hex(c.muted),
            dim_black: dim(black),
            dim_red: dim(red),
            dim_green: dim(green),
            dim_yellow: dim(yellow),
            dim_blue: dim(blue),
            dim_magenta: dim(magenta),
            dim_cyan: dim(cyan),
            dim_white: dim(white),
        }
    }
}

fn rgb(value: u32) -> Color {
    Color::from_rgb8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn hex(color: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (color.r * 255.).round() as u8,
        (color.g * 255.).round() as u8,
        (color.b * 255.).round() as u8
    )
}

/// Recover the application tokens in Iced style callbacks.
pub fn theme_colors(theme: &iced::Theme) -> Colors {
    Appearance::ALL
        .into_iter()
        .find(|appearance| appearance.colors().bg == theme.palette().background)
        .unwrap_or_default()
        .colors()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_surfaces_and_terminal_use_the_same_tokens() {
        assert_eq!(Appearance::default(), Appearance::Dark);
        let colors = Appearance::Dark.colors();
        for surface in [
            colors.bg,
            colors.panel,
            colors.raised,
            colors.hover,
            colors.edge,
            colors.overlay,
        ] {
            assert_eq!(surface.r, surface.g);
            assert_eq!(surface.g, surface.b);
        }
        assert!(colors.bg.r < colors.panel.r);
        assert!(colors.panel.r < colors.raised.r);
        assert!(colors.raised.r < colors.hover.r);

        let terminal = Appearance::Dark.terminal();
        assert_eq!(terminal.background, hex(colors.bg));
        assert_eq!(terminal.foreground, hex(colors.ink));
        assert_eq!(terminal.bright_foreground, Some(hex(colors.ink)));
        assert_eq!(terminal.dim_foreground, hex(colors.muted));
        assert_ne!(terminal.red, terminal.green);
        assert_ne!(terminal.blue, terminal.cyan);
    }
}
