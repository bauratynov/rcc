# Sovereign Tech Fund Application: rcc

## Project Name
rcc — C11 Compiler in Rust

## Description
rcc is a 8,748-line, zero-dependency C11 compiler written in Rust. It features SSA-based intermediate representation, 3 code generation backends (x86-64, WebAssembly Text, WASM binary), 11 compiler optimizations, a Language Server Protocol (LSP) server for IDE integration, and a built-in linter. It compiles 100% of the chibicc compiler test suite.

## Prevalence
C remains the most widely used systems programming language. Every operating system, embedded device, and performance-critical application depends on C compilers. An educational, hackable C compiler benefits:
- University compiler courses worldwide
- Developers building language tools
- The WebAssembly ecosystem (C-to-WASM compilation)
- The Rust ecosystem (demonstrating Rust for systems tool development)

## Relevance
Compiler infrastructure is critical digital infrastructure. Understanding how compilers work is essential for:
- Security auditing of compiled code
- Developing new programming languages
- Optimizing software performance
- Advancing WebAssembly adoption

## Vulnerability
Educational compiler projects are chronically underfunded. chibicc (10K GitHub stars) has had no active development since 2022. There is no maintained educational C compiler with modern features (SSA, WASM, LSP). rcc fills this gap but needs sustained funding to reach production quality.

## Requested Amount
€75,000

## Work Plan (12 months)

### Phase 1: Core Compiler Quality (months 1-4, €25,000)
- Full SSA with phi nodes and register allocation
- Complete C11 coverage to 98%
- Compile real-world projects: SQLite, Lua, cJSON fully
- Comprehensive test suite: 500+ assertions

### Phase 2: WASM and Tooling (months 5-8, €25,000)
- WASM backend: full Stackifier algorithm for structured control flow
- LSP server: find-references, rename, diagnostics
- Online playground: compile C in browser via WASM
- VS Code extension packaging

### Phase 3: Documentation and Community (months 9-12, €25,000)
- Book-style documentation: "Building a C Compiler in Rust"
- Video tutorial series
- Conference presentations
- Community building: contributor onboarding, mentorship

## License
MIT OR Apache-2.0 (OSI-approved)

## Links
- https://github.com/bauratynov/rcc
