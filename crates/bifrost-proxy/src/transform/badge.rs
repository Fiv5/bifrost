use bytes::Bytes;

pub const BIFROST_BADGE_ELEMENT_ID: &str = "__bifrost_badge__";

const BADGE_SNIPPET: &str = concat!(
    "<style>",
    "#__bifrost_badge__{",
    "position:fixed;left:15px;bottom:15px;z-index:2147483647;",
    "display:flex;align-items:center;",
    "height:30px;width:30px;border-radius:9999px;",
    "background:linear-gradient(135deg,#7BEBC0,#6CBFCF);",
    "box-shadow:0 0 10px 2px rgba(123,235,192,0.35),0 0 0 2px rgba(255,255,255,0.9);",
    "cursor:pointer;overflow:hidden;white-space:nowrap;",
    "font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;",
    "user-select:none;-webkit-user-select:none;",
    "transition:width .4s cubic-bezier(.4,0,.2,1),border-radius .4s cubic-bezier(.4,0,.2,1),box-shadow .3s;",
    "}",
    "#__bifrost_badge__:hover{",
    "width:220px;border-radius:15px;",
    "box-shadow:0 0 18px 4px rgba(123,235,192,0.45),0 0 0 2px rgba(255,255,255,0.95);",
    "}",
    "#__bifrost_badge__ .__bb_ico{",
    "min-width:30px;width:30px;height:30px;",
    "display:flex;align-items:center;justify-content:center;",
    "font-size:14px;font-weight:800;color:#fff;line-height:1;",
    "}",
    "#__bifrost_badge__ .__bb_txt{",
    "font-size:12px;font-weight:600;color:#fff;",
    "opacity:0;padding-right:14px;",
    "transition:opacity .25s .1s;",
    "}",
    "#__bifrost_badge__:hover .__bb_txt{opacity:1}",
    "</style>",
    r#"<div id="__bifrost_badge__" onclick="this.style.display='none'" aria-hidden="true">"#,
    r#"<span class="__bb_ico">B</span>"#,
    r#"<span class="__bb_txt">Bifrost proxy is working</span>"#,
    "</div>",
);

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
        assert!(out_str.starts_with("<html>Hello</html>"));
        assert!(out_str.contains(BIFROST_BADGE_ELEMENT_ID));
    }

    #[test]
    fn test_badge_contains_b_character_and_click_hide() {
        let snippet = BADGE_SNIPPET;
        assert!(snippet.contains("__bb_ico"));
        assert!(snippet.contains(">B</span>"));
        assert!(snippet.contains("Bifrost proxy is working"));
        assert!(snippet.contains("onclick="));
        assert!(snippet.contains("display="));
        assert!(snippet.contains("none"));
        assert!(snippet.contains("cursor:pointer"));
        assert!(snippet.contains(":hover"));
        assert!(snippet.contains("left:15px"));
        assert!(snippet.contains("bottom:15px"));
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
