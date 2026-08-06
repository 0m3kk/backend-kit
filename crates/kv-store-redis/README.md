# kv-store-redis

Redis `KvStore` implementation using [`redis-rs`](https://crates.io/crates/redis).

## Features

- **Redis Backed**: Connects asynchronously via `ConnectionManager`.
- **Native Options**: Maps `NX`, `XX`, and `EX`/`PX` TTL natively to Redis `SET` options.
- **Pipelining**: Executes batch mutations atomically using Redis pipelines.
- **Async Streaming**: Streams keys using Redis `SCAN` commands.
