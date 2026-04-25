#!/bin/bash
# Use lld if available (brew install llvm), otherwise fall back to default clang linker.
LLD="/opt/homebrew/opt/llvm/bin/ld.lld"
if [ -x "$LLD" ]; then
    exec clang "-fuse-ld=$LLD" "$@"
else
    exec clang "$@"
fi
