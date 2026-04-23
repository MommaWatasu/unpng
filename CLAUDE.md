# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run tests
cargo test

# Run a single test
cargo test test_decode

# Check without building
cargo check

# Lint
cargo clippy
```

## Architecture

`unpng` is a pure-Rust, no-std-compatible PNG decoder (the `#![no_std]` line is commented out but the structure supports it via `extern crate alloc`). It has no external dependencies.

The decoding pipeline in `lib.rs::decode()` flows through three stages:

1. **Chunk parsing** (`src/core.rs`) — `ChunkIter` walks the raw PNG bytes after verifying the 8-byte signature, yielding `Chunk` structs. `parse_ihdr` extracts the image header; `collect_idat` concatenates all IDAT chunk payloads into a single buffer.

2. **Decompression** (`src/zlib.rs` → `src/deflate.rs`) — `zlib_decompress` strips the 2-byte zlib header and hands the payload to `inflate`. `inflate` handles all three DEFLATE block types (stored, fixed Huffman, dynamic Huffman) using a `BitReader` and a flat lookup `HuffmanTree` (15-bit wide table).

3. **Unfiltering** (`src/filter.rs`) — `unfilter` reverses the five PNG row filters (None, Sub, Up, Average, Paeth) row-by-row, producing the final raw pixel bytes.

The public API is `decode(&[u8]) -> Result<(ImageHeader, Vec<u8>), PngError>`. The `prelude` module re-exports `core::unpng` (a signature-check helper).

### Known debug artifacts
`decode_symbols` in `src/deflate.rs` contains `println!` calls left from development; these will cause compilation to fail in a true `no_std` environment and add noise to test output.
