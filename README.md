# Oxidown

A blazing fast, multi-threaded file downloader written in Rust.

## Features

- **Multi-threaded downloading**: Downloads files in parallel chunks for maximum speed.
- **Resumable downloads**: Can resume interrupted downloads.
- **HTTP/S and proxy support**: Works with standard web protocols and proxies.
- **Cross-platform**: Runs on Windows, macOS, and Linux.

## Installation

1. Ensure you have Rust installed: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
2. Clone the repository: `git clone https://github.com/mengguyi/oxidown.git`
3. Build the project: `cd oxidown && cargo build --release`
4. The executable will be in `target/release/oxidown`.

## Usage

```sh
oxidown <URL> [OPTIONS]
```

### Examples

**Download a file:**

```sh
oxidown https://example.com/large-file.zip
```

**Download with a specific output name:**

```sh
oxidown https://example.com/large-file.zip -O my-file.zip
```

**Download with 16 threads:**

```sh
oxidown https://example.com/large-file.zip --threads 16
```

**See all options:**

```sh
oxidown --help
```
