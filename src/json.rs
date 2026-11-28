//! Tiny helpers for writing JSON by hand (the core has no dependencies).

/// A float with at most three decimals; non-finite values become 0.
pub fn num(x: f32) -> String {
    if !x.is_finite() {
        return "0".to_string();
    }
    let s = format!("{:.3}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-" || s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

/// A quoted JSON string.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Joins already-encoded items into an array.
pub fn array(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}
