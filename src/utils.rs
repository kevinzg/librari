pub fn slugify(input: &str) -> String {
    input
        .chars()
        .filter(|&c| c.is_ascii_alphanumeric() || c == ' ')
        .map(|c| {
            if c == ' ' {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

pub fn extract_id(input: &str) -> u64 {
    input
        .chars()
        .take_while(|&c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap()
}
