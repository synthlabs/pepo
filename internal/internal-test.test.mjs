import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  implementationTests,
  runCommand,
  runInternalTests,
} from "./internal-test.mjs";

const internal = dirname(fileURLToPath(import.meta.url));
if (existsSync(join(internal, "manifests"))) {
  test("both public manifests export the hook and its contract tests", () => {
    for (const app of ["scrybe", "pepo"]) {
      const files = readFileSync(
        join(internal, "manifests", app + ".files"),
        "utf8",
      )
        .trim()
        .split("\n");
      for (const file of ["internal-test.mjs", "internal-test.test.mjs"]) {
        assert.ok(files.includes(file), app + ": " + file);
        assert.ok(existsSync(join(internal, file)));
      }
      assert.ok(
        !files.some(
          (file) =>
            file.startsWith("scripts/") || file === "rust/src/translation.rs",
        ),
      );
    }
  });
}

function fixture(t, app = "scrybe") {
  const root = mkdtempSync(join(tmpdir(), "internal-hook-test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const relative of [
    "src-tauri/Cargo.toml",
    "internal/frontend/index.ts",
    "internal/rust/commands.rs",
    "internal/rust/Cargo.toml",
    "internal/rust/Cargo.lock",
  ]) {
    const path = join(root, relative);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, "");
  }
  writeFileSync(join(root, "package.json"), JSON.stringify({ name: app }));
  return root;
}

for (const app of ["scrybe", "pepo"]) {
  test(
    app +
      ": internal-enabled checks and tests run sequentially without host recursion",
    async (t) => {
      const root = fixture(t, app);
      const calls = [];
      let active = false;
      let copiedManifest;
      await runInternalTests({
        root,
        execute: async (command, args, options) => {
          assert.equal(active, false);
          active = true;
          await Promise.resolve();
          assert.equal(options.cwd, root);
          assert.equal(options.env.ENABLE_INTERNAL, "1");
          assert.ok(
            !args.includes("test:internal") && !args.includes("verify"),
          );
          calls.push([command, args]);
          if (args.includes("--features")) {
            copiedManifest = args[args.indexOf("--manifest-path") + 1];
            assert.ok(existsSync(copiedManifest));
            assert.ok(!copiedManifest.startsWith(root));
            assert.equal(args[args.indexOf("--features") + 1], "runtime");
          }
          active = false;
        },
      });
      assert.equal(calls.length, implementationTests ? 5 : 4);
      assert.deepEqual(calls.slice(0, 4), [
        [
          process.execPath,
          ["--test", join(root, "internal/internal-test.test.mjs")],
        ],
        ["pnpm", ["check"]],
        [
          "cargo",
          ["check", "--locked", "--manifest-path", "src-tauri/Cargo.toml"],
        ],
        [
          "cargo",
          [
            "test",
            "--locked",
            "--manifest-path",
            "src-tauri/Cargo.toml",
            "--lib",
            "--",
            "--test-threads=1",
          ],
        ],
      ]);
      if (implementationTests)
        assert.equal(existsSync(dirname(copiedManifest)), false);
    },
  );
}

test("a failing check stops later commands and keeps its error", async (t) => {
  const root = fixture(t);
  const failure = Object.assign(new Error("fixture failure"), { exitCode: 29 });
  const calls = [];
  await assert.rejects(
    runInternalTests({
      root,
      execute: async (command) => {
        calls.push(command);
        if (command === "pnpm") throw failure;
      },
    }),
    (error) => error === failure && error.exitCode === 29,
  );
  assert.deepEqual(calls, [process.execPath, "pnpm"]);
});

test("--no-run builds Rust tests without running them", async (t) => {
  const root = fixture(t);
  const tests = [];
  await runInternalTests({
    root,
    noRun: true,
    execute: async (command, args) => {
      if (command === "cargo" && args[0] === "test") tests.push(args);
    },
  });
  assert.equal(tests.length, implementationTests ? 2 : 1);
  for (const args of tests) {
    assert.equal(args.at(-1), "--no-run");
    assert.ok(!args.includes("--"));
  }
});

test("the CLI runs when reached through a symlinked directory", async (t) => {
  const root = fixture(t, "another-app");
  const link = join(root, "linked-internal");
  symlinkSync(internal, link, "junction");
  await assert.rejects(
    runCommand(process.execPath, [join(link, "internal-test.mjs")], {
      cwd: root,
    }),
    (error) => error.exitCode === 1,
  );
});

test("unsupported hosts fail before starting checks", async (t) => {
  const root = fixture(t, "another-app");
  await assert.rejects(
    runInternalTests({
      root,
      execute: () => assert.fail("Started an unsupported host"),
    }),
    /Scrybe or Pepo app root/,
  );
});

test(
  "a hangup stops the running child",
  {
    skip: process.platform === "win32" && "SIGHUP is POSIX-only",
    timeout: 20000,
  },
  async (t) => {
    const hook = pathToFileURL(join(internal, "internal-test.mjs")).href;
    const harness = spawn(
      process.execPath,
      [
        "--input-type=module",
        "-e",
        `import { runCommand } from ${JSON.stringify(hook)};
await runCommand(process.execPath, ["-e", "console.log(process.pid); setInterval(() => {}, 1000)"], {})
  .catch(() => { process.exitCode = 1; });`,
      ],
      { stdio: ["ignore", "pipe", "inherit"] },
    );
    const [output] = await once(harness.stdout.setEncoding("utf8"), "data");
    const child = Number(output.trim());
    t.after(() => {
      try {
        process.kill(child, "SIGKILL");
      } catch {}
    });
    harness.kill("SIGHUP");
    await once(harness, "exit");
    assert.throws(() => process.kill(child, 0), { code: "ESRCH" });
  },
);

test("spawn errors and child exit codes propagate", async () => {
  await assert.rejects(
    runCommand("scrybe-missing-fixture-command", [], { cwd: tmpdir() }),
    { code: "ENOENT" },
  );
  await assert.rejects(
    runCommand(
      process.execPath,
      [
        "-e",
        "console.log('fixture stdout'); console.error('fixture stderr'); process.exit(37)",
      ],
      { cwd: tmpdir() },
    ),
    (error) => error.exitCode === 37,
  );
});

if (implementationTests) {
  test("a failed implementation test removes its temporary crate", async (t) => {
    const root = fixture(t);
    const failure = new Error("implementation failure");
    let manifest;
    await assert.rejects(
      runInternalTests({
        root,
        execute: async (_, args) => {
          if (args.includes("--features")) {
            manifest = args[args.indexOf("--manifest-path") + 1];
            assert.ok(existsSync(manifest));
            throw failure;
          }
        },
      }),
      (error) => error === failure,
    );
    assert.equal(existsSync(dirname(manifest)), false);
    assert.ok(existsSync(join(root, "internal/rust/Cargo.lock")));
  });
}
