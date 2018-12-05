# edgeware-node

A Parity Substrate node implementing our edgeware-related modules.

## Implemented Modules

* [edge_identity](https://github.com/hicommonwealth/edge_identity)

## TBD Modules

* [edge_bridge](https://github.com/hicommonwealth/edge_bridge)
* [edge_delegation](https://github.com/hicommonwealth/edge_delegation)

## Adding A Module

1. Add its github repo to:
  - [Cargo.toml](Cargo.toml)
  - [runtime/Cargo.toml](runtime/Cargo.toml)
  - [runtime/wasm/Cargo.toml](runtime/wasm/Cargo.toml) (be sure to have `default-features = false`)
2. Changes to [the runtime](runtime/lib.rs):
  - Add it as an `extern crate`.
  - Implement its `Trait` with production types.
  - Add it to the `construct_runtime` macro with all implemented components.
3. Changes to [the chain spec](src/chain_spec.rs):
  - Add it to the `edgeware_runtime`'s list of `Config` types.
  - Add it to the `testnet_genesis` function, initializing all storage fields set to `config()`.
4. Build and run the chain.
5. (Optional) If using new types, add them to the API options in [Edge Api](https://github.com/hicommonwealth/edge_api).

## Usage

### Initial Setup

```
curl https://sh.rustup.rs -sSf | sh
rustup update nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup update stable
cargo install --git https://github.com/alexcrichton/wasm-gc
sudo apt install cmake pkg-config libssl-dev git clang libclang-dev
```

### Building

```
./build.sh
cargo build --release
```

### Running

```
./target/release/edgeware --dev
```
