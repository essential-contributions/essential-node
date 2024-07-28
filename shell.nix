# A dev shell providing the essentials for working on essential-node.
{ cargo-toml-lint
, clippy
, essential-node-api
, mkShell
, rust-analyzer
, rustfmt
, cargo
, rustc
}:
mkShell {
  inputsFrom = [
    essential-node-api
  ];
  buildInputs = [
    cargo-toml-lint
    clippy
    rust-analyzer
    rustfmt
    cargo
    rustc
  ];
}
