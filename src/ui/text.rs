/// Truncate `s` to at most `max` characters (not bytes), appending `…` when cut.
///
/// Byte slicing (`&s[..n]`) panics when `n` lands inside a multi-byte
/// character, so this counts Unicode scalar values instead.
pub fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max == 0 {
        String::new()
    } else {
        let kept: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{kept}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_strings_are_returned_unchanged() {
        assert_eq!(truncate_str("hi", 10), "hi");
        assert_eq!(truncate_str("exactly-ten!", 12), "exactly-ten!");
    }

    #[test]
    fn ascii_is_cut_with_ellipsis() {
        assert_eq!(
            truncate_str("abcdefghijklmnopqrstuvwxyz0123456789", 30)
                .chars()
                .count(),
            30
        );
    }

    #[test]
    fn emoji_is_never_split_mid_character() {
        // 40 rocket emojis: byte slicing at 29 would panic; char slicing must not.
        let desc = "🚀".repeat(40);
        let out = truncate_str(&desc, 30);
        assert_eq!(out.chars().count(), 30);
        assert!(out.ends_with('…'));
        // Every char is either 🚀 or … — no replacement characters.
        assert!(out.chars().all(|c| c == '🚀' || c == '…'));
    }

    #[test]
    fn mixed_text_with_emoji() {
        let s = "Fix 🦀 Rust parser for café naïve résumé handling";
        let out = truncate_str(s, 20);
        assert_eq!(out.chars().count(), 20);
        assert!(out.ends_with('…'));
    }
}
