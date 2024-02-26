#!/bin/bash

ZOMBIENET_V=v1.3.91
POLKADOT_V=v1.6.0

case "$(uname -s)" in
    Linux*)     MACHINE=Linux;;
    Darwin*)    MACHINE=Mac;;
    *)          exit 1
esac

if [ $MACHINE = "Linux" ]; then
  ZOMBIENET_BIN=zombienet-linux-x64
elif [ $MACHINE = "Mac" ]; then
  ZOMBIENET_BIN=zombienet-macos
fi

BIN_DIR=bin-$POLKADOT_V

build_polkadot() {
  echo "cloning polkadot repository..."
  CWD=$(pwd)
  mkdir -p "$BIN_DIR"
  pushd /tmp
    git clone https://github.com/paritytech/polkadot-sdk.git
    pushd polkadot-sdk
      git checkout release-polkadot-$POLKADOT_V
      echo "building polkadot executable..."
      cargo build --release --features fast-runtime
      cp target/release/polkadot "$CWD/$BIN_DIR"
      cp target/release/polkadot-execute-worker "$CWD/$BIN_DIR"
      cp target/release/polkadot-prepare-worker "$CWD/$BIN_DIR"
    popd
  popd
}

fetch_polkadot() {
  echo "fetching from polkadot repository..."
  echo $BIN_DIR
  mkdir -p "$BIN_DIR"
  pushd "$BIN_DIR"
    wget https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-$POLKADOT_V/polkadot
    wget https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-$POLKADOT_V/polkadot-execute-worker
    wget https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-$POLKADOT_V/polkadot-prepare-worker
    chmod +x *
  popd
}

zombienet_init() {
  if [ ! -f $ZOMBIENET_BIN ]; then
    echo "fetching zombienet executable..."
    curl -LO https://github.com/paritytech/zombienet/releases/download/$ZOMBIENET_V/$ZOMBIENET_BIN
    chmod +x $ZOMBIENET_BIN
  fi
  if [ ! -f $BIN_DIR/polkadot ]; then
    fetch_polkadot
  fi
}

zombienet_build() {
  if [ ! -f $ZOMBIENET_BIN ]; then
    echo "fetching zombienet executable..."
    curl -LO https://github.com/paritytech/zombienet/releases/download/$ZOMBIENET_V/$ZOMBIENET_BIN
    chmod +x $ZOMBIENET_BIN
  fi
  if [ ! -f $BIN_DIR/polkadot ]; then
    build_polkadot
  fi
}

zombienet_devnet() {
  zombienet_init
  cargo build --release
  echo "spawning rococo-local relay chain plus devnet as a parachain..."
  local dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
  ./$ZOMBIENET_BIN spawn "$dir/../zombienet-config/devnet.toml" -p native
}


print_help() {
  echo "This is a shell script to automate the execution of zombienet."
  echo ""
  echo "$ ./zombienet.sh init         # fetches zombienet and polkadot executables"
  echo "$ ./zombienet.sh build        # builds polkadot executables from source"
  echo "$ ./zombienet.sh devnet       # spawns a rococo-local relay chain plus parachain devnet-local as a parachain"
}

SUBCOMMAND=$1
case $SUBCOMMAND in
  "" | "-h" | "--help")
    print_help
    ;;
  *)
    shift
    zombienet_${SUBCOMMAND} $@
    if [ $? = 127 ]; then
      echo "Error: '$SUBCOMMAND' is not a known SUBCOMMAND." >&2
      echo "Run './zombienet.sh --help' for a list of known subcommands." >&2
        exit 1
    fi
  ;;
esac
