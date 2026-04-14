# blue-book

blue-book is a bit-perfect CD ripper and archival utility. It guarantees sector-perfect accuracy via **AccurateRip** verification, automating the pipeline from raw disc data to fully tagged, lossless FLAC archives.

# Dependencies:

## riprip

```bash
brew install libcdio

export BINDGEN_EXTRA_CLANG_ARGS="-I/opt/homebrew/include"
export CFLAGS="-I/opt/homebrew/include"
export LDFLAGS="-L/opt/homebrew/lib"
export PKG_CONFIG_PATH="/opt/homebrew/opt/libcdio/lib/pkgconfig"

cargo install --git https://github.com/Blobfolio/riprip.git --bin riprip
```

## Python Environment

This project uses **PEP 723** inline metadata. We recommend using [uv](https://github.com/astral-sh/uv) for a seamless experience—it will automatically handle Python versioning and dependencies without needing a virtualenv.

# Usage:

Insert a CD and run:

```bash
uv run blue_book.py
```
