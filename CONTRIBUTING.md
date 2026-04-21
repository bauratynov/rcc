# Contributing to rcc

Thanks for your interest in contributing! rcc is a small, approachable codebase — perfect for first-time compiler contributors.

## Quick Start

```bash
git clone https://github.com/user/rcc.git
cd rcc
cargo build
cargo test
```

## Project Structure

```
src/
  main.rs          Entry point, CLI argument parsing
  lexer.rs         Tokenizer (numbers, strings, keywords, operators)
  parser.rs        Recursive descent parser → AST
  ast.rs           AST node definitions and type system
  preprocess.rs    C preprocessor (#define, #include, #ifdef)
  optimize.rs      AST-level optimizations (constant folding, dead code)
  codegen.rs       AST → x86-64 assembly (Windows ABI)
  ir.rs            Intermediate Representation definitions
  lower.rs         AST → IR lowering
  backend_x64.rs   IR → x86-64 assembly
  backend_wasm.rs  IR → WebAssembly Text (WAT)
  lint.rs          Lint warnings (unused vars, unreachable code)
  lsp.rs           Language Server Protocol (JSON-RPC)
  driver.rs        gcc invocation for assembling/linking
  error.rs         Error messages with colors and "did you mean?"
include/
  stdio.h, stdlib.h, ...   Minimal C standard library headers
test_inputs/
  *.c              End-to-end test programs
test_real/
  cJSON/           Real-world cJSON library test
```

## How to Add a C Feature

1. Write a test in `test_inputs/myfeature.c`
2. Try to compile: `cargo run -- test_inputs/myfeature.c`
3. See the error — it tells you exactly where to fix
4. Fix the parser (`parser.rs`) or codegen (`codegen.rs`)
5. Run all tests: `cargo test`
6. End-to-end: `cargo run -- test_inputs/myfeature.c -o test.exe && ./test.exe`

## Good First Issues

- Add `unsigned int` support in codegen (use `movzbl` instead of `movsbl`)
- Add `static` local variables
- Fix `cJSON_Parse` runtime correctness
- Add `--format` flag to auto-format C code
- Add more lint rules (e.g., implicit int conversion warnings)

## Code Style

- No external dependencies
- Keep functions short (<50 lines)
- Match existing patterns — look at how similar features are implemented
- Add comments only where the logic isn't obvious

## Running Tests

```bash
# Unit tests
cargo test

# Single file end-to-end
cargo run -- test_inputs/hello_world.c -o hello.exe && ./hello.exe

# Benchmark
cargo run --release -- --bench test_inputs/simple.c

# Lint
cargo run -- --lint test_inputs/lint_test.c

# WASM output
cargo run -- --wasm test_inputs/simple.c
```
