import { spawn, ChildProcessByStdio } from "child_process";
import { Writable, Readable } from "stream";
import treeKill from "tree-kill";
import { cwd } from "process";
import { randomUUID } from "crypto";

class CacheProcess {
  public rustProgramPath: string;
  public process: ChildProcessByStdio<Writable, Readable, null> | null;
  private level: "normal" | "debug";
  private resolveStack: Array<{
    command: string;
    resolver: (data: string) => void;
  }>;

  constructor(rustProgramPath: string, level: "normal" | "debug") {
    this.rustProgramPath = rustProgramPath;
    this.process = null;
    this.level = level;
    this.resolveStack = [];
  }

  async start(): Promise<void> {
    this.process = spawn(this.rustProgramPath, [], {
      stdio: ["pipe", "pipe", "ignore"], // 'ignore' hides stderr, change to 'pipe' to debug errors
    });

    return new Promise((resolve, reject) => {
      if (this.process && this.process.stdout) {
        this.process.stdout.on("data", (chunk: Buffer) => {
          const output = this.processChunk(chunk);
          if (this.resolveStack.length > 0) {
            const resolver = this.resolveStack.shift();
            if (resolver) {
              resolver.resolver(output);
            }
          }
        });

        this.process.on("error", (err) => {
          reject(err);
        });

        // Consider the process started once we've set up listeners
        resolve();
      } else {
        reject(
          new Error(
            "Failed to spawn process or process stdout is not writable",
          ),
        );
      }
    });
  }

  processChunk(chunk: Buffer): string {
    if (this.level === "debug") {
      console.log("Received buffer from stdin: ", chunk);
      console.log("Received string from stdin: ", chunk.toString());
    }

    // Get the first 56 bytes and convert to string
    let outstr = chunk.slice(0, 56).toString().replace(/\0.*$/g, "").trim();

    if (this.level === "debug") {
      console.log("STDOUT:", outstr);
    }

    return outstr;
  }

  async insert(
    key: string,
    value: string,
    expire_duration: number,
  ): Promise<string> {
    return new Promise((resolve, reject) => {
      if (!this.process || !this.process.stdin) {
        if (this.level === "debug") {
          console.error("Rust program is not started.");
        }
        return resolve("");
      }

      try {
        // Create the buffer
        const command = Buffer.from("I");
        const key_b = Buffer.alloc(63);
        key_b.write(key, 0, "ascii");
        const value_b = Buffer.alloc(56);
        value_b.write(value, 0, "ascii");

        const cur_time = Math.floor(Date.now() / 1000);
        const cur_ts_b = Buffer.alloc(6);
        cur_ts_b.writeUIntBE(cur_time, 0, 6);

        const expire_duration_b = Buffer.alloc(2);
        expire_duration_b.writeUInt16BE(expire_duration, 0);

        const combinedBuffer = Buffer.concat(
          [command, key_b, value_b, cur_ts_b, expire_duration_b],
          128,
        );

        if (this.level === "debug") {
          console.log("DEBUG: Inserting key-value");
          console.log("Key:", key);
          console.log("Value:", value);
          console.log("Buffer length:", combinedBuffer.length);
        }

        this.resolveStack.push({
          command: "I",
          resolver: resolve,
        });

        this.process.stdin.write(combinedBuffer);
      } catch (error) {
        reject(error);
      }
    });
  }

  async get(key: string): Promise<string> {
    return new Promise((resolve, reject) => {
      try {
        if (!this.process || !this.process.stdin) {
          if (this.level === "debug") {
            console.error("Rust program is not started.");
          }
          return resolve("");
        }

        if (this.level === "debug") {
          console.log("DEBUG: Getting value for key", key);
        }

        const command = Buffer.from("G");
        const key_b = Buffer.alloc(127);
        key_b.write(key, 0, "ascii");
        const combinedBuffer = Buffer.concat([command, key_b], 128);

        this.resolveStack.push({
          command: "G",
          resolver: (data: string) => {
            if (data === "G") {
              if (this.level === "debug") {
                console.log("Key no longer exists");
              }
              resolve("");
            } else {
              resolve(data);
            }
          },
        });

        this.process.stdin.write(combinedBuffer);
      } catch (error) {
        reject(error);
      }
    });
  }

  async remove(key: string): Promise<string> {
    return new Promise((resolve, reject) => {
      try {
        if (!this.process || !this.process.stdin) {
          if (this.level === "debug") {
            console.error("Rust program is not started.");
          }
          return resolve("");
        }

        if (this.level === "debug") {
          console.log("DEBUG: Removing key", key);
        }

        const command = Buffer.from("R");
        const key_b = Buffer.alloc(127);
        key_b.write(key, 0, "ascii");
        const combinedBuffer = Buffer.concat([command, key_b], 128);

        this.resolveStack.push({
          command: "R",
          resolver: resolve,
        });

        this.process.stdin.write(combinedBuffer);
      } catch (error) {
        reject(error);
      }
    });
  }

  close(): void {
    if (this.process?.pid && this.process) {
      treeKill(this.process.pid, () => {
        if (this.level === "debug") {
          console.log("Cache closed");
        }
      });

      if (this.level === "debug") {
        console.log("Killed from treekill");
      }

      this.process.stdout?.removeAllListeners();
      return;
    } else if (this.process) {
      this.process.kill();

      if (this.level === "debug") {
        console.log("Killed from process");
      }

      this.process.stdout?.removeAllListeners();
      return;
    }

    if (this.level === "debug") {
      console.error("Could not kill process");
    }
  }
}

// Create and export the cache instance
const rustProgramPath = cwd() + "/target/release/cacherebbok.exe";
export const RustCache = new CacheProcess(rustProgramPath, "debug");

// Optional demo function
export async function demo() {
  await RustCache.start();
  let i = 0;
  let uuid = "";

  const inter = setInterval(async () => {
    let uuid_now = randomUUID().toString();
    console.log(await RustCache.insert(uuid_now, "hello world", 10));
    i += 1;
    uuid = uuid_now;

    if (i === 10) {
      clearInterval(inter);
      console.log(await RustCache.get(uuid));
      setTimeout(() => {
        RustCache.close();
      }, 2000);
    }
  }, 1000);
}
