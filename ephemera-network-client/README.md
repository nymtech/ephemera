# Client

This is meant to try out sending protobuf messages to protocol instances.

It also allows to generate key pairs.

### Send broadcast messages

```bash
RUST_LOG="debug"  cargo run -- --ephemera
```

### Protobuf messages

`broadcast.rs` is copied from the `broadcast` workspace.
