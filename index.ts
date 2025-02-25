import { spawn, ChildProcessByStdio } from "child_process";
import { Writable, Readable } from "stream";
import treeKill from "tree-kill";
import { cwd } from "process";

class CacheProcess {
  private rustProgramPath: string;
  private process: ChildProcessByStdio<Writable, Readable, null> | null;
  private level: "normal" | "debug";
  private resolveStack: Array<[string, (data: string) => void]>;

  private createCombinedBufferForInsert(
    key: string,
    value: string,
    expire_duration: number,
  ): Buffer {
    if (this.level === "debug") {
      console.log("DEBUG: Inserting key-value");
    }
    const cur_time = Math.floor(Date.now() / 1000);
    const cur_ts_b = Buffer.alloc(6);
    cur_ts_b.writeUIntBE(cur_time, 0, 6);

    const expire_duration_b = Buffer.alloc(2);
    expire_duration_b.writeUInt16BE(expire_duration, 0);

    const command = Buffer.from("I");
    const key_b = Buffer.alloc(63);
    key_b.write(key, 0, "ascii");
    const value_b = Buffer.alloc(56);
    value_b.write(value, 0, "ascii");

    let final_b = Buffer.concat(
      [command, key_b, value_b, cur_ts_b, expire_duration_b],
      128,
    );

    if (this.level === "debug") {
      console.log("Created insert buffer: ", final_b);
      console.log("Created insert string: ", final_b.toString());
    }

    return final_b;
  }

  private createCombinedBufferForGetAndRemove(
    isGet: boolean,
    key: string,
  ): Buffer {
    if (this.level === "debug") {
      console.log("DEBUG: Getting value for key");
    }

    let command = Buffer.from("");

    if (isGet) {
      command = Buffer.from("G");
    } else {
      command = Buffer.from("R");
    }
    const keyBuffer = Buffer.alloc(63, key, "ascii");
    return Buffer.concat([command, keyBuffer], 128);
  }

  processChunk(chunk: Buffer): string {
    if (this.level === "debug") {
      console.log("Recieved buffer from stdin: ", chunk);
      console.log("Recieved string from stdin: ", chunk.toString());
    }
    let outstr = chunk.subarray(0, 56).toString();
    let output: string = Array.from(outstr).join("");

    output = output.replace("\n", "").trim();

    if (this.level === "debug") {
      console.log("STDOUT:", output);
    }

    return output;
  }

  private handleProcessData(data: string) {
    // Get most recent resolve
    const resolve = this.resolveStack.shift();
    resolve[1](data);
    return;
  }

  constructor(rustProgramPath: string, level: "normal" | "debug") {
    this.rustProgramPath = rustProgramPath;
    this.process = null;
    this.level = level;
    this.resolveStack = new Array();
  }

  async start(): Promise<void> {
    this.process = spawn(this.rustProgramPath, [], {
      stdio: ["pipe", "pipe", "ignore"],
    });

    return new Promise((resolve, reject) => {
      if (this.process && this.process.stdout) {
        this.process.stdout.on("data", (chunk: Buffer) => {
          const output = this.processChunk(chunk);
          this.handleProcessData(output);
        });

        this.process.on("error", (err) => {
          reject(err);
        });

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

  async insert(
    key: string,
    value: string,
    expire_duration: number,
  ): Promise<string> {
    return new Promise((resolve) => {
      if (!this.process || !this.process.stdin) {
        return resolve("E");
      }
      const buf = this.createCombinedBufferForInsert(
        key,
        value,
        expire_duration,
      );
      this.process.stdin.write(buf);
      this.resolveStack.push(["I", resolve]);
    });
  }

  async get(key: string): Promise<string> {
    return new Promise((resolve) => {
      if (!this.process || !this.process.stdin) {
        return resolve("E");
      }
      const buf = this.createCombinedBufferForGetAndRemove(true, key);
      this.process.stdin.write(buf);
      this.resolveStack.push(["G", resolve]);
    });
  }

  async remove(key: string): Promise<string> {
    return new Promise((resolve) => {
      if (!this.process || !this.process.stdin) {
        return resolve("E");
      }
      const buf = this.createCombinedBufferForGetAndRemove(false, key);
      this.process.stdin.write(buf);
      this.resolveStack.push(["R", resolve]);
    });
  }

  close(): void {
    if (this.process?.pid && this.process) {
      treeKill(this.process.pid);
      this.process.stdout?.removeAllListeners();
      return;
    } else if (this.process) {
      this.process.kill();
      if (this.level === "debug") {
        console.error("Killed from process");
      }
      this.process.stdout?.removeAllListeners();
      return;
    }
    if (this.level === "debug") {
      console.error("could not kill");
    }
  }
}

const rustProgramPath = cwd() + "/target/release/cacherebbok.exe";
export const RustCache = new CacheProcess(rustProgramPath, "debug");
