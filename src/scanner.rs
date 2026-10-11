use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use crate::LanguageStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassifierState {
    Plain,
    String,
    Char,
    LineComment,
    BlockComment
}

pub fn classify_lines(path: &Path) -> std::io::Result<LanguageStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut stats = LanguageStats {
        files: 1,
        ..Default::default()
    };

    let mut state = ClassifierState::Plain;
    let mut escaped = false;

    for line in reader.lines() {
        let line = line?;
        let bytes = line.as_bytes();

        let mut has_code = false;
        let mut has_comment = false;
        let mut i = 0;

        while i < bytes.len() {
            let current_char = bytes[i];
            // For look ahead to recognize 2 Char delimiters
            let next_char = bytes.get(i + 1).copied().unwrap_or(b'\0');

            match state {
                ClassifierState::Plain => {
                    if current_char == b'/' && next_char == b'/' {
                        state = ClassifierState::LineComment;
                        has_comment = true;
                        i += 2;
                        continue;
                    }

                    if current_char == b'/' && next_char == b'*' {
                        state = ClassifierState::BlockComment;
                        has_comment = true;
                        i += 2;
                        continue;
                    }

                    if current_char == b'\'' {
                        state = ClassifierState::Char;
                        has_code = true;
                    }

                    if current_char == b'"' {
                        state = ClassifierState::String;
                        has_code = true;
                    }

                    if !current_char.is_ascii_whitespace() {
                        has_code = true;
                    }
                }
                ClassifierState::String => {
                    if escaped {
                        escaped = false;
                    } else if current_char == b'\\' {
                        escaped = true;
                    } else if current_char == b'"' {
                        state = ClassifierState::Plain;
                    }
                }
                ClassifierState::Char => {
                    if escaped {
                        escaped = false;
                    } else if current_char == b'\\' {
                        escaped = true;
                    } else if current_char == b'\'' {
                        state = ClassifierState::Plain;
                    }
                }
                ClassifierState::LineComment => {
                    // Everything after this belongs to the comment
                    break;
                }
                ClassifierState::BlockComment => {
                    has_comment = true;
                    if current_char == b'*' && next_char == b'/' {
                        state = ClassifierState::Plain;
                        i += 2;
                        continue;
                    }
                }        
            }
            
            i += 1;
        }

        if has_code {
            stats.code += 1;
        } else if has_comment {
            stats.comments += 1;
        } else {
            stats.blanks += 1;
        }
        stats.lines += 1;

        if state == ClassifierState::LineComment {
            state = ClassifierState::Plain;
        }
        if state == ClassifierState::String || state == ClassifierState::Char {
            escaped = false;
        }
    }

    Ok(stats)
}