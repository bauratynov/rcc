/// LSP (Language Server Protocol) for rcc
///
/// Implements a minimal LSP server over stdin/stdout:
/// - textDocument/didOpen, didChange, didClose
/// - textDocument/publishDiagnostics (errors + lint warnings)
/// - textDocument/completion (keywords + local variables)
/// - textDocument/hover (type information)
///
/// Zero dependencies — hand-rolled JSON-RPC.

use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};

use crate::{lexer, parser, lint, optimize, preprocess, error::Span};

pub fn run_lsp() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    let mut documents: HashMap<String, String> = HashMap::new();

    eprintln!("rcc LSP server started");

    loop {
        // Read LSP header
        let mut header = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return; // EOF
            }
            if line.trim().is_empty() { break; }
            header.push_str(&line);
        }

        // Parse Content-Length
        let content_length = header.lines()
            .find(|l| l.starts_with("Content-Length:"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(0);

        if content_length == 0 { continue; }

        // Read body
        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body).unwrap();
        let body = String::from_utf8_lossy(&body).to_string();

        // Parse JSON (minimal hand-rolled)
        let method = json_get_str(&body, "method").unwrap_or_default();
        let id = json_get_number(&body, "id");

        match method.as_str() {
            "initialize" => {
                let result = r#"{"capabilities":{"textDocumentSync":1,"completionProvider":{"triggerCharacters":["."]},"hoverProvider":true,"definitionProvider":true}}"#;
                send_response(&mut writer, id.unwrap_or(0), result);
            }
            "initialized" => {
                // No response needed
            }
            "shutdown" => {
                send_response(&mut writer, id.unwrap_or(0), "null");
            }
            "exit" => {
                return;
            }
            "textDocument/didOpen" => {
                if let (Some(uri), Some(text)) = (
                    json_get_nested_str(&body, "textDocument", "uri"),
                    json_get_nested_str(&body, "textDocument", "text"),
                ) {
                    documents.insert(uri.clone(), text.clone());
                    let diagnostics = compute_diagnostics(&uri, &text);
                    send_notification(&mut writer, "textDocument/publishDiagnostics",
                        &format!(r#"{{"uri":"{}","diagnostics":[{}]}}"#, uri, diagnostics));
                }
            }
            "textDocument/didChange" => {
                if let Some(uri) = json_get_nested_str(&body, "textDocument", "uri") {
                    // Full sync: get the text from contentChanges[0].text
                    if let Some(text) = json_get_content_change(&body) {
                        documents.insert(uri.clone(), text.clone());
                        let diagnostics = compute_diagnostics(&uri, &text);
                        send_notification(&mut writer, "textDocument/publishDiagnostics",
                            &format!(r#"{{"uri":"{}","diagnostics":[{}]}}"#, uri, diagnostics));
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = json_get_nested_str(&body, "textDocument", "uri") {
                    documents.remove(&uri);
                }
            }
            "textDocument/completion" => {
                let items = get_completions(&body, &documents);
                send_response(&mut writer, id.unwrap_or(0), &items);
            }
            "textDocument/hover" => {
                let hover = get_hover(&body, &documents);
                send_response(&mut writer, id.unwrap_or(0), &hover);
            }
            "textDocument/definition" => {
                let def = get_definition(&body, &documents);
                send_response(&mut writer, id.unwrap_or(0), &def);
            }
            _ => {
                if let Some(id) = id {
                    send_response(&mut writer, id, "null");
                }
            }
        }
    }
}

fn compute_diagnostics(uri: &str, source: &str) -> String {
    let filename = uri_to_path(uri);
    let mut diags = Vec::new();

    // Preprocess
    let mut pp = preprocess::Preprocessor::new();
    let source = match pp.preprocess(source, &filename) {
        Ok(s) => s,
        Err(e) => {
            diags.push(format!(
                r#"{{"range":{{"start":{{"line":0,"character":0}},"end":{{"line":0,"character":1}}}},"severity":1,"message":"{}"}}"#,
                escape_json(&e)
            ));
            return diags.join(",");
        }
    };

    // Tokenize
    let tokens = match lexer::tokenize(&filename, &source) {
        Ok(t) => t,
        Err(e) => {
            let (line, col) = offset_to_lc(&source, e.span.offset);
            diags.push(format!(
                r#"{{"range":{{"start":{{"line":{},"character":{}}},"end":{{"line":{},"character":{}}}}},"severity":1,"message":"{}"}}"#,
                line, col, line, col + e.span.len.max(1), escape_json(&e.message)
            ));
            return diags.join(",");
        }
    };

    // Parse
    let mut parser = parser::Parser::new(tokens);
    let mut program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => {
            let (line, col) = offset_to_lc(&source, e.span.offset);
            diags.push(format!(
                r#"{{"range":{{"start":{{"line":{},"character":{}}},"end":{{"line":{},"character":{}}}}},"severity":1,"message":"{}"}}"#,
                line, col, line, col + e.span.len.max(1), escape_json(&e.message)
            ));
            return diags.join(",");
        }
    };

    optimize::optimize(&mut program);

    // Lint warnings
    let warnings = lint::lint(&program);
    for w in &warnings {
        let (line, col) = offset_to_lc(&source, w.span.offset);
        diags.push(format!(
            r#"{{"range":{{"start":{{"line":{},"character":{}}},"end":{{"line":{},"character":{}}}}},"severity":2,"message":"[{}] {}"}}"#,
            line, col, line, col + w.span.len.max(1), w.code, escape_json(&w.message)
        ));
    }

    diags.join(",")
}

fn get_completions(_body: &str, _documents: &HashMap<String, String>) -> String {
    // Return C keywords + common types
    let keywords = [
        "auto", "break", "case", "char", "const", "continue", "default", "do",
        "double", "else", "enum", "extern", "float", "for", "goto", "if",
        "int", "long", "register", "return", "short", "signed", "sizeof",
        "static", "struct", "switch", "typedef", "union", "unsigned", "void",
        "volatile", "while", "printf", "scanf", "malloc", "free", "strlen",
        "strcmp", "strcpy", "memcpy", "memset", "NULL", "sizeof",
    ];

    let items: Vec<String> = keywords.iter().map(|kw| {
        format!(r#"{{"label":"{}","kind":14}}"#, kw)
    }).collect();

    format!("[{}]", items.join(","))
}

fn get_hover(body: &str, documents: &HashMap<String, String>) -> String {
    // Get position
    let uri = json_get_nested_str(body, "textDocument", "uri").unwrap_or_default();
    let line = json_get_nested_number(body, "position", "line").unwrap_or(0);
    let col = json_get_nested_number(body, "position", "character").unwrap_or(0);

    if let Some(source) = documents.get(&uri) {
        // Find word at position
        let offset = lc_to_offset(source, line as usize, col as usize);
        if let Some(word) = word_at(source, offset) {
            // Check if it's a keyword
            let info = match word {
                "int" => "Type: `int` (32-bit signed integer, 4 bytes)",
                "char" => "Type: `char` (8-bit character, 1 byte)",
                "long" => "Type: `long` (64-bit signed integer, 8 bytes)",
                "short" => "Type: `short` (16-bit signed integer, 2 bytes)",
                "float" => "Type: `float` (32-bit floating point, 4 bytes)",
                "double" => "Type: `double` (64-bit floating point, 8 bytes)",
                "void" => "Type: `void` (no value)",
                "sizeof" => "Operator: `sizeof(type)` — returns size in bytes",
                "return" => "Statement: returns a value from the current function",
                "if" => "Statement: `if (condition) { ... } else { ... }`",
                "for" => "Statement: `for (init; cond; inc) { ... }`",
                "while" => "Statement: `while (condition) { ... }`",
                "printf" => "Function: `int printf(const char *fmt, ...)` — formatted output",
                "malloc" => "Function: `void *malloc(size_t size)` — allocate memory",
                "free" => "Function: `void free(void *ptr)` — deallocate memory",
                "strlen" => "Function: `size_t strlen(const char *s)` — string length",
                "NULL" => "Macro: null pointer constant (0)",
                _ => "",
            };
            if !info.is_empty() {
                return format!(r#"{{"contents":{{"kind":"markdown","value":"{}"}}}}"#, info);
            }
        }
    }
    "null".to_string()
}

fn get_definition(body: &str, documents: &HashMap<String, String>) -> String {
    let uri = json_get_nested_str(body, "textDocument", "uri").unwrap_or_default();
    let line = json_get_nested_number(body, "position", "line").unwrap_or(0) as usize;
    let col = json_get_nested_number(body, "position", "character").unwrap_or(0) as usize;

    if let Some(source) = documents.get(&uri) {
        let offset = lc_to_offset(source, line, col);
        if let Some(word) = word_at(source, offset) {
            // Search for declaration of this identifier: "type word" pattern
            let patterns = [
                format!("int {}", word), format!("long {}", word),
                format!("char {}", word), format!("double {}", word),
                format!("void {}", word), format!("struct {}", word),
            ];
            for pat in &patterns {
                if let Some(pos) = source.find(pat.as_str()) {
                    let (def_line, def_col) = offset_to_lc(source, pos);
                    return format!(
                        r#"{{"uri":"{}","range":{{"start":{{"line":{},"character":{}}},"end":{{"line":{},"character":{}}}}}}}"#,
                        uri, def_line, def_col, def_line, def_col + pat.len()
                    );
                }
            }
        }
    }
    "null".to_string()
}

// ── JSON helpers (no serde, hand-rolled) ──

fn json_get_str(json: &str, key: &str) -> Option<String> {
    let pattern = format!(r#""{}":"#, key);
    let pos = json.find(&pattern)? + pattern.len();
    if json.as_bytes().get(pos)? == &b'"' {
        let start = pos + 1;
        let end = json[start..].find('"')? + start;
        Some(json[start..end].to_string())
    } else {
        None
    }
}

fn json_get_number(json: &str, key: &str) -> Option<i64> {
    let pattern = format!(r#""{}":"#, key);
    let pos = json.find(&pattern)? + pattern.len();
    let rest = &json[pos..];
    let end = rest.find(|c: char| !c.is_ascii_digit() && c != '-').unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn json_get_nested_str(json: &str, obj: &str, key: &str) -> Option<String> {
    let obj_pattern = format!(r#""{}":"#, obj);
    let obj_pos = json.find(&obj_pattern)?;
    let sub = &json[obj_pos..];
    json_get_str(sub, key)
}

fn json_get_nested_number(json: &str, obj: &str, key: &str) -> Option<i64> {
    let obj_pattern = format!(r#""{}":"#, obj);
    let obj_pos = json.find(&obj_pattern)?;
    let sub = &json[obj_pos..];
    json_get_number(sub, key)
}

fn json_get_content_change(json: &str) -> Option<String> {
    // Find "contentChanges":[{"text":"..."}]
    let pos = json.find(r#""text":"#)?;
    let rest = &json[pos + 7..];
    if rest.starts_with('"') {
        let mut escaped = false;
        let mut end = 0;
        for (i, ch) in rest[1..].char_indices() {
            if escaped { escaped = false; continue; }
            if ch == '\\' { escaped = true; continue; }
            if ch == '"' { end = i + 1; break; }
        }
        Some(unescape_json(&rest[1..end]))
    } else {
        None
    }
}

fn send_response(writer: &mut impl Write, id: i64, result: &str) {
    let body = format!(r#"{{"jsonrpc":"2.0","id":{},"result":{}}}"#, id, result);
    let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    writer.write_all(msg.as_bytes()).unwrap();
    writer.flush().unwrap();
}

fn send_notification(writer: &mut impl Write, method: &str, params: &str) {
    let body = format!(r#"{{"jsonrpc":"2.0","method":"{}","params":{}}}"#, method, params);
    let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    writer.write_all(msg.as_bytes()).unwrap();
    writer.flush().unwrap();
}

fn offset_to_lc(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in source.char_indices() {
        if i >= offset { break; }
        if ch == '\n' { line += 1; col = 0; } else { col += 1; }
    }
    (line, col)
}

fn lc_to_offset(source: &str, line: usize, col: usize) -> usize {
    let mut cur_line = 0;
    let mut cur_col = 0;
    for (i, ch) in source.char_indices() {
        if cur_line == line && cur_col == col { return i; }
        if ch == '\n' { cur_line += 1; cur_col = 0; } else { cur_col += 1; }
    }
    source.len()
}

fn word_at(source: &str, offset: usize) -> Option<&str> {
    let bytes = source.as_bytes();
    if offset >= bytes.len() { return None; }
    if !bytes[offset].is_ascii_alphanumeric() && bytes[offset] != b'_' { return None; }
    let mut start = offset;
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    Some(&source[start..end])
}

fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file:///").unwrap_or(uri).replace("%20", " ").to_string()
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

fn unescape_json(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t").replace("\\\"", "\"").replace("\\\\", "\\")
}
