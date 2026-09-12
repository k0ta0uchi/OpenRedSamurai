//! Minimal order-preserving INI document, byte-compatible with `RSProfile*.pfd`
//! files written by the original MFC software.
//!
//! Design rules:
//! - Preserve section order and key order exactly as found in the file.
//! - Write with CRLF line endings, `[Section]` + `Key=Value` lines.
//! - Keys are looked up case-insensitively, but the original spelling is kept.
//! - Unknown keys / sections are never dropped (lossless round-trip).

use std::fmt::Write as _;

/// Ordered key/value section.
#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub items: Vec<(String, String)>,
}

impl Section {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    pub fn get_parse<T: std::str::FromStr>(&self, key: &str) -> Option<T> {
        self.get(key).and_then(|v| v.trim().parse().ok())
    }

    /// Update an existing key in place (keeping position and spelling),
    /// or append it at the end of the section.
    pub fn set(&mut self, key: &str, value: &str) {
        if let Some(slot) = self
            .items
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
        {
            slot.1 = value.to_string();
        } else {
            self.items.push((key.to_string(), value.to_string()));
        }
    }
}

/// Whole INI document.
#[derive(Debug, Clone, Default)]
pub struct IniDoc {
    pub sections: Vec<Section>,
}

impl IniDoc {
    pub fn parse(text: &str) -> Self {
        let mut doc = IniDoc::default();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix('[') {
                let name = rest.trim_end_matches(']').trim().to_string();
                doc.sections.push(Section {
                    name,
                    items: Vec::new(),
                });
            } else if let Some((k, v)) = line.split_once('=') {
                let key = k.trim().to_string();
                let value = v.to_string();
                match doc.sections.last_mut() {
                    Some(sec) => sec.items.push((key, value)),
                    None => doc.sections.push(Section {
                        name: String::new(),
                        items: vec![(key, value)],
                    }),
                }
            }
        }
        doc
    }

    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn section_mut(&mut self, name: &str) -> Option<&mut Section> {
        self.sections
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Get a section, creating it at the end when missing.
    pub fn section_or_create(&mut self, name: &str) -> &mut Section {
        if let Some(pos) = self
            .sections
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name))
        {
            &mut self.sections[pos]
        } else {
            self.sections.push(Section {
                name: name.to_string(),
                items: Vec::new(),
            });
            self.sections.last_mut().expect("just pushed")
        }
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.section(section).and_then(|s| s.get(key))
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        self.section_or_create(section).set(key, value);
    }

    /// Serialize exactly like the original files (CRLF, no blank lines).
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        for sec in &self.sections {
            out.push('[');
            out.push_str(&sec.name);
            out.push_str("]\r\n");
            for (k, v) in &sec.items {
                let _ = write!(out, "{k}={v}\r\n");
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_serialize_roundtrip() {
        let src = "[GROUP0]\r\nMouseSensitivity=15\r\nLedColor1=255\r\n[ButtonAssigned0_1]\r\nButtonID=255\r\n";
        let doc = IniDoc::parse(src);
        assert_eq!(doc.serialize(), src);
    }

    #[test]
    fn set_keeps_position_and_spelling() {
        let src = "[GROUP0]\r\nPollingRate=1\r\nLedColor1=255\r\n";
        let mut doc = IniDoc::parse(src);
        doc.set("group0", "ledcolor1", "65280");
        assert_eq!(
            doc.serialize(),
            "[GROUP0]\r\nPollingRate=1\r\nLedColor1=65280\r\n"
        );
    }
}
