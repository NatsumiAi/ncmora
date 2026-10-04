use serde::{Deserialize, Serialize};
use std::io::IsTerminal;
use std::sync::LazyLock;
use std::time::Duration;

/// Terminal fonts cannot be queried reliably. Auto chooses plain symbols on
/// Linux consoles and non-interactive output; explicit modes override it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IconMode {
    #[default]
    Auto,
    Ascii,
    Nerd,
}

static AUTO_ASCII: LazyLock<bool> = LazyLock::new(|| {
    let term = std::env::var("TERM").unwrap_or_default();
    !std::io::stdout().is_terminal() || console_term(&term)
});

fn console_term(term: &str) -> bool {
    matches!(
        term,
        "" | "linux" | "kmscon" | "dumb" | "vt100" | "vt102" | "cons25"
    )
}
#[derive(Debug, Clone, Copy)]
pub struct UiIcons {
    ascii: bool,
}

impl UiIcons {
    pub fn for_mode(mode: IconMode) -> Self {
        Self::with_console(mode, *AUTO_ASCII)
    }

    pub const fn with_console(mode: IconMode, console: bool) -> Self {
        Self {
            ascii: match mode {
                IconMode::Auto => console,
                IconMode::Ascii => true,
                IconMode::Nerd => false,
            },
        }
    }

    pub const fn previous(self) -> &'static str {
        if self.ascii { "[<<]" } else { "[\u{f048}]" }
    }

    pub const fn next(self) -> &'static str {
        if self.ascii { "[>>]" } else { "[\u{f051}]" }
    }

    pub const fn play_pause(self, playing: bool) -> &'static str {
        match (self.ascii, playing) {
            (true, true) => "[||]",
            (true, false) => "[>]",
            (false, true) => "[\u{f04c}]",
            (false, false) => "[\u{f04b}]",
        }
    }

    pub const fn heart(self, liked: bool) -> &'static str {
        match (self.ascii, liked) {
            (true, true) => "[*]",
            (true, false) => "[ ]",
            (false, true) => "\u{f004}",
            (false, false) => "\u{f08a}",
        }
    }

    pub const fn sequence(self) -> &'static str {
        if self.ascii { "SEQ" } else { "\u{f08f}" }
    }

    pub const fn shuffle(self) -> &'static str {
        if self.ascii { "SHF" } else { "\u{f074}" }
    }

    pub const fn loop_all(self) -> &'static str {
        if self.ascii { "ALL" } else { "\u{f0b6}" }
    }

    pub const fn loop_one(self) -> &'static str {
        if self.ascii { "ONE" } else { "\u{f01e}" }
    }

    pub const fn download(self) -> char {
        if self.ascii { 'v' } else { '\u{ec74}' }
    }

    pub const fn downloaded(self) -> char {
        if self.ascii { '+' } else { '\u{f00c}' }
    }

    pub fn downloading(self, phase: Duration) -> char {
        let tick = phase.as_millis() / 100;
        if self.ascii {
            const FRAMES: [char; 4] = ['|', '/', '-', '\\'];
            FRAMES[(tick % FRAMES.len() as u128) as usize]
        } else {
            const FRAMES: [char; 10] = [
                '\u{280b}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283c}', '\u{2834}', '\u{2826}',
                '\u{2827}', '\u{2807}', '\u{280f}',
            ];
            FRAMES[(tick % FRAMES.len() as u128) as usize]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IconMode, UiIcons, console_term};
    use std::time::Duration;

    #[test]
    fn console_auto_uses_readable_ascii_controls() {
        let icons = UiIcons::with_console(IconMode::Auto, true);
        assert_eq!(icons.previous(), "[<<]");
        assert_eq!(icons.play_pause(true), "[||]");
        assert_eq!(icons.play_pause(false), "[>]");
        assert_eq!(icons.next(), "[>>]");
        assert_eq!(icons.heart(true), "[*]");
        assert_eq!(icons.heart(false), "[ ]");
        assert_eq!(icons.shuffle(), "SHF");
        assert_eq!(icons.download(), 'v');
        assert_eq!(icons.downloaded(), '+');
        assert_eq!(icons.downloading(Duration::from_millis(100)), '/');
    }

    #[test]
    fn explicit_font_choice_overrides_console_detection() {
        let nerd = UiIcons::with_console(IconMode::Nerd, true);
        assert_eq!(nerd.heart(true), "\u{f004}");
        assert_eq!(nerd.download(), '\u{ec74}');
        let ascii = UiIcons::with_console(IconMode::Ascii, false);
        assert_eq!(ascii.loop_one(), "ONE");
        assert_eq!(ascii.downloading(Duration::from_millis(400)), '|');
        assert!(console_term("linux"));
        assert!(console_term("dumb"));
        assert!(!console_term("xterm-256color"));
    }
}
