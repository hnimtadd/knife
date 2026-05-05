# Knife

A Rust-based CLI tool which includes my daily development tools/toils.

### How to use

1. **Install cargo (we need cargo to build the knife)**

   ```bash
   nix develop

   # or:
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. **Install knife to your bin path**:
    ```bash
    make install
    ```
    this will install knife to `~/.cargo/bin`, make sure your PATH include this one, if not:
    ```bash
    export PATH=$PATH:~/.cargo/bin
    ```
3. Register knife completion:
   ```bash
   eval "$(knife completion $(basename $SHELL) 2>/dev/null)"
   ```
   or add this below line to your shell configuration
   ```bash
   if type knife 1>/dev/null 2>&1; then
    eval "$(knife completion $(basename $SHELL) 2>/dev/null)"
   fi
   ```
3. Enjoy knife :)
    ```bash
    knife echo --debug # this will start echo server
    ```

## Development Setup

This project uses Nix for reproducible development environments.

### Prerequisites

- [Nix](https://nixos.org/download.html) installed on your system
- [direnv](https://direnv.net/) (optional, but recommended)


### Development

1. **Clone the repository and enter the development shell:**

   ```bash
   # With direnv (recommended)
   direnv allow

   # Or manually
   nix develop
   ```

2. **Build the project:**

   ```bash
   cargo build
   ```

3. **Run the project:**

   ```bash
   cargo run
   ```


### Available Commands

- `make build` - Build the project
- `make run` - Run the project
- `make test` - Run tests
- `make clean` - Clean build artifacts
- `make fmt` - Format code with rustfmt
- `make check` - Run cargo check
- `make dev` - Enter development shell
- `make install` - Install the binary

### Development Tools

The Nix development environment includes:

- **Rust toolchain** (stable with rust-src, rustfmt, clippy)
- **Development tools**: cargo-edit, cargo-watch, cargo-audit, cargo-outdated
- **Language server**: rust-analyzer
- **Build tools**: pkg-config, openssl
- **Git tools**: git, gh (GitHub CLI)
- **Utilities**: jq, curl, wget

### Building with Nix

You can also build the project using Nix directly:

```bash
nix build
```

This will create a `result` symlink with the built binary.

### Configuration Files

- `flake.nix` - Main Nix flake configuration
- `.envrc` - direnv configuration
- `shell.nix` - Backward compatibility shell
