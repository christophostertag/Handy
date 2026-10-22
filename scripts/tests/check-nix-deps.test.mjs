import assert from "node:assert/strict";
import { test } from "node:test";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const hash = (value) => createHash("sha256").update(value).digest("hex");
function fixture(t, converter) {
  const dir = mkdtempSync(join(tmpdir(), "handy-node-nix-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  mkdirSync(join(dir, "scripts"));
  mkdirSync(join(dir, ".nix"));
  mkdirSync(join(dir, "bin"));
  writeFileSync(join(dir, "package.json"), '{"type":"module"}');
  copyFileSync(
    new URL("../check-nix-deps.ts", import.meta.url),
    join(dir, "scripts/check-nix-deps.ts"),
  );
  writeFileSync(join(dir, "bun.lock"), "changed-lock");
  writeFileSync(join(dir, ".nix/bun-lock-hash"), "old-hash\n");
  if (converter != null)
    writeFileSync(join(dir, "bin/bun2nix"), converter, { mode: 0o755 });
  return {
    dir,
    run: () =>
      spawnSync(process.execPath, ["scripts/check-nix-deps.ts"], {
        cwd: dir,
        env: { ...process.env, PATH: join(dir, "bin") },
        encoding: "utf8",
      }),
    stored: () => readFileSync(join(dir, ".nix/bun-lock-hash"), "utf8").trim(),
  };
}

test("a Node-only install skips an already current Nix graph without a converter", (t) => {
  const f = fixture(t);
  writeFileSync(join(f.dir, ".nix/bun-lock-hash"), hash("changed-lock"));
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "");
});
test("a standalone converter updates the graph and hash only after success", (t) => {
  const f = fixture(t, '#!/bin/sh\nprintf "generated" > "$2"\n');
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(join(f.dir, ".nix/bun.nix"), "utf8"), "generated");
  assert.equal(f.stored(), hash("changed-lock"));
});
for (const [name, converter] of [
  ["missing", undefined],
  ["failed", "#!/bin/sh\nexit 1\n"],
]) {
  test(`a ${name} converter preserves the old hash and does not break a non-Nix install`, (t) => {
    const f = fixture(t, converter);
    const result = f.run();
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stderr, /may be outdated/);
    assert.equal(f.stored(), "old-hash");
  });
}
