/// Lossless stream/JSON-pointer identity used while reading stored rule fields.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct MetricSelector {
    pub stream_key: String,
    pub field_path: String,
}

impl MetricSelector {
    pub fn parse(field: &str) -> Option<Self> {
        let (stream, path) = field.split_once("./")?;
        if !(1..=64).contains(&stream.len())
            || !stream.bytes().next()?.is_ascii_lowercase()
            || !stream.bytes().all(|value| {
                value.is_ascii_lowercase()
                    || value.is_ascii_digit()
                    || matches!(value, b'-' | b'_' | b'.')
            })
        {
            return None;
        }
        let mut chars = path.chars();
        while let Some(character) = chars.next() {
            if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
                return None;
            }
        }
        Some(Self {
            stream_key: stream.into(),
            field_path: format!("/{path}"),
        })
    }

    pub fn field_key(&self) -> String {
        format!("{}.{}", self.stream_key, self.field_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_nested_literal_dotted_and_escaped_paths() {
        for key in [
            "env./a/b",
            "env./a.b",
            "env.v2./a~1b",
            "env./a~0b",
            "env.//a",
        ] {
            assert_eq!(MetricSelector::parse(key).unwrap().field_key(), key);
        }
        assert_ne!(
            MetricSelector::parse("env./a/b"),
            MetricSelector::parse("env./a.b")
        );
        for key in ["temperature", "env.a.b", "env./a~2", "env./a~", "./a"] {
            assert!(MetricSelector::parse(key).is_none());
        }
    }
}
