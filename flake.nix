{
  description = "Knife - A Rust-based CLI tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Use the latest stable Rust toolchain
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain  # Includes cargo, rustc, rustfmt, clippy
            pkg-config
            openssl
            # Development tools
            git
            just  # Alternative to make
            rust-analyzer  # LSP for IDE support
            bacon  # Cargo watch alternative
          ];

          shellHook = ''
            # Set up environment variables
            export RUST_BACKTRACE=1
            export RUST_LOG=debug
          '';
        };

        # Optional: define packages for building
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "knife";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          buildInputs = with pkgs; [
            pkg-config
            openssl
          ];
        };
      });
}
