import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  cpSync,
  rmSync,
  statSync,
  existsSync,
  chmodSync,
  readdirSync,
} from "node:fs";
import path from "node:path";
import os from "node:os";
import test from "node:test";
const repo = path.resolve(import.meta.dirname, "../..");
function fixture(
  t,
  {
    fresh = false,
    stopped = false,
    publicFailure = false,
    smokeFailure = false,
    reloadFailure = false,
    reloadDelay = false,
    bootEnableFailure = false,
    immutable = false,
    nginxStartFailure = false,
  } = {},
) {
  const stack = mkdtempSync(path.join(os.tmpdir(), "pipeline-maintenance-"));
  const writable = (directory) => {
    chmodSync(directory, 0o755);
    for (const entry of readdirSync(directory, { withFileTypes: true }))
      if (entry.isDirectory()) writable(path.join(directory, entry.name));
  };
  t.after(() => {
    if (immutable) writable(stack);
    rmSync(stack, { recursive: true, force: true });
  });
  const root = path.join(stack, "deployment");
  mkdirSync(path.join(stack, "nginx/conf.d"), { recursive: true });
  mkdirSync(path.join(stack, "bin"));
  mkdirSync(path.join(root, "scripts"), { recursive: true });
  cpSync(path.join(repo, "nginx-configs"), path.join(root, "nginx-configs"), {
    recursive: true,
  });
  cpSync(
    path.join(repo, "scripts/render_nginx_config.sh"),
    path.join(root, "scripts/render_nginx_config.sh"),
  );
  writeFileSync(path.join(root, "podman-compose.yml"), "services: {}");
  writeFileSync(path.join(stack, ".env"), "");
  writeFileSync(
    path.join(root, "scripts/smoke_test.sh"),
    `#!/bin/bash\n[[ $SMOKE_RELEASE_PROBE_TOKEN =~ ^[0-9a-f]{64}$ ]] || exit 64\nexit ${smokeFailure ? 9 : 0}\n`,
  );
  writeFileSync(
    path.join(stack, "bin/podman"),
    `#!/bin/bash
printf '%s\\n' "$*" >> "$SCHOOLORBIT_STACK_ROOT/commands"
if [[ $1 == container && $2 == exists ]]; then exit ${fresh ? 1 : 0}; fi
if [[ $1 == inspect ]]; then
 if [[ -f $SCHOOLORBIT_STACK_ROOT/backend-ready ]]; then printf 'true\\n'; else printf '%s\\n' ${stopped ? "false" : "true"}; fi
 exit 0
fi
if [[ $* == *'nginx -t'* && ${stopped ? "true" : "false"} == true && ! -f $SCHOOLORBIT_STACK_ROOT/backend-ready ]]; then
 if grep -q proxy_pass "$SCHOOLORBIT_STACK_ROOT/nginx/conf.d/"*.conf; then exit 8; fi
fi
if [[ $* == *'nginx -s reload'* && -f $SCHOOLORBIT_STACK_ROOT/fail-reload ]]; then rm "$SCHOOLORBIT_STACK_ROOT/fail-reload"; exit 8; fi
exit 0
`,
    { mode: 0o755 },
  );
  writeFileSync(
    path.join(stack, "bin/podman-compose"),
    '#!/bin/bash\nprintf "compose %s\\n" "$*" >> "$SCHOOLORBIT_STACK_ROOT/commands"\nif [[ -f $SCHOOLORBIT_STACK_ROOT/fail-nginx-start ]]; then rm "$SCHOOLORBIT_STACK_ROOT/fail-nginx-start"; exit 23; fi\n',
    { mode: 0o755 },
  );
  if (nginxStartFailure)
    writeFileSync(path.join(stack, "fail-nginx-start"), "");
  writeFileSync(
    path.join(stack, "bin/systemctl"),
    `#!/bin/bash
printf 'systemctl %s\\n' "$*" >> "$SCHOOLORBIT_STACK_ROOT/commands"
[[ $* == '--user enable podman-restart.service' || $* == '--user is-enabled --quiet podman-restart.service' ]] || exit 64
grep -q '"maintenance"' "$SCHOOLORBIT_STACK_ROOT/nginx/conf.d/school-api.conf" || exit 65
grep -q '"maintenance"' "$SCHOOLORBIT_STACK_ROOT/nginx/conf.d/admin-api.conf" || exit 65
exit ${bootEnableFailure ? 1 : 0}
`,
    { mode: 0o755 },
  );
  writeFileSync(
    path.join(stack, "bin/curl"),
    `#!/bin/bash
file=''; probe=false
while (($#)); do
 case "$1" in -o) file=$2; shift 2;; -H) probe=true; shift 2;; *) shift;; esac
done
if [[ $probe == true ]]; then printf '{"status":"ready"}'; exit 0; fi
if [[ -n $file ]]; then
 if [[ -f $SCHOOLORBIT_STACK_ROOT/delayed-reload ]]; then
  rm "$SCHOOLORBIT_STACK_ROOT/delayed-reload"
  if grep -q '"maintenance"' "$SCHOOLORBIT_STACK_ROOT/nginx/conf.d/school-api.conf"; then
   printf '{"status":"ready"}' > "$file"; printf 200
  else
   printf '{"error":"maintenance"}' > "$file"; printf 503
  fi
  exit 0
 fi
 if grep -q '"maintenance"' "$SCHOOLORBIT_STACK_ROOT/nginx/conf.d/school-api.conf"; then
  printf '{"error":"maintenance"}' > "$file"; printf 503
 else
  printf '{"status":"ready"}' > "$file"; printf ${publicFailure ? 500 : 200}
 fi
fi
`,
    { mode: 0o755 },
  );
  const env = {
    ...process.env,
    PATH: path.join(stack, "bin") + ":" + process.env.PATH,
    SCHOOLORBIT_STACK_ROOT: stack,
    BASE_DOMAIN: "example.test",
    RELEASE_SHA: "a".repeat(40),
  };
  let script = path.join(repo, "scripts/lib/pipeline-remote/maintenance.sh"),
    codeRoot = root;
  if (immutable) {
    env.RELEASE_BUNDLE_SHA256 = "b".repeat(64);
    codeRoot = path.join(
      root,
      "release-bundles",
      env.RELEASE_SHA,
      env.RELEASE_BUNDLE_SHA256,
    );
    mkdirSync(codeRoot, { recursive: true });
    for (const name of ["scripts", "nginx-configs", "podman-compose.yml"]) {
      cpSync(path.join(root, name), path.join(codeRoot, name), {
        recursive: true,
      });
      rmSync(path.join(root, name), { recursive: true });
    }
    const remote = path.join(codeRoot, "scripts/lib/pipeline-remote");
    mkdirSync(remote, { recursive: true });
    for (const name of ["maintenance.sh", "release-paths.sh"])
      cpSync(
        path.join(repo, "scripts/lib/pipeline-remote", name),
        path.join(remote, name),
      );
    writeFileSync(path.join(codeRoot, "release-bundle.json"), "{}");
    const seal = (directory) => {
      for (const entry of readdirSync(directory, { withFileTypes: true })) {
        const file = path.join(directory, entry.name);
        if (entry.isDirectory()) seal(file);
        else chmodSync(file, entry.name.endsWith(".sh") ? 0o555 : 0o444);
      }
      chmodSync(directory, 0o555);
    };
    seal(codeRoot);
    script = path.join(remote, "maintenance.sh");
  }
  const run = (mode, override = {}) => {
    if (reloadDelay && ["enter", "accept"].includes(mode))
      writeFileSync(path.join(stack, "delayed-reload"), "");
    return spawnSync("bash", [script, mode], {
      env: { ...env, ...override },
      encoding: "utf8",
    });
  };
  return { stack, root, codeRoot, run, reloadFailure };
}
test("immutable fresh maintenance retries a failed nginx start with a writable runtime Compose", (t) => {
  assert.notEqual(
    process.getuid(),
    0,
    "permission regressions require an unprivileged process",
  );
  const f = fixture(t, {
    immutable: true,
    fresh: true,
    nginxStartFailure: true,
  });
  const source = path.join(f.codeRoot, "podman-compose.yml"),
    active = path.join(f.stack, "podman-compose.yml");
  const hash = createHash("sha256").update(readFileSync(source)).digest("hex");
  assert.equal(f.run("enter").status, 23);
  assert.equal(statSync(active).mode & 0o777, 0o644);
  const probe = readFileSync(
    path.join(f.root, "releases", "a".repeat(40), "probe-token"),
  );
  const retry = f.run("enter");
  assert.equal(retry.status, 0, retry.stderr);
  assert.equal(statSync(active).mode & 0o777, 0o644);
  assert.deepEqual(readFileSync(active), readFileSync(source));
  assert.equal(
    createHash("sha256").update(readFileSync(source)).digest("hex"),
    hash,
  );
  assert.equal(statSync(source).mode & 0o777, 0o444);
  assert.deepEqual(
    readFileSync(path.join(f.root, "releases", "a".repeat(40), "probe-token")),
    probe,
  );
});
for (const mode of [0o400, 0o444])
  test(`immutable fresh maintenance recovers a stranded ${mode.toString(8)} active Compose`, (t) => {
    assert.notEqual(
      process.getuid(),
      0,
      "permission regressions require an unprivileged process",
    );
    const f = fixture(t, { immutable: true, fresh: true });
    const source = path.join(f.codeRoot, "podman-compose.yml"),
      active = path.join(f.stack, "podman-compose.yml");
    const bytes = readFileSync(source);
    writeFileSync(active, "stranded prior bytes\n", { mode });
    chmodSync(active, mode);
    assert.equal(statSync(active).mode & 0o777, mode);
    const result = f.run("enter");
    assert.equal(result.status, 0, result.stderr);
    assert.equal(statSync(active).mode & 0o777, 0o644);
    assert.deepEqual(readFileSync(active), bytes);
    assert.deepEqual(readFileSync(source), bytes);
    assert.equal(statSync(source).mode & 0o777, 0o444);
  });
test("immutable maintenance runs real code with stable journals and private probes", (t) => {
  const f = fixture(t, { immutable: true, fresh: true });
  for (const mode of ["enter", "verify", "accept"]) {
    const result = f.run(mode);
    assert.equal(result.status, 0, result.stderr);
  }
  assert.equal(
    readFileSync(path.join(f.root, "accepted-release"), "utf8").trim(),
    "a".repeat(40),
  );
  assert.equal(
    statSync(path.join(f.root, "releases", "a".repeat(40), "probe-token"))
      .mode & 0o777,
    0o600,
  );
  assert.equal(existsSync(path.join(f.codeRoot, "releases")), false);
  assert.equal(existsSync(path.join(f.codeRoot, "accepted-release")), false);
  assert.equal(statSync(f.codeRoot).mode & 0o777, 0o555);
});
test("immutable maintenance refuses missing/wrong identity and failed acceptance retains public maintenance", (t) => {
  const f = fixture(t, { immutable: true, smokeFailure: true });
  assert.equal(f.run("enter").status, 0);
  const probe = readFileSync(
    path.join(f.root, "releases", "a".repeat(40), "probe-token"),
    "utf8",
  );
  for (const value of ["", "c".repeat(64), "../shared"])
    assert.notEqual(
      f.run("verify", { RELEASE_BUNDLE_SHA256: value }).status,
      0,
    );
  assert.notEqual(f.run("accept").status, 0);
  assert.equal(existsSync(path.join(f.root, "pending-release")), true);
  assert.equal(existsSync(path.join(f.root, "accepted-release")), false);
  assert.equal(
    readFileSync(
      path.join(f.root, "releases", "a".repeat(40), "probe-token"),
      "utf8",
    ),
    probe,
  );
  for (const part of ["school", "admin"])
    assert.match(
      readFileSync(
        path.join(f.stack, "nginx/conf.d", part + "-api.conf"),
        "utf8",
      ),
      /return 503/,
    );
});
test("fresh origin starts only a maintenance proxy without unresolved backend upstreams", (t) => {
  const f = fixture(t, { fresh: true });
  const result = f.run("enter");
  assert.equal(result.status, 0, result.stderr);
  for (const part of ["school", "admin"]) {
    const source = readFileSync(
      path.join(f.stack, "nginx/conf.d", part + "-api.conf"),
      "utf8",
    );
    assert.match(source, /return 503/);
    assert.doesNotMatch(source, /proxy_pass|return 418/);
  }
  assert.match(
    readFileSync(path.join(f.stack, "commands"), "utf8"),
    /compose .*up -d --no-deps nginx/,
  );
  assert.equal(
    statSync(path.join(f.root, "releases", "a".repeat(40), "probe-token"))
      .mode & 0o777,
    0o600,
  );
});
test("releases enable boot recovery after entering maintenance without starting the unit", (t) => {
  const f = fixture(t);
  const result = f.run("enter");
  assert.equal(result.status, 0, result.stderr);
  const commands = readFileSync(path.join(f.stack, "commands"), "utf8");
  assert.ok(
    commands.indexOf("systemctl --user enable podman-restart.service") >
      commands.indexOf("nginx -s reload"),
  );
  assert.match(
    commands,
    /systemctl --user is-enabled --quiet podman-restart.service/,
  );
  assert.doesNotMatch(
    commands,
    /systemctl --user (?:start|restart|enable --now) /,
  );
});
test("boot recovery configuration failure keeps public maintenance active", (t) => {
  const f = fixture(t, { bootEnableFailure: true });
  assert.notEqual(f.run("enter").status, 0);
  assert.ok(existsSync(path.join(f.root, "pending-release")));
  for (const part of ["school", "admin"])
    assert.match(
      readFileSync(
        path.join(f.stack, "nginx/conf.d", part + "-api.conf"),
        "utf8",
      ),
      /return 503/,
    );
  assert.doesNotMatch(
    readFileSync(path.join(f.stack, "commands"), "utf8"),
    /systemctl --user (?:start|restart|enable --now) /,
  );
});
test("database cutover can re-enter maintenance when both source containers exist but are stopped", (t) => {
  const f = fixture(t, { stopped: true });
  const result = f.run("enter");
  assert.equal(result.status, 0, result.stderr);
  for (const part of ["school", "admin"]) {
    const source = readFileSync(
      path.join(f.stack, "nginx/conf.d", part + "-api.conf"),
      "utf8",
    );
    assert.match(source, /return 503/);
    assert.doesNotMatch(source, /proxy_pass|return 418/);
  }
  assert.doesNotMatch(
    readFileSync(path.join(f.stack, "commands"), "utf8"),
    /compose /,
  );
  writeFileSync(path.join(f.stack, "backend-ready"), "");
  assert.equal(f.run("enter").status, 0);
  for (const part of ["school", "admin"]) {
    const source = readFileSync(
      path.join(f.stack, "nginx/conf.d", part + "-api.conf"),
      "utf8",
    );
    assert.match(source, /return 503/);
    assert.match(source, /location @release_probe/);
    assert.match(source, /proxy_pass/);
  }
});
test("School boot delegates maintenance before replacement and refreshes private upstreams after readiness", () => {
  const source = readFileSync(
    path.join(repo, "scripts/lib/pipeline-remote/school-deploy-backend.sh"),
    "utf8",
  );
  const first = source.indexOf("if ! enter_maintenance ||"),
    second = source.indexOf("\nenter_maintenance\n");
  assert.ok(first >= 0);
  assert.ok(first < source.indexOf('school-storage-rollout.sh" cutover'));
  assert.ok(
    second > source.indexOf("schoolorbit_timer_report backend_readiness"),
  );
  assert.ok(second < source.indexOf("migration_status_started="));
  assert.doesNotMatch(source, /\/internal\/migrate-all/);
  assert.doesNotMatch(
    source,
    /cp "\$maintenance_proxy_source" "\$proxy_target"/,
  );
});
test("School maintenance receives validated domain and release after Compose removes exported context", (t) => {
  const source = readFileSync(
    path.join(repo, "scripts/lib/pipeline-remote/school-deploy-backend.sh"),
    "utf8",
  );
  const helper = source.match(/enter_maintenance\(\) \{[\s\S]*?\n\}/)?.[0];
  assert.ok(helper);
  const root = mkdtempSync(path.join(os.tmpdir(), "maintenance-context-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const remote = path.join(root, "scripts/lib/pipeline-remote");
  mkdirSync(remote, { recursive: true });
  writeFileSync(
    path.join(remote, "maintenance.sh"),
    "#!/bin/bash\nset -eu\n[[ $BASE_DOMAIN == example.test && $RELEASE_SHA == " +
      "a".repeat(40) +
      " && $1 == enter ]]\n",
  );
  const result = spawnSync(
    "bash",
    [
      "-euc",
      helper +
        "\nunset BASE_DOMAIN\nbase_domain=example.test\nRELEASE_SHA=" +
        "a".repeat(40) +
        '\ndeployment_root="$1"\nenter_maintenance',
      "fixture",
      root,
    ],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
});
test("private verification preserves maintenance and acceptance opens both proxies last", (t) => {
  const f = fixture(t);
  assert.equal(f.run("enter").status, 0);
  const first = readFileSync(
    path.join(f.root, "releases", "a".repeat(40), "probe-token"),
    "utf8",
  );
  assert.equal(f.run("enter").status, 0);
  assert.equal(
    readFileSync(
      path.join(f.root, "releases", "a".repeat(40), "probe-token"),
      "utf8",
    ),
    first,
  );
  assert.equal(f.run("verify").status, 0);
  assert.ok(existsSync(path.join(f.root, "pending-release")));
  assert.equal(f.run("accept").status, 0);
  assert.ok(!existsSync(path.join(f.root, "pending-release")));
  for (const part of ["school", "admin"])
    assert.doesNotMatch(
      readFileSync(
        path.join(f.stack, "nginx/conf.d", part + "-api.conf"),
        "utf8",
      ),
      /return 503/,
    );
});
for (const failure of [
  "smokeFailure",
  "publicFailure",
  "reloadFailure",
  "renderFailure",
])
  test(`${failure} refuses acceptance and leaves both public APIs in maintenance`, (t) => {
    const f = fixture(t, { [failure]: true });
    assert.equal(f.run("enter").status, 0);
    if (failure === "renderFailure")
      rmSync(path.join(f.root, "nginx-configs/admin-api.conf.template"));
    if (failure === "reloadFailure")
      writeFileSync(path.join(f.stack, "fail-reload"), "");
    assert.notEqual(f.run("accept").status, 0);
    assert.ok(existsSync(path.join(f.root, "pending-release")));
    assert.ok(!existsSync(path.join(f.root, "accepted-release")));
    for (const part of ["school", "admin"])
      assert.match(
        readFileSync(
          path.join(f.stack, "nginx/conf.d", part + "-api.conf"),
          "utf8",
        ),
        /return 503/,
      );
  });

test("entering and opening maintenance wait for the asynchronous Nginx reload to take effect", (t) => {
  const f = fixture(t, { reloadDelay: true });
  for (const mode of ["enter", "verify", "accept"]) {
    const result = f.run(mode);
    assert.equal(result.status, 0, result.stderr);
  }
  assert.ok(existsSync(path.join(f.root, "accepted-release")));
  assert.ok(!existsSync(path.join(f.root, "pending-release")));
});
