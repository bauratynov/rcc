# NLnet Grant Proposal: rcc — Educational C Compiler in Rust

## Proposal Name
rcc — A Complete C11 Compiler with SSA IR, WASM Backend, and LSP Server

## Abstract

rcc is a small (8.7K LOC), zero-dependency C11 compiler written in Rust that demonstrates how modern compiler techniques — SSA-based intermediate representation, multi-target code generation, and language server integration — can be implemented in a fraction of the code used by production compilers. It compiles 100% of chibicc test files and supports 3 backends (x86-64, WebAssembly Text, WASM binary), 11 optimizations across 3 levels, a built-in LSP server for IDE integration, and a linter.

The project's goal is to become the standard educational compiler: small enough to read in a weekend, complete enough to compile real C code, and modern enough to teach SSA, register allocation, and WASM compilation.

## What problem does this solve?

Existing educational compilers fall into two categories:
1. **Too simple** (tutorials, toy parsers) — teach syntax but not compiler engineering
2. **Too complex** (LLVM, GCC) — millions of lines, impossible to learn from

rcc fills the gap: a production-quality compiler architecture in 8.7K lines that students, researchers, and developers can study, fork, and extend. It is the only educational compiler with SSA IR + WASM backend + LSP server.

## Requested Amount
€30,000

## Budget Breakdown

| Task | Hours | Rate | Cost |
|------|-------|------|------|
| Complete SSA with phi nodes and register allocation | 120h | €50/h | €6,000 |
| WASM backend: full structured control flow (Stackifier) | 80h | €50/h | €4,000 |
| C11 coverage: remaining edge cases to 98%+ | 100h | €50/h | €5,000 |
| Second real-world project (compile SQLite or Lua) | 80h | €50/h | €4,000 |
| Educational documentation: book-style code walkthrough | 60h | €50/h | €3,000 |
| LSP server: find-references, rename, code actions | 40h | €50/h | €2,000 |
| Cross-platform testing and CI (Linux, macOS, Windows) | 40h | €50/h | €2,000 |
| Community building: tutorials, blog posts, conference talk | 40h | €50/h | €2,000 |
| Project management and coordination | 40h | €50/h | €2,000 |
| **Total** | **600h** | | **€30,000** |

## Technical Challenges

1. **Full SSA construction** — implementing Braun's algorithm with proper phi node insertion and elimination for loop variables across complex control flow graphs
2. **WASM structured control flow** — converting arbitrary CFG to WASM's block/loop/br_table using the Stackifier algorithm
3. **Register allocation** — enabling the linear scan allocator with correct callee-saved register management
4. **C11 completeness** — handling the long tail of C edge cases (bitfield semantics, complex declarators, variadic function codegen)

## Comparison with Existing Efforts

| Project | LOC | IR | Backends | LSP | Optimizations |
|---------|-----|----|----------|-----|---------------|
| chibicc | 9K | No | 1 | No | 0 |
| 8cc | 7K | No | 1 | No | 0 |
| tcc | 40K | No | 2 | No | 0 |
| lcc | 15K | Yes | 3 | No | 0 |
| **rcc** | **8.7K** | **SSA** | **3** | **Yes** | **11** |

## Ecosystem and Engagement

- **Target audience**: CS students, compiler course instructors, Rust developers, WASM enthusiasts
- **Distribution**: GitHub (MIT/Apache-2.0), crates.io, blog posts, conference talks
- **Community**: GitHub Issues/Discussions, planned Discord/Matrix channel
- **Sustainability**: GitHub Sponsors for ongoing maintenance after grant period

## Prior Experience

The maintainer is a systems programmer with experience in:
- Video surveillance systems (66K LOC Go, 44K LOC standalone modules)
- CPU-optimized AI inference engines (pure C, AVX2/SSE2)
- H.265/H.264 bitstream transcoders (C/AVX2)
- 31+ standalone C99 libraries

## Links

- Repository: https://github.com/bauratynov/rcc
- License: MIT OR Apache-2.0
