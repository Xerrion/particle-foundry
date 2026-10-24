import { resolve } from "node:path";

const expectedVersion = "wasm-bindgen 0.2.129";
const engine = resolve(import.meta.dir, "../../engine");

function installedVersion(): string | undefined {
  const result = Bun.spawnSync(["wasm-bindgen", "--version"], {
    stdout: "pipe",
    stderr: "ignore",
  });
  return result.exitCode === 0 ? result.stdout.toString().trim() : undefined;
}

if (installedVersion() !== expectedVersion) {
  const install = Bun.spawnSync(
    ["cargo", "install", "wasm-bindgen-cli", "--version", "0.2.129", "--locked"],
    {
      cwd: engine,
      stdout: "inherit",
      stderr: "inherit",
    },
  );
  if (install.exitCode !== 0) {
    throw new Error("Could not install the pinned wasm-bindgen CLI");
  }
}

const actualVersion = installedVersion();
if (actualVersion !== expectedVersion) {
  throw new Error(
    `Expected ${expectedVersion}, found ${actualVersion ?? "no installed CLI"}`,
  );
}

console.log(`Using ${actualVersion}`);
