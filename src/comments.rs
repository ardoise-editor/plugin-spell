//! Where the comments and strings of a program are: only their words are
//! prose to check.

/// A part of the code that is not code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Comment,
    Str,
}

/// How a language writes comments and strings.
struct Syntax {
    line: &'static [&'static str],
    block: &'static [(&'static str, &'static str)],
    /// Delimiters of strings, longest first (`"""` before `"`).
    strings: &'static [&'static str],
    /// Strings that go on past the end of a line.
    multiline: &'static [&'static str],
    /// `\` escapes the next character in strings.
    escapes: bool,
    /// Rust: raw strings `r#"…"#`, and `'` is a lifetime unless it closes
    /// a character.
    rust: bool,
    /// HTML, XML: strings only inside tags.
    markup: bool,
}

const C_LIKE: Syntax = Syntax {
    line: &["//"],
    block: &[("/*", "*/")],
    strings: &["\"", "'"],
    multiline: &[],
    escapes: true,
    rust: false,
    markup: false,
};

fn syntax_for(ext: &str) -> Option<Syntax> {
    Some(match ext {
        "rs" => Syntax {
            strings: &["\""],
            multiline: &["\""],
            rust: true,
            ..C_LIKE
        },
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts" | "tsx" | "vue" | "svelte" => Syntax {
            strings: &["`", "\"", "'"],
            multiline: &["`"],
            ..C_LIKE
        },
        "go" => Syntax {
            strings: &["`", "\"", "'"],
            multiline: &["`"],
            ..C_LIKE
        },
        "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "cs" | "java" | "kt" | "kts"
        | "swift" | "dart" | "scala" | "php" | "groovy" | "gradle" | "zig" => C_LIKE,
        "css" | "scss" | "less" => Syntax {
            line: if ext == "css" { &[] } else { &["//"] },
            ..C_LIKE
        },
        "json" | "jsonc" | "json5" => Syntax {
            strings: &["\""],
            ..C_LIKE
        },
        "py" | "pyi" | "pyw" => Syntax {
            line: &["#"],
            block: &[],
            strings: &["\"\"\"", "'''", "\"", "'"],
            multiline: &["\"\"\"", "'''"],
            ..C_LIKE
        },
        "sh" | "bash" | "zsh" | "fish" | "rb" | "pl" | "pm" | "r" | "yaml" | "yml" | "toml"
        | "mk" | "dockerfile" | "conf" | "nix" | "ex" | "exs" | "jl" | "tf" => Syntax {
            line: &["#"],
            block: &[],
            strings: &["\"", "'"],
            multiline: &[],
            ..C_LIKE
        },
        "ini" | "cfg" | "properties" => Syntax {
            line: &["#", ";"],
            block: &[],
            strings: &[],
            ..C_LIKE
        },
        "sql" => Syntax {
            line: &["--"],
            strings: &["'", "\""],
            ..C_LIKE
        },
        "lua" => Syntax {
            line: &["--"],
            block: &[("--[[", "]]")],
            strings: &["\"", "'"],
            ..C_LIKE
        },
        "hs" => Syntax {
            line: &["--"],
            block: &[("{-", "-}")],
            strings: &["\""],
            ..C_LIKE
        },
        "html" | "htm" | "xml" | "xhtml" | "svg" | "plist" | "xsd" | "xsl" => Syntax {
            line: &[],
            block: &[("<!--", "-->")],
            strings: &["\"", "'"],
            multiline: &["\"", "'"],
            escapes: false,
            rust: false,
            markup: true,
        },
        _ => return None,
    })
}

/// The comments and string literals of `text`, a file of extension `ext`,
/// as byte ranges in order. A language the editor doesn't know has none.
pub fn regions(ext: &str, text: &str) -> Vec<(usize, usize, Region)> {
    let Some(syntax) = syntax_for(&ext.to_ascii_lowercase()) else {
        return Vec::new();
    };
    let bytes = text.as_bytes();
    let at = |i: usize, s: &str| bytes[i..].starts_with(s.as_bytes());
    let mut found = Vec::new();
    let mut in_tag = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if syntax.markup {
            if c == b'<' && !at(i, "<!--") {
                in_tag = true;
            } else if c == b'>' {
                in_tag = false;
            }
        }
        if let Some(start) = syntax.line.iter().find(|s| at(i, s)) {
            let _ = start;
            let end = text[i..].find('\n').map_or(bytes.len(), |n| i + n);
            found.push((i, end, Region::Comment));
            i = end;
            continue;
        }
        if let Some((open, close)) = syntax.block.iter().find(|(o, _)| at(i, o)) {
            let from = i + open.len();
            let end = text[from..]
                .find(close)
                .map_or(bytes.len(), |n| from + n + close.len());
            found.push((i, end, Region::Comment));
            i = end;
            continue;
        }
        // Rust: raw strings, and a `'` that is a character, not a lifetime.
        if syntax.rust {
            if c == b'r' && (i == 0 || !is_ident(bytes[i - 1])) {
                let hashes = bytes[i + 1..].iter().take_while(|b| **b == b'#').count();
                if bytes.get(i + 1 + hashes) == Some(&b'"') {
                    let close = format!("\"{}", "#".repeat(hashes));
                    let from = i + 2 + hashes;
                    let end = text[from..]
                        .find(&close)
                        .map_or(bytes.len(), |n| from + n + close.len());
                    found.push((i, end, Region::Str));
                    i = end;
                    continue;
                }
            }
            if c == b'\'' {
                let rest = &text[i + 1..];
                let mut chars = rest.char_indices();
                let close = match chars.next() {
                    Some((_, '\\')) => rest[1..].find('\'').map(|n| n + 2),
                    Some((_, ch)) => {
                        let after = ch.len_utf8();
                        (rest[after..].starts_with('\'')).then_some(after + 1)
                    }
                    None => None,
                };
                if let Some(len) = close.filter(|l| *l <= 12) {
                    found.push((i, i + 1 + len, Region::Str));
                    i += 1 + len;
                } else {
                    i += 1;
                }
                continue;
            }
        }
        if !syntax.markup || in_tag {
            if let Some(delim) = syntax.strings.iter().find(|s| at(i, s)) {
                let multiline = syntax.multiline.contains(delim);
                let mut j = i + delim.len();
                let end = loop {
                    if j >= bytes.len() {
                        break bytes.len();
                    }
                    if syntax.escapes && bytes[j] == b'\\' {
                        j += 2;
                        continue;
                    }
                    if at(j, delim) {
                        break j + delim.len();
                    }
                    if bytes[j] == b'\n' && !multiline {
                        break j;
                    }
                    j += 1;
                };
                found.push((i, end.min(bytes.len()), Region::Str));
                i = end.min(bytes.len()).max(i + 1);
                continue;
            }
        }
        i += 1;
    }
    found
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
