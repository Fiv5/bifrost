use bytes::Bytes;

pub const BIFROST_BADGE_ELEMENT_ID: &str = "__bifrost_badge__";

const BADGE_SNIPPET: &str = r#"<div id="__bifrost_badge__" style="position:fixed;left:12px;bottom:12px;width:10px;height:10px;border-radius:9999px;background:#ff4d4f;box-shadow:0 0 0 2px rgba(255,255,255,0.9),0 2px 8px rgba(0,0,0,0.15);z-index:2147483647;opacity:0.9;pointer-events:none" aria-hidden="true"></div>"#;

fn contains_badge(body: &[u8]) -> bool {
    let marker = BIFROST_BADGE_ELEMENT_ID.as_bytes();
    body.windows(marker.len()).any(|w| w == marker)
}

fn find_last_body_close_tag_start(body: &[u8]) -> Option<usize> {
    const PATTERN: &[u8] = b"</body>";
    if body.len() < PATTERN.len() {
        return None;
    }

    for start in (0..=body.len() - PATTERN.len()).rev() {
        if body[start..start + PATTERN.len()]
            .iter()
            .zip(PATTERN.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
        {
            return Some(start);
        }
    }
    None
}

pub fn maybe_inject_bifrost_badge_html(body: Bytes) -> (Bytes, bool) {
    if body.is_empty() || contains_badge(&body) {
        return (body, false);
    }

    let snippet = BADGE_SNIPPET.as_bytes();

    if let Some(insert_at) = find_last_body_close_tag_start(&body) {
        let mut out = Vec::with_capacity(body.len() + snippet.len());
        out.extend_from_slice(&body[..insert_at]);
        out.extend_from_slice(snippet);
        out.extend_from_slice(&body[insert_at..]);
        (Bytes::from(out), true)
    } else {
        let mut out = Vec::with_capacity(body.len() + snippet.len());
        out.extend_from_slice(&body);
        out.extend_from_slice(snippet);
        (Bytes::from(out), true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_badge_before_body_end() {
        let html = Bytes::from_static(b"<html><body>Hello</body></html>");
        let (out, changed) = maybe_inject_bifrost_badge_html(html);
        assert!(changed);

        let out_str = String::from_utf8(out.to_vec()).unwrap();
        let badge_pos = out_str.find(BIFROST_BADGE_ELEMENT_ID).unwrap();
        let body_close_pos = out_str.to_ascii_lowercase().rfind("</body>").unwrap();
        assert!(badge_pos < body_close_pos);
    }

    #[test]
    fn test_inject_badge_append_when_no_body_end() {
        let html = Bytes::from_static(b"<html>Hello</html>");
        let (out, changed) = maybe_inject_bifrost_badge_html(html);
        assert!(changed);

        let out_str = String::from_utf8(out.to_vec()).unwrap();
        assert!(out_str.ends_with(&format!("</html>{}", BADGE_SNIPPET)));
    }

    #[test]
    fn test_inject_badge_case_insensitive_body_end() {
        let html = Bytes::from_static(b"<html><body>Hello</BODY></html>");
        let (out, changed) = maybe_inject_bifrost_badge_html(html);
        assert!(changed);

        let out_str = String::from_utf8(out.to_vec()).unwrap();
        let badge_pos = out_str.find(BIFROST_BADGE_ELEMENT_ID).unwrap();
        let body_close_pos = out_str.to_ascii_lowercase().rfind("</body>").unwrap();
        assert!(badge_pos < body_close_pos);
    }
}
