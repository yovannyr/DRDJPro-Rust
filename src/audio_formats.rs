//! Supported extensions; actual decoding still validates the container and codec.
use std::path::Path;

pub const EXTENSIONS: &[&str] = &["mp3", "wav", "m4a", "flac", "aif", "aiff", "ogg", "oga"];

pub fn supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            EXTENSIONS
                .iter()
                .any(|item| extension.eq_ignore_ascii_case(item))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_policy_is_case_insensitive_and_does_not_accept_other_containers() {
        for extension in EXTENSIONS {
            assert!(supported(Path::new(&format!("song.{extension}"))));
            assert!(supported(Path::new(&format!(
                "song.{}",
                extension.to_uppercase()
            ))));
        }
        for name in ["song.opus", "song.m4p", "song.wma", "song.txt", "song"] {
            assert!(!supported(Path::new(name)));
        }
    }
}
