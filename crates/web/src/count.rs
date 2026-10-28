//! Shared English count labels.
pub(crate) fn count_label(count: usize, singular: &str) -> String {
    format!("{count} {singular}{}", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::count_label;

    #[test]
    fn singular_only_for_one() {
        for noun in ["ticket", "project", "comment"] {
            assert_eq!(count_label(0, noun), format!("0 {noun}s"));
            assert_eq!(count_label(1, noun), format!("1 {noun}"));
            assert_eq!(count_label(2, noun), format!("2 {noun}s"));
        }
    }
}
