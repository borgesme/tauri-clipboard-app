use sha2::{Digest, Sha256};

const PREVIEW_LIMIT: usize = 100;

pub fn normalize_text(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn content_hash(content: &str) -> String {
    let normalized = normalize_text(content);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn preview(content: &str) -> String {
    let mut chars = content.chars();
    let preview: String = chars.by_ref().take(PREVIEW_LIMIT).collect();
    if chars.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

#[cfg(test)]
mod tests {
    use super::{content_hash, normalize_text, preview};

    #[test]
    fn normalizes_line_endings_without_trimming() {
        assert_eq!(" hello \nworld ", normalize_text(" hello \r\nworld "));
    }

    #[test]
    fn hashes_equivalent_line_endings_equally() {
        assert_eq!(content_hash("a\r\nb"), content_hash("a\nb"));
    }

    #[test]
    fn truncates_long_preview() {
        let source = "a".repeat(101);
        assert_eq!(101, preview(&source).chars().count());
    }
}
