# A dev shell providing the essentials for working on essential-node.
{ cargo-toml-lint
, clippy
, essential-node
, mkShell
, rust-analyzer
, rustfmt
, cargo
, rustc
, openssl
}:
mkShell {
  inputsFrom = [
    essential-node
  ];
  buildInputs = [
    cargo-toml-lint
    clippy
    rust-analyzer
    rustfmt
    cargo
    rustc
    openssl
    openssl.dev
  ];
}
