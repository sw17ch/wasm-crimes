# How to Use

First, do not use this. These are crimes.

However, if you are a criminal, you probably are looking to do crimes. In which case, here is how to do crimes:

Build the guest:

```
cd wasm-crimes-guest/
cargo build --target wasm32-wasip1
```

Run the host:

```
cd wasm-crimes-host/
cargo run -- ../wasm-crimes-guest/target/wasm32-wasip1/debug/wasm_crimes_guest.wasm
```
