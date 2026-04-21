# Changelog

All notable changes to rcc are documented here.

## [1.0.0] — 2026-04-21

### Highlights
- **41/41 chibicc test files compile** (100% parse compatibility)
- **Cross-platform**: Windows, Linux, macOS via `#[cfg]` auto-detection
- **Full SSA IR** with cross-block promotion and loop variable support
- **3 backends**: x86-64 native, WebAssembly Text, WASM binary
- **LSP server** with diagnostics, completion, hover, go-to-definition
- **11 optimizations** across AST, IR, and assembly levels

### Added
- Cross-platform support: Linux System V ABI, macOS, auto-detect via cfg!()
- `__VA_OPT__` (C23 extension) in preprocessor
- Named variadic macros (`args...`)
- `find_outside_strings` for correct function-like macro expansion
- Trailing comma in function calls

## [0.9.0] — 2026-04-14

### Added
- 40/41 chibicc tests compile
- `_Generic` expression support
- `typeof(type)` and `typeof(expr)`
- Compound literals `(int){1}`, `(int[]){0,1,2}`
- Inline `asm` statement parsing
- `&&label` labels-as-values (GCC extension)
- Computed `goto *p`
- `__builtin_*` function parsing
- Unicode identifiers (`int π = 3`)
- Wide char literals `L'x'`, `u'x'`, `U'x'`
- Hex float lexing `0x10.1p0`
- `__attribute__((packed))` on struct
- Dollar sign `$` in identifiers
- `__FILE__` and `__LINE__` builtin macros
- `#line` and linemarker directives
- Anonymous struct/union members
- Bitfield parsing (`int x:3`, anonymous `int:0`)
- `_Alignof` / `_Alignas` support
- `restrict` / `auto` / `register` qualifiers
- Case ranges `case 0 ... 5:` (GCC extension)
- GNU ternary `a ?: b`
- Unary `+` operator
- VLA parsing `int arr[n]`
- Designated initializer chains `.a.b[n]=val`
- Complex grouped declarators `char (*x[3])[4]`
- Function pointer parameters `void *(*fn)(void *)`
- Function type decay in params `int x()`
- Local typedef inside function body
- Comma-separated typedef `typedef int A, B[4]`
- Comma-separated global declarations
- Enum constant expressions `enum { ten=1+2+3+4 }`
- `va_list` / `va_start` / `va_arg` (stdarg.h stub)
- `pthread.h` stub

## [0.8.0] — 2026-03-28

### Added
- Full SSA construction with cross-block phi-like promotion
- Loop variables promoted to registers (mem2reg pass)
- Predecessor map for CFG
- Iterative dataflow for reaching definitions
- 166 chibicc-adapted assert tests (all pass)
- Peephole optimizer: self-move, push/pop→mov, dead store elimination
- IR-level optimizations: constant folding, dead instruction, branch simplification
- Register allocator infrastructure (linear scan with live ranges)

### Fixed
- Nested function call arg temp overlap (unique offsets per call)
- Switch default label placement
- Global int variables: proper typed load
- `sizeof(type *)` parsing
- Octal escape `\012` in char literals

## [0.7.0] — 2026-03-10

### Added
- Unsigned types: `UChar`, `UShort`, `UInt`, `ULong`
- Statement expressions `({ int a=5; a; })` (GCC extension)
- Preprocessor `#` stringify and `##` token paste
- Pointer arithmetic scaling (`ptr + n` multiplies by `sizeof(*ptr)`)
- `int ++/--` uses `addl`/`subl` (type-aware increment)
- `emit_load_typed()` for signed vs unsigned loads

## [0.6.0] — 2026-02-20

### Added
- WASM binary encoder (`--wasm-bin`, produces valid .wasm without wat2wasm)
- Fix double function params: store from xmm registers (Windows ABI)
- cJSON mini test (parse_value pattern, struct init, function pointers — all pass)
- Preprocessor: don't expand macros inside string literals

### Fixed
- cJSON Parse+Print: works when linked with gcc-compiled cJSON library
- Register allocator enabled for safe vregs (BinOp, Cmp, UnOp, Const)
- WASM backend: structured control flow with br_table dispatch loop
- Backslash line continuation in `#define`

## [0.5.0] — 2026-02-01

### Added
- Float/double codegen: `addsd`, `subsd`, `mulsd`, `divsd`, `ucomisd`
- Float printf: raw double bits passed via XMM + integer regs (Windows ABI)
- `strtod()` return value: read from xmm0 for double-returning functions
- `cvtsi2sd` / `cvttsd2si` for int-float casts
- Struct member access with proper offsets and typed load/store
- Self-referential struct support (linked lists)
- Struct assignment (memcpy for >8 byte structs)
- Function pointer initializers `{ malloc, free, realloc }`
- 5+ argument function calls (Windows x64 stack args)

### Fixed
- Switch/case codegen rewrite (jump table with labels)
- `sizeof(var)` uses actual variable type
- Nested init list flattening for struct initialization

## [0.4.0] — 2026-01-15

### Added
- Linear IR with virtual registers and basic blocks
- AST → IR lowering (expressions, statements, control flow)
- IR → x86-64 backend
- IR → WebAssembly Text (WAT) backend
- IR pretty-printer (`--ir` flag)
- `--use-ir` flag for IR compilation pipeline
- `--wasm` flag for WAT output
- cJSON library: parse, compile, assemble, link (CRUD operations work)

## [0.3.0] — 2025-12-20

### Added
- C preprocessor: `#define`, `#include`, `#ifdef/#ifndef/#if/#elif/#else/#endif`
- Function-like macros with parameter substitution
- Variadic macros with `__VA_ARGS__`
- 13 C standard library header stubs
- AST optimizations: constant folding, dead code elimination, strength reduction
- `--lint` flag: unused variable detection, unreachable code
- `--explain` flag: compilation pipeline steps
- `--bench` flag: compilation speed measurement
- Error recovery with "did you mean?" suggestions (Levenshtein distance)

## [0.2.0] — 2025-11-15

### Added
- Recursive descent parser for C
- AST with full expression, statement, and declaration support
- x86-64 codegen (Windows x64 ABI)
- Struct, union, enum, typedef support
- Array and pointer support
- Control flow: if/else, while, for, do-while, switch/case, break/continue, goto
- All arithmetic, bitwise, logical, comparison, assignment operators
- Ternary `?:`, comma, sizeof, cast
- Pre/post increment/decrement
- String and char literals with escape sequences
- Hex, octal, binary integer literals

## [0.1.0] — 2025-10-01

### Added
- Initial project structure
- Lexer: tokenization of C source code
- Basic CLI: `-o`, `-S`, `-E`, `--help`
- Colored error messages (Rust-style with `^` pointer)
- `printf("Hello, World!\n")` compiles and runs
- End-to-end: `.c` → `.s` → gcc → `.exe`

## License

Dual-licensed under MIT and Apache 2.0.
