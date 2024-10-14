# A dev shell providing the essentials for working on essential-node.
{ pint
, mkShell
}:
mkShell {
  buildInputs = [
    pint
  ];
}
