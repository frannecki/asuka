pub(crate) fn summarize_text(input: &str, max_words: usize) -> String {
    let words = input.split_whitespace().take(max_words).collect::<Vec<_>>();
    if words.is_empty() {
        String::new()
    } else if input.split_whitespace().count() > max_words {
        format!("{}...", words.join(" "))
    } else {
        words.join(" ")
    }
}
