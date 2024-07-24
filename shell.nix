# A dev shell providing the essentials for working on essential-node.
{ cargo-toml-lint
, clippy
, mkShell
, rust-analyzer
, rustfmt
, cargo
, rustc
}:
mkShell {
  buildInputs = [
    cargo-toml-lint
    clippy
    rust-analyzer
    rustfmt
    cargo
    rustc
  ];
}
