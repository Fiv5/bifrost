use super::url_decode;

pub struct ParsedReplaceRules {
    pub string_rules: Vec<(String, String)>,
    pub regex_rules: Vec<bifrost_proxy::RegexReplace>,
}

pub fn parse_regex_pattern(s: &str) -> Option<(regex::Regex, bool)> {
    let s = s.trim();
    if !s.starts_with('/') {
        return None;
    }

    let global = s.ends_with("/g") || s.ends_with("/gi") || s.ends_with("/ig");
    let case_insensitive = s.ends_with("/i") || s.ends_with("/gi") || s.ends_with("/ig");

    let end_pos = if global && case_insensitive {
        s.len() - 3
    } else if global || case_insensitive {
        s.len() - 2
    } else if s.len() > 1 && s.ends_with('/') {
        s.len() - 1
    } else {
        return None;
    };

    let pattern_str = &s[1..end_pos];
    if pattern_str.is_empty() {
        return None;
    }

    let regex_result = if case_insensitive {
        regex::RegexBuilder::new(pattern_str)
            .case_insensitive(true)
            .build()
    } else {
        regex::Regex::new(pattern_str)
    };

    match regex_result {
        Ok(re) => Some((re, global)),
        Err(e) => {
            tracing::warn!("Invalid regex pattern '{}': {}", pattern_str, e);
            None
        }
    }
}

pub fn parse_replace_value(value: &str) -> ParsedReplaceRules {
    let mut string_rules = Vec::new();
    let mut regex_rules = Vec::new();

    for pair in value.split('&') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }

        if let Some((from, to)) = pair.split_once('=') {
            let from = url_decode(from);
            let to = url_decode(to);

            if let Some((regex, global)) = parse_regex_pattern(&from) {
                regex_rules.push(bifrost_proxy::RegexReplace {
                    pattern: regex,
                    replacement: to,
                    global,
                });
            } else {
                string_rules.push((from, to));
            }
        } else {
            let from = url_decode(pair);
            if let Some((regex, global)) = parse_regex_pattern(&from) {
                regex_rules.push(bifrost_proxy::RegexReplace {
                    pattern: regex,
                    replacement: String::new(),
                    global,
                });
            } else {
                string_rules.push((from, String::new()));
            }
        }
    }

    ParsedReplaceRules {
        string_rules,
        regex_rules,
    }
}
