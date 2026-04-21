use std::collections::HashMap;
use std::path::Path;

/// A preprocessor macro
#[derive(Debug, Clone)]
struct Macro {
    params: Option<Vec<String>>, // None = object-like, Some = function-like
    body: String,
    is_variadic: bool,
    va_args_name: Option<String>, // named variadic: args...
}

pub struct Preprocessor {
    defines: HashMap<String, Macro>,
    include_paths: Vec<String>,
    if_stack: Vec<bool>,      // true = currently active section
    if_taken: Vec<bool>,      // true = some branch in this if/elif/else was taken
}

impl Preprocessor {
    pub fn new() -> Self {
        // Find include dir relative to executable
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let builtin_include = exe_dir.join("../include").to_string_lossy().to_string();
        // Also try relative to cwd for development
        let cwd_include = "include".to_string();

        let mut pp = Self {
            defines: HashMap::new(),
            include_paths: vec![".".to_string(), cwd_include, builtin_include],
            if_stack: Vec::new(),
            if_taken: Vec::new(),
        };
        pp.define_simple("__rcc__", "1");
        pp.define_simple("__x86_64__", "1");
        pp.define_simple("__LP64__", "1");
        pp.define_simple("__STDC__", "1");

        #[cfg(target_os = "windows")]
        {
            pp.define_simple("_WIN64", "1");
            pp.define_simple("_WIN32", "1");
            pp.define_simple("__WINDOWS__", "1");
        }
        #[cfg(target_os = "linux")]
        {
            pp.define_simple("__linux__", "1");
            pp.define_simple("__linux", "1");
            pp.define_simple("__unix__", "1");
            pp.define_simple("__ELF__", "1");
        }
        #[cfg(target_os = "macos")]
        {
            pp.define_simple("__APPLE__", "1");
            pp.define_simple("__MACH__", "1");
            pp.define_simple("__unix__", "1");
        }
        pp
    }

    pub fn add_include_path(&mut self, path: &str) {
        self.include_paths.push(path.to_string());
    }

    pub fn defines_insert(&mut self, name: &str, value: &str) {
        self.define_simple(name, value);
    }

    fn define_simple(&mut self, name: &str, value: &str) {
        self.defines.insert(name.to_string(), Macro {
            params: None,
            body: value.to_string(),
            is_variadic: false,
            va_args_name: None,
        });
    }

    fn is_active(&self) -> bool {
        self.if_stack.iter().all(|&b| b)
    }

    pub fn preprocess(&mut self, source: &str, filename: &str) -> Result<String, String> {
        let is_toplevel = self.if_stack.is_empty();
        self.preprocess_inner(source, filename, is_toplevel)
    }

    fn preprocess_inner(&mut self, source: &str, filename: &str, check_unterminated: bool) -> Result<String, String> {
        // First strip all comments (handles multi-line)
        let source = strip_all_comments(source);
        let mut output = String::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            // Join backslash-continued lines
            let mut line = lines[i].to_string();
            while line.trim_end().ends_with('\\') && i + 1 < lines.len() {
                line.truncate(line.trim_end().len() - 1); // remove backslash
                i += 1;
                line.push_str(lines[i].trim_start());
            }
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                self.handle_directive(trimmed, filename, i + 1, &mut output)?;
            } else if self.is_active() {
                // Replace __LINE__ and __FILE__ before macro expansion
                let line = line.replace("__LINE__", &format!("{}", i + 1))
                    .replace("__FILE__", &format!("\"{}\"", filename));
                let expanded = self.expand_macros(&line);
                output.push_str(&expanded);
                output.push('\n');
            } else {
                output.push('\n');
            }
            i += 1;
        }

        if check_unterminated && !self.if_stack.is_empty() {
            return Err("unterminated #if/#ifdef".to_string());
        }

        Ok(output)
    }

    fn handle_directive(
        &mut self,
        line: &str,
        filename: &str,
        line_no: usize,
        output: &mut String,
    ) -> Result<(), String> {
        let after_hash = line[1..].trim();

        // Extract directive name
        let (directive, rest) = match after_hash.find(|c: char| c.is_whitespace()) {
            Some(pos) => (&after_hash[..pos], after_hash[pos..].trim()),
            None => (after_hash, ""),
        };

        match directive {
            "define" => {
                if self.is_active() {
                    self.handle_define(rest)?;
                }
            }
            "undef" => {
                if self.is_active() {
                    self.defines.remove(rest.trim());
                }
            }
            "include" => {
                if self.is_active() {
                    // Expand macros in #include argument
                    let expanded_rest = self.expand_macros(rest);
                    let content = self.handle_include(&expanded_rest, filename)?;
                    output.push_str(&content);
                }
            }
            "ifdef" => {
                let name = rest.trim();
                let parent_active = self.is_active();
                let active = parent_active && self.defines.contains_key(name);
                self.if_stack.push(active);
                self.if_taken.push(active);
            }
            "ifndef" => {
                let name = rest.trim();
                let parent_active = self.is_active();
                let active = parent_active && !self.defines.contains_key(name);
                self.if_stack.push(active);
                self.if_taken.push(active);
            }
            "if" => {
                let parent_active = self.is_active();
                let active = if parent_active {
                    self.eval_pp_expr(rest) != 0
                } else {
                    false
                };
                self.if_stack.push(active);
                self.if_taken.push(active);
            }
            "elif" => {
                let n = self.if_stack.len();
                if n > 0 {
                    let already_taken = self.if_taken[n - 1];
                    if already_taken {
                        self.if_stack[n - 1] = false;
                    } else {
                        // Check parent is active
                        let parent_active = if n >= 2 {
                            self.if_stack[..n-1].iter().all(|&b| b)
                        } else {
                            true
                        };
                        if parent_active {
                            let val = self.eval_pp_expr(rest);
                            let active = val != 0;
                            self.if_stack[n - 1] = active;
                            if active { self.if_taken[n - 1] = true; }
                        } else {
                            self.if_stack[n - 1] = false;
                        }
                    }
                }
            }
            "else" => {
                let n = self.if_stack.len();
                if n > 0 {
                    let already_taken = self.if_taken[n - 1];
                    let parent_active = if n >= 2 {
                        self.if_stack[..n-1].iter().all(|&b| b)
                    } else {
                        true
                    };
                    self.if_stack[n - 1] = parent_active && !already_taken;
                }
            }
            "endif" => {
                self.if_stack.pop();
                self.if_taken.pop();
                if self.if_stack.len() != self.if_taken.len() {
                    return Err(format!("{}:{}: #endif mismatch", filename, line_no));
                }
            }
            "pragma" => {
                // Ignore pragmas
            }
            "error" => {
                if self.is_active() {
                    return Err(format!("{}:{}: #error {}", filename, line_no, rest));
                }
            }
            "warning" => {
                if self.is_active() {
                    eprintln!("{}:{}: warning: #warning {}", filename, line_no, rest);
                }
            }
            "line" | "" => {
                // Ignore
            }
            _ => {
                // Linemarker: # 200 "filename" — starts with digit
                if directive.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                    // Ignore linemarker
                } else if self.is_active() {
                    return Err(format!("{}:{}: unknown directive '#{}'", filename, line_no, directive));
                }
            }
        }

        output.push('\n'); // keep line numbers
        Ok(())
    }

    fn handle_define(&mut self, rest: &str) -> Result<(), String> {
        let rest = rest.trim();
        if rest.is_empty() {
            return Err("#define: missing macro name".to_string());
        }

        // Find macro name
        let name_end = rest.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(rest.len());
        let name = &rest[..name_end];
        let after_name = &rest[name_end..];

        // Check for function-like macro: name immediately followed by (
        if after_name.starts_with('(') {
            let close = after_name.find(')').ok_or("unterminated macro parameter list")?;
            let params_str = &after_name[1..close];
            let body = after_name[close + 1..].trim().to_string();

            let mut params = Vec::new();
            let mut is_variadic = false;
            let mut va_args_name = None;
            for p in params_str.split(',') {
                let p = p.trim();
                if p == "..." {
                    is_variadic = true;
                } else if p.ends_with("...") {
                    // Named variadic: args...
                    is_variadic = true;
                    va_args_name = Some(p.trim_end_matches("...").trim().to_string());
                } else if !p.is_empty() {
                    params.push(p.to_string());
                }
            }

            self.defines.insert(name.to_string(), Macro {
                params: Some(params),
                body,
                is_variadic,
                va_args_name,
            });
        } else {
            // Object-like macro
            let body = after_name.trim().to_string();
            self.defines.insert(name.to_string(), Macro {
                params: None,
                body,
                is_variadic: false,
                va_args_name: None,
            });
        }
        Ok(())
    }

    fn handle_include(&mut self, rest: &str, current_file: &str) -> Result<String, String> {
        let rest = rest.trim();
        let (path, _is_system) = if rest.starts_with('"') {
            let end = rest[1..].find('"').ok_or("#include: unterminated filename")?;
            (&rest[1..1 + end], false)
        } else if rest.starts_with('<') {
            let end = rest[1..].find('>').ok_or("#include: unterminated filename")?;
            (&rest[1..1 + end], true)
        } else {
            return Err(format!("#include: expected filename, got '{}'", rest));
        };

        // Search for file
        let current_dir = Path::new(current_file)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());

        let search_dirs = std::iter::once(current_dir.as_str())
            .chain(self.include_paths.iter().map(|s| s.as_str()));

        for dir in search_dirs {
            let full_path = if dir.is_empty() {
                path.to_string()
            } else {
                format!("{}/{}", dir, path)
            };

            if let Ok(content) = std::fs::read_to_string(&full_path) {
                return self.preprocess_inner(&content, &full_path, false);
            }
        }

        // If file not found, silently skip (for system headers we don't have)
        Ok(format!("/* #include {} not found */\n", rest))
    }

    fn expand_macros(&self, line: &str) -> String {
        let mut result = line.to_string();

        for _ in 0..10 {
            let mut changed = false;

            // Object-like macros
            for (name, mac) in &self.defines {
                if mac.params.is_some() { continue; }
                if result.contains(name.as_str()) {
                    let new = replace_whole_word(&result, name, &mac.body);
                    if new != result { result = new; changed = true; }
                }
            }

            // Function-like macros: NAME(args)
            for (name, mac) in &self.defines {
                let params = match &mac.params {
                    Some(p) => p,
                    None => continue,
                };
                let pattern = format!("{}(", name);
                if let Some(start) = find_outside_strings(&result, &pattern) {
                    // Check whole word boundary
                    if start > 0 && is_ident_char(result.as_bytes()[start - 1]) { continue; }
                    let args_start = start + pattern.len();
                    if let Some((args, end)) = extract_macro_args(&result[args_start..]) {
                        let end = args_start + end;
                        let mut body = mac.body.clone();
                        // Step 1: Handle # stringify FIRST (uses raw args, must check not ##)
                        for (i, param) in params.iter().enumerate() {
                            if i < args.len() {
                                // Match #param but NOT ##param
                                let hash_param = format!("#{}", param);
                                let double_hash = format!("##{}", param);
                                if body.contains(&hash_param) {
                                    // Only replace if not preceded by another #
                                    let new_body = body.replace(&double_hash, "\x01PASTE\x01"); // temp placeholder
                                    let stringified = format!("\"{}\"", args[i].replace('\\', "\\\\").replace('"', "\\\""));
                                    let new_body = new_body.replace(&hash_param, &stringified);
                                    body = new_body.replace("\x01PASTE\x01", &double_hash); // restore
                                }
                            }
                        }
                        // Step 2: Replace params in body
                        for (i, param) in params.iter().enumerate() {
                            if i < args.len() {
                                body = replace_whole_word(&body, param, &args[i]);
                            }
                        }
                        // Step 3: Handle ## token paste (remove ## and join)
                        while body.contains("##") {
                            body = body.replace(" ## ", "").replace("## ", "").replace(" ##", "").replace("##", "");
                        }
                        // Handle __VA_ARGS__ and named variadic (args...)
                        if mac.is_variadic {
                            let va_args = if args.len() > params.len() {
                                args[params.len()..].join(", ")
                            } else {
                                String::new()
                            };
                            body = body.replace("__VA_ARGS__", &va_args);
                            // Handle __VA_OPT__(content) → content if va_args non-empty, else nothing
                            while let Some(opt_start) = body.find("__VA_OPT__(") {
                                let after = &body[opt_start + 11..];
                                if let Some(close) = after.find(')') {
                                    let content = &after[..close];
                                    let replacement = if va_args.is_empty() { "" } else { content };
                                    body = format!("{}{}{}", &body[..opt_start], replacement, &body[opt_start + 11 + close + 1..]);
                                } else { break; }
                            }
                            // Named variadic: replace the named param with variadic args
                            if let Some(ref va_name) = mac.va_args_name {
                                body = replace_whole_word(&body, va_name, &va_args);
                            }
                        }
                        result = format!("{}{}{}", &result[..start], body, &result[end..]);
                        changed = true;
                    }
                }
            }

            if !changed { break; }
        }
        result
    }

    fn eval_pp_expr(&self, expr: &str) -> i64 {
        let expr = expr.trim();
        let expr = self.expand_defined(expr);
        let expr = self.expand_macros(&expr);
        let expr = expr.trim();
        self.eval_pp_or(expr)
    }

    fn eval_pp_or(&self, expr: &str) -> i64 {
        // Split on || (outside parens)
        if let Some((left, right)) = split_outside_parens(expr, "||") {
            let l = self.eval_pp_or(left.trim());
            let r = self.eval_pp_or(right.trim());
            return if l != 0 || r != 0 { 1 } else { 0 };
        }
        self.eval_pp_and(expr)
    }

    fn eval_pp_and(&self, expr: &str) -> i64 {
        if let Some((left, right)) = split_outside_parens(expr, "&&") {
            let l = self.eval_pp_and(left.trim());
            let r = self.eval_pp_and(right.trim());
            return if l != 0 && r != 0 { 1 } else { 0 };
        }
        self.eval_pp_atom(expr)
    }

    fn eval_pp_atom(&self, expr: &str) -> i64 {
        let expr = expr.trim();
        if expr.is_empty() { return 0; }

        // Handle !expr
        if let Some(rest) = expr.strip_prefix('!') {
            return if self.eval_pp_atom(rest.trim()) == 0 { 1 } else { 0 };
        }

        // Handle (expr)
        if expr.starts_with('(') && expr.ends_with(')') {
            return self.eval_pp_or(&expr[1..expr.len()-1]);
        }

        // Handle comparisons: ==, !=, >, <, >=, <=
        for (op, f) in &[
            ("==", (|a: i64, b: i64| if a == b { 1 } else { 0 }) as fn(i64, i64) -> i64),
            ("!=", (|a, b| if a != b { 1 } else { 0 }) as fn(i64, i64) -> i64),
            (">=", (|a, b| if a >= b { 1 } else { 0 }) as fn(i64, i64) -> i64),
            ("<=", (|a, b| if a <= b { 1 } else { 0 }) as fn(i64, i64) -> i64),
            (">",  (|a, b| if a > b { 1 } else { 0 }) as fn(i64, i64) -> i64),
            ("<",  (|a, b| if a < b { 1 } else { 0 }) as fn(i64, i64) -> i64),
        ] {
            if let Some((left, right)) = split_outside_parens(expr, op) {
                let l = self.eval_pp_atom(left.trim());
                let r = self.eval_pp_atom(right.trim());
                return f(l, r);
            }
        }

        // Handle + and -
        if let Some((left, right)) = split_outside_parens(expr, "+") {
            if !left.is_empty() {
                return self.eval_pp_atom(left.trim()) + self.eval_pp_atom(right.trim());
            }
        }

        // Number
        if let Ok(n) = expr.parse::<i64>() { return n; }
        // Hex
        if let Some(hex) = expr.strip_prefix("0x").or_else(|| expr.strip_prefix("0X")) {
            if let Ok(n) = i64::from_str_radix(hex.trim_end_matches(|c: char| c.is_ascii_alphabetic()), 16) {
                return n;
            }
        }

        // Identifier evaluates to 0 (undefined macro)
        0
    }

    fn expand_defined(&self, expr: &str) -> String {
        let mut result = expr.to_string();

        // defined(NAME) form
        while let Some(pos) = result.find("defined(") {
            let start = pos;
            let after = &result[pos + 8..];
            if let Some(close) = after.find(')') {
                let name = after[..close].trim();
                let val = if self.defines.contains_key(name) { "1" } else { "0" };
                result = format!("{}{}{}", &result[..start], val, &result[pos + 8 + close + 1..]);
            } else {
                break;
            }
        }

        // defined NAME form (without parens)
        while let Some(pos) = result.find("defined ") {
            let after = &result[pos + 8..];
            let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(after.len());
            let name = &after[..name_end];
            let val = if self.defines.contains_key(name) { "1" } else { "0" };
            result = format!("{}{}{}", &result[..pos], val, &result[pos + 8 + name_end..]);
        }

        result
    }
}

/// Strip all C comments from source (handles multi-line /* ... */)
fn strip_all_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    let mut in_char = false;

    while i < bytes.len() {
        if in_string {
            result.push(bytes[i] as char);
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                result.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if bytes[i] == b'"' { in_string = false; }
            i += 1;
            continue;
        }
        if in_char {
            result.push(bytes[i] as char);
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                result.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if bytes[i] == b'\'' { in_char = false; }
            i += 1;
            continue;
        }
        if bytes[i] == b'"' { in_string = true; result.push('"'); i += 1; continue; }
        if bytes[i] == b'\'' { in_char = true; result.push('\''); i += 1; continue; }

        // Line comment
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            // Skip until newline
            while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
            continue;
        }
        // Block comment
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if bytes[i] == b'\n' { result.push('\n'); } // preserve line numbers
                i += 1;
            }
            if i + 1 < bytes.len() { i += 2; }
            result.push(' ');
            continue;
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

/// Strip C comments from a single line (handles /* ... */ and // ...)
fn strip_comments(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_string = false;

    while i < bytes.len() {
        if in_string {
            result.push(bytes[i] as char);
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                result.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if bytes[i] == b'"' { in_string = false; }
            i += 1;
            continue;
        }
        if bytes[i] == b'"' {
            in_string = true;
            result.push('"');
            i += 1;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            break; // line comment: skip rest
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < bytes.len() { i += 2; } // skip */
            result.push(' '); // replace comment with space
            continue;
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

/// Split on operator outside parentheses
fn split_outside_parens<'a>(s: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let op_bytes = op.as_bytes();
    let op_len = op_bytes.len();
    for i in 0..bytes.len() {
        if bytes[i] == b'(' { depth += 1; }
        else if bytes[i] == b')' { depth -= 1; }
        else if depth == 0 && i + op_len <= bytes.len() && &bytes[i..i+op_len] == op_bytes {
            // Make sure it's not part of a longer operator
            return Some((&s[..i], &s[i+op_len..]));
        }
    }
    None
}

/// Extract comma-separated args from "(a, b, c)..." returning (args, end_pos including ')')
fn extract_macro_args(s: &str) -> Option<(Vec<String>, usize)> {
    let mut depth = 0i32;
    let mut args = Vec::new();
    let mut current = String::new();

    for (i, ch) in s.char_indices() {
        match ch {
            '(' => { depth += 1; current.push(ch); }
            ')' if depth > 0 => { depth -= 1; current.push(ch); }
            ')' => {
                args.push(current.trim().to_string());
                return Some((args, i + 1));
            }
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    None
}

fn replace_whole_word(text: &str, word: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let word_bytes = word.as_bytes();
    let word_len = word_bytes.len();
    let mut i = 0;
    let mut in_string = false;
    let mut in_char = false;

    while i < bytes.len() {
        // Skip string literals — don't expand macros inside "..."
        if !in_char && bytes[i] == b'"' && (i == 0 || bytes[i-1] != b'\\') {
            in_string = !in_string;
            result.push('"');
            i += 1;
            continue;
        }
        if !in_string && bytes[i] == b'\'' && (i == 0 || bytes[i-1] != b'\\') {
            in_char = !in_char;
            result.push('\'');
            i += 1;
            continue;
        }
        if in_string || in_char {
            result.push(bytes[i] as char);
            i += 1;
            continue;
        }

        if i + word_len <= bytes.len() && &bytes[i..i + word_len] == word_bytes {
            let before_ok = i == 0 || !is_ident_char(bytes[i - 1]);
            let after_ok = i + word_len >= bytes.len() || !is_ident_char(bytes[i + word_len]);
            if before_ok && after_ok {
                result.push_str(replacement);
                i += word_len;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Find pattern in text, skipping string literals
fn find_outside_strings(text: &str, pattern: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let pat = pattern.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    let mut in_char = false;

    while i < bytes.len() {
        if !in_char && bytes[i] == b'"' && (i == 0 || bytes[i-1] != b'\\') {
            in_string = !in_string;
            i += 1;
            continue;
        }
        if !in_string && bytes[i] == b'\'' && (i == 0 || bytes[i-1] != b'\\') {
            in_char = !in_char;
            i += 1;
            continue;
        }
        if in_string || in_char {
            i += 1;
            continue;
        }
        if i + pat.len() <= bytes.len() && &bytes[i..i+pat.len()] == pat {
            return Some(i);
        }
        i += 1;
    }
    None
}
