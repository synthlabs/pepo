import { spawn } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const implementationTests = false;

export function runCommand(command, args, options) {
  return new Promise((resolveCommand, reject) => {
    const child = spawn(command, args, {
      ...options,
      stdio: "inherit",
      detached: process.platform !== "win32",
      shell: process.platform === "win32" && command === "pnpm",
    });
    let timer;
    const forward = (signal) => {
      if (process.platform === "win32") child.kill(signal);
      else if (child.pid) {
        try {
          process.kill(-child.pid, signal);
        } catch (error) {
          if (error.code !== "ESRCH") throw error;
        }
      }
      timer = setTimeout(() => {
        if (process.platform === "win32") child.kill("SIGKILL");
        else if (child.pid) {
          try {
            process.kill(-child.pid, "SIGKILL");
          } catch (error) {
            if (error.code !== "ESRCH") throw error;
          }
        }
      }, 5000);
      timer.unref();
    };
    const interrupt = () => forward("SIGINT");
    const terminate = () => forward("SIGTERM");
    const hangup = () => forward("SIGHUP");
    process.on("SIGINT", interrupt);
    process.on("SIGTERM", terminate);
    process.on("SIGHUP", hangup);
    const cleanup = () => {
      clearTimeout(timer);
      process.off("SIGINT", interrupt);
      process.off("SIGTERM", terminate);
      process.off("SIGHUP", hangup);
    };
    child.once("error", (error) => {
      cleanup();
      reject(error);
    });
    child.once("exit", (code, signal) => {
      cleanup();
      if (code === 0) resolveCommand();
      else {
        const error = new Error(
          `${command} ${args.join(" ")} failed (${signal ?? code})`,
        );
        error.exitCode = code ?? (signal === "SIGINT" ? 130 : 1);
        reject(error);
      }
    });
  });
}

export async function runInternalTests({
  root = process.cwd(),
  execute = runCommand,
  noRun = false,
} = {}) {
  root = resolve(root);
  const testRun = noRun ? ["--no-run"] : ["--", "--test-threads=1"];
  const app = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).name;
  if (!["scrybe", "pepo"].includes(app))
    throw new Error("Run internal-test.mjs from a Scrybe or Pepo app root");
  const internal = join(root, "internal");
  for (const relative of [
    "src-tauri/Cargo.toml",
    "internal/frontend/index.ts",
    "internal/rust/commands.rs",
  ]) {
    if (!existsSync(join(root, relative)))
      throw new Error("Missing " + join(root, relative));
  }
  const options = { cwd: root, env: { ...process.env, ENABLE_INTERNAL: "1" } };
  await execute(
    process.execPath,
    ["--test", join(internal, "internal-test.test.mjs")],
    options,
  );
  await execute("pnpm", ["check"], options);
  await execute(
    "cargo",
    ["check", "--locked", "--manifest-path", "src-tauri/Cargo.toml"],
    options,
  );
  await execute(
    "cargo",
    [
      "test",
      "--locked",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--lib",
      ...testRun,
    ],
    options,
  );
  if (implementationTests) {
    const fixture = mkdtempSync(join(tmpdir(), "internal-rust-tests-"));
    try {
      const source = join(internal, "rust");
      if (!existsSync(join(source, "Cargo.lock")))
        throw new Error("Missing internal/rust/Cargo.lock");
      cpSync(source, join(fixture, "rust"), {
        recursive: true,
        filter: (path) => basename(path) !== "target",
      });
      await execute(
        "cargo",
        [
          "test",
          "--locked",
          "--manifest-path",
          join(fixture, "rust/Cargo.toml"),
          "--features",
          "runtime",
          "--lib",
          ...testRun,
        ],
        {
          ...options,
          env: {
            ...options.env,
            CARGO_TARGET_DIR:
              process.env.CARGO_TARGET_DIR ??
              join(root, "target/internal-tests"),
          },
        },
      );
    } finally {
      rmSync(fixture, { recursive: true, force: true });
    }
  }
}

if (
  process.argv[1] &&
  realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  runInternalTests({ noRun: process.argv.includes("--no-run") }).catch((error) => {
    console.error(error.message);
    process.exitCode = error.exitCode ?? 1;
  });
}
