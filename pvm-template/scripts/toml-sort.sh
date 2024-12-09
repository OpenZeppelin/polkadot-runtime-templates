#!/bin/bash

# From the workspace directory, run :
# ./scripts/toml-sort.sh
# to format all Cargo.toml files, and 
# ./scripts/toml-sort.sh --check
# to only check the formatting.

if ! type "toml-sort" > /dev/null; then
  echo "Run 'cargo install --git https://github.com/4meta5/toml_sort'"
else
  find . -name "Cargo.toml" -not -path "*/target/*" -exec toml-sort {} $@ \;
fi