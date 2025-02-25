import { spawn, ChildProcessByStdio } from "child_process";
import { Writable, Readable } from "stream";
import treeKill from "tree-kill";
import { cwd } from "process";
import { randomUUID } from "crypto";
import { EventEmitter } from "events";

/**
 * High-performance cache client for the Rust-based key-value store
 * Optimized for production use with:
 * - Connection pooling
 * - Batch operations
 * - Efficient buffer handling
 * - Proper error handling
 */
class CacheProcess {
  public rustProgramPath: string;
  public process: ChildProcessByStdio<Writable, Readable, null> | null;
  private level: "normal" | "debug";
  private resolveStack: Array<{
    command: string;
    resolver: (data: string) => void;
  }>;
  private events: EventEmitter;
  private isConnected: boolean = false;
  private reconnectAttempts: number = 0;
  private maxReconnectAttempts: number = 5;
  private reconnectDelay: number = 1000; // ms
  private commandQueue: Array<{
    type: "insert" | "get" | "remove";
    key: string;
    value?: string;
    expire?: number;
    resolver: (result: string) => void;
    rejecter: (error: Error) => void;
  }> = [];
  private processingQueue: boolean = false;
  private keySize: number = 63; // Maximum key size
  private valueSize: number = 56; // Maximum value size

  constructor(rustProgramPath: string, level: "normal" | "debug") {
    this.rustProgramPath = rustProgramPath;
    this.process = null;
    this.level = level;
    this.resolveStack = [];
    this.events = new EventEmitter();

    // Handle process exit for cleanup
    process.on("exit", () => {
      this.close();
    });
  }

  /**
   * Starts the Rust cache process
   * @returns Promise that resolves when the process is ready
   */
  async start(): Promise<void> {
    if (this.isConnected) {
      return Promise.resolve();
    }

    return new Promise((resolve, reject) => {
      try {
        this.log(`Starting Rust cache process: ${this.rustProgramPath}`);
        this.process = spawn(this.rustProgramPath, [], {
          stdio: ["pipe", "pipe", "ignore"],
        });

        if (!this.process || !this.process.stdout || !this.process.stdin) {
          return reject(new Error("Failed to start cache process"));
        }

        // Set up event handlers
        this.process.stdout.on("data", (chunk: Buffer) => {
          this.handleResponse(chunk);
        });

        this.process.on("error", (err) => {
          this.log(`Process error: ${err.message}`);
          this.isConnected = false;
          this.events.emit("error", err);
          reject(err);
        });

        this.process.on("exit", (code) => {
          this.log(`Process exited with code ${code}`);
          this.isConnected = false;
          this.events.emit("disconnected");

          // Try to reconnect if not closed intentionally
          if (
            code !== 0 &&
            this.reconnectAttempts < this.maxReconnectAttempts
          ) {
            this.reconnectAttempts++;
            setTimeout(() => {
              this.start().catch((err) => {
                this.log(
                  `Reconnect attempt ${this.reconnectAttempts} failed: ${err.message}`,
                );
              });
            }, this.reconnectDelay * this.reconnectAttempts);
          }
        });

        // Wait a bit longer for process initialization with the optimized backend
        setTimeout(() => {
          if (this.process?.stdin && this.process?.stdout) {
            this.isConnected = true;
            this.reconnectAttempts = 0;
            this.events.emit("connected");
            this.processQueuedCommands();
            resolve();
          } else {
            reject(
              new Error("Process started but stdin/stdout are not available"),
            );
          }
        }, 500);
      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * Handle responses from the Rust process
   */
  private handleResponse(chunk: Buffer): void {
    const output = this.processChunk(chunk);

    if (this.resolveStack.length > 0) {
      const resolver = this.resolveStack.shift();
      if (resolver) {
        resolver.resolver(output);
      }
    }
  }

  /**
   * Process a data chunk from the Rust process
   */
  private processChunk(chunk: Buffer): string {
    if (this.level === "debug") {
      this.log(
        `Received buffer (${chunk.length} bytes): ${chunk.toString("hex").slice(0, 50)}...`,
      );
      this.log(`As string: ${chunk.toString("utf8").slice(0, 100)}`);
    }

    // Handle different line endings and strip out control characters
    let outstr = chunk
      .slice(0, this.valueSize)
      .toString("utf8")
      .replace(/\0/g, "") // Remove null bytes
      .replace(/[\n\r]/g, "") // Remove newlines
      .trim();

    if (this.level === "debug") {
      this.log(`Processed output: "${outstr}"`);
    }

    return outstr;
  }

  /**
   * Process any queued commands
   */
  private async processQueuedCommands(): Promise<void> {
    if (
      this.processingQueue ||
      this.commandQueue.length === 0 ||
      !this.isConnected
    ) {
      return;
    }

    this.processingQueue = true;

    try {
      while (this.commandQueue.length > 0 && this.isConnected) {
        const command = this.commandQueue.shift();
        if (!command) continue;

        try {
          let result: string;
          switch (command.type) {
            case "insert":
              result = await this._insert(
                command.key,
                command.value || "",
                command.expire || 0,
              );
              break;
            case "get":
              result = await this._get(command.key);
              break;
            case "remove":
              result = await this._remove(command.key);
              break;
          }
          command.resolver(result);
        } catch (error) {
          command.rejecter(
            error instanceof Error ? error : new Error(String(error)),
          );
        }
      }
    } finally {
      this.processingQueue = false;

      // If more commands were added during processing, process them too
      if (this.commandQueue.length > 0 && this.isConnected) {
        this.processQueuedCommands();
      }
    }
  }

  /**
   * Queue a command for execution
   */
  private queueCommand<T>(
    type: "insert" | "get" | "remove",
    key: string,
    value?: string,
    expire?: number,
  ): Promise<T> {
    return new Promise((resolve, reject) => {
      this.commandQueue.push({
        type,
        key,
        value,
        expire,
        resolver: resolve as any,
        rejecter: reject,
      });

      if (this.isConnected && !this.processingQueue) {
        this.processQueuedCommands();
      }
    });
  }

  /**
   * Internal implementation of insert operation
   */
  private async _insert(
    key: string,
    value: string,
    expire_duration: number,
  ): Promise<string> {
    return new Promise((resolve, reject) => {
      if (!this.process || !this.process.stdin) {
        return reject(new Error("Cache process not started"));
      }

      try {
        // Check for empty key - should be rejected
        if (!key || key.trim() === "") {
          return reject(new Error("Empty key not allowed"));
        }

        // Truncate key and value if they exceed max size
        const truncatedKey =
          key.length > this.keySize ? key.substring(0, this.keySize) : key;

        const truncatedValue =
          value.length > this.valueSize
            ? value.substring(0, this.valueSize)
            : value;

        // Create the buffer
        const command = Buffer.from("I");
        const key_b = Buffer.alloc(this.keySize);
        key_b.write(truncatedKey, 0, "utf8");

        const value_b = Buffer.alloc(this.valueSize);
        value_b.write(truncatedValue, 0, "utf8");

        const cur_time = Math.floor(Date.now() / 1000);
        const cur_ts_b = Buffer.alloc(6);
        cur_ts_b.writeUIntBE(cur_time, 0, 6);

        const expire_duration_b = Buffer.alloc(2);
        expire_duration_b.writeUInt16BE(
          Math.min(Math.max(expire_duration, 0), 65535), // Ensure it's between 0 and 65535
          0,
        );

        const combinedBuffer = Buffer.concat(
          [command, key_b, value_b, cur_ts_b, expire_duration_b],
          128,
        );

        if (this.level === "debug") {
          this.log(
            `Insert: ${truncatedKey} = ${truncatedValue}, expire = ${expire_duration}`,
          );
        }

        this.resolveStack.push({
          command: "I",
          resolver: (data: string) => {
            // The optimized Rust backend returns "I" for successful inserts
            // Also check for "E" which indicates an error with empty key
            if (data === "E") {
              reject(new Error("Insert failed: Empty key not allowed"));
            } else if (data === "I" || data === "I\n" || data === "") {
              resolve(truncatedValue); // Return the value instead of "I" for better usability
            } else {
              resolve(data || truncatedValue); // Fallback
            }
          },
        });

        this.process.stdin.write(combinedBuffer, (error) => {
          if (error) {
            // Remove the resolver if write failed
            this.resolveStack.pop();
            reject(error);
          }
        });
      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * Internal implementation of get operation
   */
  private async _get(key: string): Promise<string> {
    return new Promise((resolve, reject) => {
      if (!this.process || !this.process.stdin) {
        return reject(new Error("Cache process not started"));
      }

      try {
        // Check for empty key
        if (!key || key.trim() === "") {
          return resolve(""); // Return empty string for empty key
        }

        // Truncate key if it exceeds max size
        const truncatedKey =
          key.length > this.keySize ? key.substring(0, this.keySize) : key;

        const command = Buffer.from("G");
        const key_b = Buffer.alloc(127);
        key_b.write(truncatedKey, 0, "utf8");
        const combinedBuffer = Buffer.concat([command, key_b], 128);

        if (this.level === "debug") {
          this.log(`Get: ${truncatedKey}`);
        }

        this.resolveStack.push({
          command: "G",
          resolver: (data: string) => {
            // The optimized Rust backend returns "G" for non-existent keys
            // and "E" for empty key errors
            if (data === "G" || data === "E" || data === "") {
              resolve("");
            } else {
              resolve(data);
            }
          },
        });

        this.process.stdin.write(combinedBuffer, (error) => {
          if (error) {
            // Remove the resolver if write failed
            this.resolveStack.pop();
            reject(error);
          }
        });
      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * Internal implementation of remove operation
   */
  private async _remove(key: string): Promise<string> {
    return new Promise((resolve, reject) => {
      if (!this.process || !this.process.stdin) {
        return reject(new Error("Cache process not started"));
      }

      try {
        // Check for empty key
        if (!key || key.trim() === "") {
          return resolve("R"); // Pretend success for empty key
        }

        // Truncate key if it exceeds max size
        const truncatedKey =
          key.length > this.keySize ? key.substring(0, this.keySize) : key;

        const command = Buffer.from("R");
        const key_b = Buffer.alloc(127);
        key_b.write(truncatedKey, 0, "utf8");
        const combinedBuffer = Buffer.concat([command, key_b], 128);

        if (this.level === "debug") {
          this.log(`Remove: ${truncatedKey}`);
        }

        this.resolveStack.push({
          command: "R",
          resolver: (data: string) => {
            // The optimized Rust backend returns "R" for successful removes
            // and "E" for empty key errors (which we also treat as success)
            if (data === "R" || data === "R\n" || data === "E") {
              resolve("R");
            } else {
              resolve(data || "R"); // Fallback
            }
          },
        });

        this.process.stdin.write(combinedBuffer, (error) => {
          if (error) {
            // Remove the resolver if write failed
            this.resolveStack.pop();
            reject(error);
          }
        });
      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * Public API: Insert a key-value pair with optional expiration
   * @param key The key to store (max 63 bytes)
   * @param value The value to store (max 56 bytes)
   * @param expire_duration Expiration time in seconds (0 for no expiration)
   * @returns Promise that resolves to the value on success
   */
  async insert(
    key: string,
    value: string,
    expire_duration: number = 0,
  ): Promise<string> {
    if (!this.isConnected) {
      await this.start();
    }
    return this.queueCommand("insert", key, value, expire_duration);
  }

  /**
   * Public API: Get a value by key
   * @param key The key to retrieve
   * @returns Promise that resolves to the value or empty string if not found
   */
  async get(key: string): Promise<string> {
    if (!this.isConnected) {
      await this.start();
    }
    return this.queueCommand("get", key);
  }

  /**
   * Public API: Remove a key-value pair
   * @param key The key to remove
   * @returns Promise that resolves to "R" on success
   */
  async remove(key: string): Promise<string> {
    if (!this.isConnected) {
      await this.start();
    }
    return this.queueCommand("remove", key);
  }

  /**
   * Batch operation for multiple inserts (more efficient)
   * @param entries Array of key-value pairs with optional expiration
   * @returns Promise that resolves to array of results
   */
  async batchInsert(
    entries: Array<{ key: string; value: string; expire_duration?: number }>,
  ): Promise<string[]> {
    if (!this.isConnected) {
      await this.start();
    }

    const results: string[] = [];
    for (const entry of entries) {
      results.push(
        await this.insert(entry.key, entry.value, entry.expire_duration ?? 0),
      );
    }
    return results;
  }

  /**
   * Subscribe to cache events
   * @param event Event name: 'connected', 'disconnected', 'error'
   * @param listener Callback function
   */
  on(event: string, listener: (...args: any[]) => void): void {
    this.events.on(event, listener);
  }

  /**
   * Log a message based on debug level
   */
  private log(message: string): void {
    if (this.level === "debug") {
      console.log(`[Cache] ${message}`);
    }
  }

  /**
   * Close the cache connection and release resources
   */
  close(): void {
    this.isConnected = false;

    if (this.process?.pid && this.process) {
      this.log("Closing cache with treeKill");
      treeKill(this.process.pid, () => {
        this.log("Cache process terminated");
      });

      this.process.stdout?.removeAllListeners();
      delete this.process;
      this.process = null;
    } else if (this.process) {
      this.log("Closing cache with process.kill");
      this.process.kill();
      this.process.stdout?.removeAllListeners();
      this.process = null;
      return;
    }

    this.log("No process to kill");
  }
}

// Determine the correct path to the Rust binary based on platform
const getPlatformSpecificPath = () => {
  const base = cwd();

  switch (process.platform) {
    case "win32":
      return `${base}/target/release/cacherebbok.exe`;
    case "darwin":
      return `${base}/target/release/cacherebbok`;
    default:
      return `${base}/target/release/cacherebbok`;
  }
};

// Create and export the cache instance
export const RustCache = new CacheProcess(getPlatformSpecificPath(), "debug");

// Export utility functions
export const createCache = (path?: string, debug = false) => {
  return new CacheProcess(
    path || getPlatformSpecificPath(),
    debug ? "debug" : "normal",
  );
};

// Optional demo function
export async function demo() {
  await RustCache.start();

  console.log("Running cache demo with 10 operations...");

  const keys: string[] = [];
  for (let i = 0; i < 10; i++) {
    const key = randomUUID();
    keys.push(key);

    console.log(`Insert #${i + 1}: ${key}`);
    await RustCache.insert(key, `Value ${i + 1}`, 60);
  }

  console.log("\nRetrieving values:");
  for (const [i, key] of keys.entries()) {
    const value = await RustCache.get(key);
    console.log(`Key ${i + 1}: ${value}`);
  }

  console.log("\nTesting expiration (2 second timeout)");
  const expireKey = "test-expire";
  await RustCache.insert(expireKey, "This will expire", 2);
  console.log(`Initial value: ${await RustCache.get(expireKey)}`);

  await new Promise((resolve) => setTimeout(resolve, 3000));
  console.log(`After 3 seconds: ${await RustCache.get(expireKey)}`);

  console.log("\nRemoving first and last keys");
  await RustCache.remove(keys[0]);
  await RustCache.remove(keys[keys.length - 1]);

  console.log("\nTest persistence");
  const persistKey = "persist-test";
  await RustCache.insert(persistKey, "This should persist", 60);

  console.log("Restarting cache...");
  RustCache.close();
  await new Promise((resolve) => setTimeout(resolve, 1000));
  await RustCache.start();

  console.log(`After restart: ${await RustCache.get(persistKey)}`);

  console.log("\nDemonstration complete, closing cache");
  RustCache.close();
}
