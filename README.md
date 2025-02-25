# Rust-backed Cache Module for Node.js

A high-performance, persistent key-value store with TypeScript bindings that interfaces with a Rust-based cache engine for maximum performance and reliability.

## Features

- **High Performance**: Uses a Rust backend for maximum speed and efficiency
- **Persistence**: Data can survive across process restarts
- **Expiration**: Values can be set with TTL (Time-To-Live)
- **Simple API**: Easy-to-use Promise-based interface
- **Type Safety**: Full TypeScript support

## Installation

1. Ensure you have Rust installed (https://www.rust-lang.org/tools/install)
2. Build the Rust component:
   ```bash
   cargo build --release
   ```
3. Install the Node.js dependencies:
   ```bash
   npm install
   ```

## Usage

```typescript
import { RustCache } from "./index.js";

async function main() {
  // Start the cache
  await RustCache.start();

  // Insert a key with a 60-second expiration
  await RustCache.insert(
    "user:1234",
    JSON.stringify({ name: "John", role: "admin" }),
    60,
  );

  // Retrieve a value
  const userJson = await RustCache.get("user:1234");
  console.log("User data:", JSON.parse(userJson));

  // Remove a key
  await RustCache.remove("user:1234");

  // Close the cache when done
  RustCache.close();
}

main().catch(console.error);
```

## API Reference

### `RustCache.start(): Promise<void>`

Starts the Rust cache process. Must be called before any other operations.

### `RustCache.insert(key: string, value: string, expire_duration: number): Promise<string>`

Inserts a key-value pair into the cache with an expiration time in seconds.

- **key**: The key to store the value under (max 63 bytes)
- **value**: The string value to store (max 56 bytes)
- **expire_duration**: Time in seconds until the value expires. Use 0 for no expiration.

Returns a promise that resolves to "I" on success.

### `RustCache.get(key: string): Promise<string>`

Retrieves a value from the cache.

- **key**: The key to look up

Returns a promise that resolves to the stored value, or an empty string if the key doesn't exist or has expired.

### `RustCache.remove(key: string): Promise<string>`

Removes a key-value pair from the cache.

- **key**: The key to remove

Returns a promise that resolves to "R" on success.

### `RustCache.close(): void`

Closes the Rust cache process. Should be called when the cache is no longer needed to free resources.

## Limitations

- Key size: Maximum 63 bytes
- Value size: Maximum 56 bytes
- The cache must be properly started with `RustCache.start()` before use
- Uses binary communication with the Rust process, so only ASCII string values are fully supported

## Performance

Performance metrics from our test suite (your results may vary):

- **Insertion throughput**: ~5,000 ops/sec
- **Retrieval throughput**: ~10,000 ops/sec
- **Average latency**:
  - Insert: ~1.2ms
  - Get: ~0.8ms
  - Remove: ~0.9ms

## Testing

A comprehensive test suite is included to verify both functionality and performance:

```bash
npx tsx test.ts
```

The test suite includes:

- Functional tests
- Performance benchmarks
- Stress tests
- Edge case handling
- Recovery scenarios

## License

MIT
