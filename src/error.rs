/// Source location for error reporting
#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub offset: usize,
    pub len: usize,
}

impl Span {
    pub fn new(offset: usize, len: usize) -> Self {
        Self { offset, len }
    }
}

/// Compiler error with source location and optional hint
#[derive(Debug)]
pub struct CompileError {
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
}

impl CompileError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Print a beautiful error message with source context and ^ pointer
    pub fn print(&self, filename: &str, source: &str) {
        let (line_no, col, line_text) = self.locate(source);

        eprintln!("\x1b[1;31merror\x1b[0m: {}", self.message);

        eprintln!(
            "  \x1b[1;34m-->\x1b[0m {}:{}:{}",
            filename, line_no, col + 1
        );

        let gutter = format!("{}", line_no).len();

        eprintln!("{:>width$} \x1b[1;34m|\x1b[0m", "", width = gutter);

        eprintln!(
            "{:>width$} \x1b[1;34m|\x1b[0m {}",
            line_no, line_text, width = gutter
        );

        let pointer_len = self.span.len.max(1);
        eprintln!(
            "{:>width$} \x1b[1;34m|\x1b[0m {}\x1b[1;31m{}\x1b[0m",
            "", " ".repeat(col), "^".repeat(pointer_len), width = gutter
        );

        if let Some(hint) = &self.hint {
            eprintln!(
                "{:>width$} \x1b[1;34m|\x1b[0m",
                "", width = gutter
            );
            eprintln!(
                "{:>width$} \x1b[1;32m= help\x1b[0m: {}",
                "", hint, width = gutter
            );
        }
    }

    fn locate<'a>(&self, source: &'a str) -> (usize, usize, &'a str) {
        let mut line_no = 1;
        let mut line_start = 0;

        for (i, ch) in source.char_indices() {
            if i >= self.span.offset {
                break;
            }
            if ch == '\n' {
                line_no += 1;
                line_start = i + 1;
            }
        }

        let col = self.span.offset.saturating_sub(line_start);

        let line_end = source[self.span.offset..]
            .find('\n')
            .map(|pos| self.span.offset + pos)
            .unwrap_or(source.len());

        let line_text = &source[line_start..line_end];
        (line_no, col, line_text)
    }
}

/// Levenshtein distance between two strings
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}

/// Find the best match for `name` from a list of candidates
pub fn did_you_mean<'a>(name: &str, candidates: &[&'a str]) -> Option<&'a str> {
    // Very strict: only suggest at distance 1 (single typo)
    let max_dist = 1;
    candidates
        .iter()
        .map(|&c| (c, levenshtein(name, c)))
        .filter(|&(_, d)| d > 0 && d <= max_dist)
        // Also require similar length
        .filter(|&(c, _)| {
            let len_diff = (name.len() as isize - c.len() as isize).unsigned_abs();
            len_diff <= 2
        })
        .min_by_key(|&(_, d)| d)
        .map(|(c, _)| c)
}
