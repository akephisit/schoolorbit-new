import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  cpSync,
  rmSync,
  existsSync,
  statSync,
  unlinkSync,
  chmodSync,
  appendFileSync,
  readdirSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
const repo = path.resolve(import.meta.dirname, "../..");
const sha = "a".repeat(40),
  digest = "sha256:" + "e".repeat(64),
  publicHash = "b".repeat(64),
  privateHash = "c".repeat(64),
  inventoryHash = "d".repeat(64);
const evidence = {
  schema_version: 1,
  evidence_digest: "9".repeat(64),
  expected_inventory_sha256: inventoryHash,
  expected_public_namespace_sha256: publicHash,
  expected_private_namespace_sha256: privateHash,
  scope: "all_unbound",
};
function input(value, manual = "true") {
  return spawnSync(
    "bash",
    [
      "-euc",
      '. "$1"; storage_namespace_input',
      "fixture",
      path.join(repo, "scripts/lib/pipeline-remote/storage-namespace-input.sh"),
    ],
    {
      env: {
        ...process.env,
        NAMESPACE_ATTESTATION_JSON: value,
        NAMESPACE_ATTESTATION_MANUAL: manual,
      },
      encoding: "utf8",
    },
  );
}
test("only explicit manual strict bounded attestation JSON reaches transport", () => {
  assert.equal(input("", "false").status, 0);
  assert.equal(input(JSON.stringify(evidence)).status, 0);
  assert.equal(input(JSON.stringify(evidence), "false").status, 64);
  for (const value of [
    JSON.stringify({ ...evidence, extra: "private-sentinel" }),
    JSON.stringify({ ...evidence, scope: "auto" }),
    JSON.stringify({ ...evidence, evidence_digest: "private-sentinel" }),
    JSON.stringify(evidence) + JSON.stringify(evidence),
    JSON.stringify(evidence).replace(
      '"schema_version":1',
      '"schema_version":1,"schema_version":1',
    ),
    " ".repeat(4097) + JSON.stringify(evidence),
  ]) {
    const r = input(value);
    assert.equal(r.status, 64);
    assert.doesNotMatch(r.stdout + r.stderr, /private-sentinel/);
  }
});
function fixture(t, options = {}) {
  const stack = mkdtempSync(path.join(os.tmpdir(), "storage-rollout-"));
  t.after(() => rmSync(stack, { recursive: true, force: true }));
  const root = path.join(stack, "deployment");
  mkdirSync(path.join(root, "scripts/lib/pipeline-remote"), {
    recursive: true,
  });
  mkdirSync(path.join(stack, "bin"));
  mkdirSync(path.join(stack, "postgresql"));
  for (const name of [
    "runtime-env.sh",
    "storage-namespace-input.sh",
    "school-storage-rollout.sh",
  ])
    cpSync(
      path.join(repo, "scripts/lib/pipeline-remote", name),
      path.join(root, "scripts/lib/pipeline-remote", name),
    );
  writeFileSync(path.join(stack, "postgresql/ca.crt"), "synthetic");
  if (options.customCa) {
    writeFileSync(path.join(stack, "postgresql/custom ca.crt"), "synthetic");
  }
  writeFileSync(
    path.join(stack, ".env"),
    Object.entries({
      BACKEND_ADMIN_URL: "http://admin:8080",
      INTERNAL_API_SECRET: "private-sentinel",
      R2_ACCOUNT_ID: "fixed-account",
      R2_PUBLIC_BUCKET_NAME: "public-fixed",
      R2_PRIVATE_BUCKET_NAME: "private-fixed",
    })
      .map(([k, v]) => `${k}='${v}'`)
      .join("\n") + "\n",
    { mode: 0o600 },
  );
  if (options.customCa)
    appendFileSync(
      path.join(stack, ".env"),
      `POSTGRES_CA_FILE='${stack}/postgresql/custom ca.crt'\n`,
    );
  const state = {
    Id: "f".repeat(64),
    Image: "1".repeat(64),
    State: { Running: true },
    Config: {
      Env: [
        "R2_ACCOUNT_ID=fixed-account",
        "R2_PUBLIC_BUCKET_NAME=public-fixed",
        `R2_PRIVATE_BUCKET_NAME=${options.oldMismatch ? "private-other" : "private-fixed"}`,
        "INTERNAL_API_SECRET=private-sentinel",
      ],
    },
  };
  if (!options.fresh)
    writeFileSync(path.join(stack, "container.json"), JSON.stringify(state));
  writeFileSync(path.join(stack, "options.json"), JSON.stringify(options));
  writeFileSync(
    path.join(stack, "bin/podman"),
    `#!/usr/bin/env node
const fs=require('node:fs'),p=require('node:path');const root=process.env.FIXTURE_ROOT,args=process.argv.slice(2),opts=JSON.parse(fs.readFileSync(p.join(root,'options.json'))),file=p.join(root,'container.json');
const oneshot=p.join(root,'oneshot.json');const state=(name)=>JSON.parse(fs.readFileSync(name==='schoolorbit-school-rollout'?oneshot:file));const log=JSON.stringify({args,envKeys:Object.keys(process.env).filter(k=>k.startsWith('R2_')||k.startsWith('FILE_STORAGE_NAMESPACE_'))});fs.appendFileSync(p.join(root,'commands'),log+'\\n');
if(args[0]==='image'&&args[1]==='exists')process.exit(0);
if(args[0]==='container'&&args[1]==='exists')process.exit(fs.existsSync(args[2]==='schoolorbit-school-rollout'?oneshot:file)?0:1);
if(args[0]==='inspect'){const format=args[2],s=state(args.at(-1));if(format==='{{.Id}}')console.log(s.Id);else if(format==='{{.Image}}')console.log(s.Image);else if(format==='{{.State.Running}}')console.log(s.State.Running);else if(format==='{{json .Config.Env}}')console.log(JSON.stringify(s.Config.Env));else if(format==='{{json .Config}}')console.log(JSON.stringify(s.Config));else if(format==='{{json .Config.Labels}}')console.log(JSON.stringify(s.Config.Labels));else process.exit(91);process.exit(0);}
if(args[0]==='stop'){if(opts.stopFailure)process.exit(1);const s=state();s.State.Running=!!opts.stopLies;fs.writeFileSync(file,JSON.stringify(s));process.exit(0);}
if(args[0]==='rm'){if(state().State.Running)process.exit(92);fs.unlinkSync(file);process.exit(0);}
if(args[0]==='run'){
 const i=args.indexOf('/app/backend-school'),command=args.slice(i+1).join(' ');if(i<0||args.includes('--replace')||!args.includes('--read-only')||!args.includes('--userns=keep-id')||args[args.indexOf('--user')+1]!==process.getuid()+':'+process.getgid()||args.some(a=>a==='-p'||a==='--publish'))process.exit(93);
 const name=args[args.indexOf('--name')+1];const labels={};for(let n=0;n<args.length;n++)if(args[n]==='--label'){const v=args[n+1],i=v.indexOf('=');labels[v.slice(0,i)]=v.slice(i+1);}
 const operation=command==='migrate-tenants'?'migrate-tenants':command.split(' ').at(-1);
 if(name!=='schoolorbit-school-rollout'||labels['org.schoolorbit.rollout.release']!==process.env.RELEASE_SHA||labels['org.schoolorbit.rollout.image']!==process.env.BACKEND_IMAGE_DIGEST||labels['org.schoolorbit.rollout.operation']!==operation)process.exit(100);
 try{fs.writeFileSync(oneshot,JSON.stringify({State:{Running:true},Config:{Labels:labels}}),{flag:'wx'});}catch(error){if(error.code==='EEXIST'){console.error('private-sentinel name collision');process.exit(125);}throw error;}
 let orphan=false;process.on('exit',()=>{if(!orphan)fs.unlinkSync(oneshot);});
 fs.appendFileSync(p.join(root,'executed'),command+'\\n');
 if(opts.orphanOperation==='migrate-tenants'&&operation==='migrate-tenants'){orphan=true;process.exit(1);}
 const route={schema_version:1,public_namespace_sha256:'${publicHash}',private_namespace_sha256:process.env.R2_PRIVATE_BUCKET_NAME==='private-other'?'0'.repeat(64):'${privateHash}'};
 if(command==='storage-namespace routing-fingerprints'){if(args[args.indexOf('--network')+1]!=='none')process.exit(94);console.log(JSON.stringify(route));process.exit(0);}
 if(command==='storage-namespace context-admin'){console.log(JSON.stringify({...route,tenants:opts.fresh?0:2,inventory_sha256:opts.inventoryMismatch?'0'.repeat(64):'${inventoryHash}',...(opts.hostileContext?{private:'private-sentinel'}:{})}));process.exit(0);}
 if(fs.existsSync(file))process.exit(95);
 if(command==='migrate-tenants'){if(opts.migrationFailure){console.log(JSON.stringify({schema_version:1,tenants:2,migrated:1,failed:1,latest_version:137}));console.error('storage_binding_database_failed');console.error('private-sentinel postgres://secret');process.exit(1);}console.log(JSON.stringify({schema_version:1,tenants:opts.fresh?0:2,migrated:opts.fresh?0:2,failed:0,latest_version:137}));process.exit(0);}
 if(command==='storage-namespace attest-admin'){
  if(!process.env.FILE_STORAGE_NAMESPACE_ATTESTATION_JSON||!args.includes('--env')||process.env.FILE_STORAGE_NAMESPACE_MANIFEST_DIRECTORY!=='/run/storage-namespace-manifests')process.exit(96);
  const mount=args.find(a=>a.endsWith(':/run/storage-namespace-manifests:rw'));if(!mount)process.exit(97);const directory=mount.split(':')[0];const st=fs.statSync(directory);if((st.mode&511)!==448||st.uid!==process.getuid())process.exit(98);
  fs.writeFileSync(p.join(directory,'exact-manifest.json'),JSON.stringify({watch_ids:['synthetic-exact-id'],expected_revision:1}),{mode:0o600,flag:'wx'});
  if(opts.orphanOperation==='attest-admin'){orphan=true;process.exit(1);}  if(opts.attestFailure){console.error('storage_binding_active_claim');process.exit(1);}console.log(JSON.stringify({schema_version:1,tenants:2,applied:1,ready:true}));process.exit(0);
 }
 if(command==='storage-namespace check-admin'){if(opts.checkFailure){console.log(JSON.stringify({schema_version:1,tenants:2,checked:2,failed:0,watches:3,unresolved:2,mismatched:1,...(opts.maliciousFailure?{tenants:'private-sentinel'}:{})}));console.error('storage_binding_namespace_invalid');console.error('private-sentinel postgres://secret');process.exit(1);}console.log(JSON.stringify({schema_version:1,tenants:opts.fresh?0:2,checked:opts.fresh?0:2,failed:0,watches:0,unresolved:0,mismatched:0}));process.exit(0);}
}
process.exit(99);
`,
    { mode: 0o755 },
  );
  if (options.fsyncFailure)
    writeFileSync(path.join(stack, "bin/python3"), "#!/bin/sh\nexit 1\n", {
      mode: 0o755,
    });
  if (options.fsyncStopFailure) {
    const python = spawnSync("bash", ["-c", "command -v python3"], {
      encoding: "utf8",
    }).stdout.trim();
    writeFileSync(
      path.join(stack, "bin/python3"),
      `#!/usr/bin/env node
if(process.argv[3]?.includes('/stopped-'))process.exit(1);const r=require('node:child_process').spawnSync(${JSON.stringify(python)},process.argv.slice(2),{stdio:'inherit'});process.exit(r.status??1);
`,
      { mode: 0o755 },
    );
  }
  const env = {
    ...process.env,
    PATH: path.join(stack, "bin") + ":" + process.env.PATH,
    FIXTURE_ROOT: stack,
    SCHOOLORBIT_STACK_ROOT: stack,
    RELEASE_SHA: sha,
    BACKEND_IMAGE_DIGEST: digest,
    NAMESPACE_ATTESTATION_JSON: options.attest ? JSON.stringify(evidence) : "",
    NAMESPACE_ATTESTATION_MANUAL: options.attest ? "true" : "false",
  };
  const script = path.join(
    root,
    "scripts/lib/pipeline-remote/school-storage-rollout.sh",
  );
  const run = (mode, override = {}) =>
    spawnSync("bash", [script, mode], {
      env: { ...env, ...override },
      encoding: "utf8",
    });
  const maintenance = () =>
    writeFileSync(path.join(root, "pending-release"), sha + "\n");
  const deploy = readFileSync(
    path.join(repo, "scripts/lib/pipeline-remote/school-deploy-backend.sh"),
    "utf8",
  );
  const block = deploy.slice(
    deploy.indexOf("# The headless phase"),
    deploy.indexOf("# /ready checks"),
  );
  const cutover = () =>
    spawnSync(
      "bash",
      [
        "-euc",
        `deployment_root="$1"\nexport_school_compose_env(){ :; }\nunset_school_compose_env(){ :; }\ncompose_up_quiet(){ touch "$FIXTURE_ROOT/started"; }\nvalidate_backend_network(){ :; }\n` +
          block,
        "fixture",
        root,
      ],
      { env, encoding: "utf8" },
    );
  return {
    stack,
    root,
    env,
    run,
    maintenance,
    cutover,
    journal: path.join(root, "releases", sha, "storage-namespace"),
    commands: () =>
      existsSync(path.join(stack, "commands"))
        ? readFileSync(path.join(stack, "commands"), "utf8")
            .trim()
            .split("\n")
            .filter(Boolean)
            .map((s) => JSON.parse(s))
        : [],
  };
}
test("regular release executes real headless stages before actual production start block, never attests", (t) => {
  const f = fixture(t);
  assert.equal(f.run("preflight").status, 0);
  assert.equal(
    f.commands().some((x) => x.args[0] === "stop"),
    false,
  );
  f.maintenance();
  const r = f.cutover();
  assert.equal(r.status, 0, r.stderr);
  assert.ok(existsSync(path.join(f.stack, "started")));
  const calls = f.commands().map((x) => x.args.join(" "));
  const stop = calls.findIndex((x) => x.startsWith("stop ")),
    remove = calls.findIndex((x) => x.startsWith("rm ")),
    migrate = calls.findIndex((x) => x.endsWith("migrate-tenants")),
    check = calls.findIndex((x) => x.endsWith("check-admin"));
  assert.ok(stop < remove && remove < migrate && migrate < check);
  assert.ok(
    calls.slice(stop + 1, remove).some((x) => x.includes(".State.Running")),
  );
  assert.equal(
    calls.some((x) => x.endsWith("attest-admin")),
    false,
  );
  assert.doesNotMatch(
    r.stdout + r.stderr,
    /private-sentinel|fixed-account|public-fixed/,
  );
  assert.equal(
    statSync(path.join(f.journal, "old-routing-config.json")).mode & 0o777,
    0o600,
  );
  assert.equal(
    statSync(path.join(f.journal, "old-context.json")).mode & 0o777,
    0o600,
  );
});
test("explicit binding verifies old and target proof and writes a private durable manifest as mapped host user", (t) => {
  const f = fixture(t, { attest: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  const r = f.cutover();
  assert.equal(r.status, 0, r.stderr);
  assert.ok(existsSync(path.join(f.stack, "started")));
  assert.equal(statSync(path.join(f.journal, "manifests")).mode & 0o777, 0o700);
  assert.equal(
    statSync(path.join(f.journal, "manifests/exact-manifest.json")).mode &
      0o777,
    0o600,
  );
  assert.doesNotMatch(
    r.stdout + r.stderr,
    /private-sentinel|fixed-account|public-fixed/,
  );
});
for (const fault of ["oldMismatch", "inventoryMismatch"])
  test(`wrong attestation ${fault} fails before stopping old writer`, (t) => {
    const f = fixture(t, { attest: true, [fault]: true });
    const r = f.run("preflight");
    assert.notEqual(r.status, 0);
    assert.equal(
      f.commands().some((x) => x.args[0] === "stop"),
      false,
    );
    assert.ok(existsSync(path.join(f.stack, "container.json")));
  });
for (const fault of [
  "migrationFailure",
  "checkFailure",
  "stopFailure",
  "stopLies",
])
  test(`${fault} never starts a backend or restores old binary`, (t) => {
    const f = fixture(t, { [fault]: true });
    assert.equal(f.run("preflight").status, 0);
    f.maintenance();
    const r = f.cutover();
    assert.notEqual(r.status, 0);
    assert.equal(existsSync(path.join(f.stack, "started")), false);
    assert.equal(
      f.commands().some((x) => x.args[0] === "start"),
      false,
    );
    assert.ok(existsSync(path.join(f.root, "pending-release")));
    if (fault.startsWith("stop"))
      assert.equal(
        f.commands().some((x) => x.args[0] === "rm"),
        false,
      );
    else assert.ok(existsSync(path.join(f.journal, "migration-attempted")));
  });
test("first installation accepts authoritative empty inventory without historical proof or binding", (t) => {
  const f = fixture(t, { fresh: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  assert.equal(f.cutover().status, 0);
  assert.equal(
    JSON.parse(readFileSync(path.join(f.journal, "old-context.json"))).routing,
    null,
  );
  assert.equal(
    f.commands().some((x) => x.args.join(" ").endsWith("attest-admin")),
    false,
  );
});
test("same release retry preserves old proof after container removal and refuses another target digest", (t) => {
  const f = fixture(t, { checkFailure: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  assert.notEqual(f.cutover().status, 0);
  const proof = readFileSync(path.join(f.journal, "old-context.json"), "utf8");
  assert.equal(f.run("preflight").status, 0);
  assert.equal(
    readFileSync(path.join(f.journal, "old-context.json"), "utf8"),
    proof,
  );
  assert.notEqual(
    f.run("preflight", { BACKEND_IMAGE_DIGEST: "sha256:" + "2".repeat(64) })
      .status,
    0,
  );
});
test("missing or unsafe journal proof cannot be reconstructed from target configuration", (t) => {
  const f = fixture(t, { checkFailure: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  assert.notEqual(f.cutover().status, 0);
  unlinkSync(path.join(f.journal, "old-context.json"));
  assert.notEqual(f.run("preflight").status, 0);
});
test("public mode on private routing proof is refused", (t) => {
  const f = fixture(t);
  assert.equal(f.run("preflight").status, 0);
  chmodSync(path.join(f.journal, "old-context.json"), 0o644);
  assert.notEqual(f.run("preflight").status, 0);
});

test("preflight emits only the approved complete context and routing hashes", (t) => {
  const f = fixture(t);
  const r = f.run("preflight");
  assert.equal(r.status, 0, r.stderr);
  const values = r.stdout
    .split("\n")
    .filter((x) => x.startsWith("{"))
    .map((x) => JSON.parse(x));
  assert.equal(values[0].inventory_sha256, inventoryHash);
  assert.equal(values[0].tenants, 2);
  assert.equal(values[1].public_namespace_sha256, publicHash);
  for (const file of readdirSync(f.journal)) {
    const full = path.join(f.journal, file);
    if (statSync(full).isFile())
      assert.doesNotMatch(readFileSync(full, "utf8"), /private-sentinel/);
  }
  assert.doesNotMatch(
    r.stdout + r.stderr,
    /private-sentinel|fixed-account|public-fixed/,
  );
});
test("canonical custom CA path is mounted unchanged for networked headless commands", (t) => {
  const f = fixture(t, { customCa: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  assert.equal(f.cutover().status, 0);
  for (const call of f
    .commands()
    .filter(
      (x) =>
        x.args[0] === "run" &&
        x.args[x.args.indexOf("--network") + 1] !== "none",
    ))
    assert.ok(
      call.args.includes(
        path.join(f.stack, "postgresql/custom ca.crt") +
          ":/run/secrets/postgresql-ca.crt:ro",
      ),
    );
});
test("failure exposes approved aggregate counts and known static code but never arbitrary stderr", (t) => {
  const f = fixture(t, { checkFailure: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  const r = f.cutover();
  assert.notEqual(r.status, 0);
  assert.match(r.stdout, /"unresolved":2/);
  assert.match(r.stdout, /"mismatched":1/);
  assert.match(r.stderr, /storage_binding_namespace_invalid/);
  assert.doesNotMatch(r.stdout + r.stderr, /private-sentinel|postgres:\/\//);
});
test("malformed aggregate and hostile context cannot disclose arbitrary process output", (t) => {
  const f = fixture(t, { checkFailure: true, maliciousFailure: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  let r = f.cutover();
  assert.notEqual(r.status, 0);
  assert.doesNotMatch(r.stdout + r.stderr, /private-sentinel/);
  const g = fixture(t, { hostileContext: true });
  r = g.run("preflight");
  assert.notEqual(r.status, 0);
  assert.doesNotMatch(r.stdout + r.stderr, /private-sentinel/);
  assert.equal(
    g.commands().some((x) => x.args[0] === "stop"),
    false,
  );
});
test("journal fsync failure prevents disruptive stages", (t) => {
  const f = fixture(t, { fsyncFailure: true });
  const r = f.run("preflight");
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /journal_sync_failed/);
  assert.deepEqual(f.commands(), []);
  assert.ok(existsSync(path.join(f.stack, "container.json")));
});
test("changed private routing configuration invalidates its durable integrity handle", (t) => {
  const f = fixture(t);
  assert.equal(f.run("preflight").status, 0);
  writeFileSync(path.join(f.journal, "old-routing-config.json"), "[]");
  assert.notEqual(f.run("preflight").status, 0);
  assert.equal(
    f.commands().some((x) => x.args[0] === "stop"),
    false,
  );
});
test("stop proof remains immutable when the migration-attempt marker is published", (t) => {
  const f = fixture(t, { checkFailure: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  assert.notEqual(f.cutover().status, 0);
  const name = "stopped-" + "f".repeat(64);
  assert.equal(
    readFileSync(path.join(f.journal, name), "utf8"),
    "f".repeat(64) + "\n",
  );
  assert.equal(
    readFileSync(path.join(f.journal, "migration-attempted"), "utf8"),
    sha + "\n",
  );
});

test("a stop-proof fsync failure preserves the stopped container and prevents migration", (t) => {
  const f = fixture(t, { fsyncStopFailure: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  const r = f.cutover();
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /journal_sync_failed/);
  assert.ok(existsSync(path.join(f.stack, "container.json")));
  assert.equal(
    f
      .commands()
      .some(
        (x) =>
          x.args[0] === "rm" || x.args.join(" ").endsWith("migrate-tenants"),
      ),
    false,
  );
});
test("active claim blocks explicit binding without deleting its durable exact manifest or starting writers", (t) => {
  const f = fixture(t, { attest: true, attestFailure: true });
  assert.equal(f.run("preflight").status, 0);
  f.maintenance();
  const r = f.cutover();
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /storage_binding_active_claim/);
  assert.ok(existsSync(path.join(f.journal, "manifests/exact-manifest.json")));
  assert.equal(existsSync(path.join(f.stack, "started")), false);
  assert.equal(
    f.commands().some((x) => x.args.join(" ").endsWith("check-admin")),
    false,
  );
});
test("changed container identity between preflight and cutover refuses to stop the replacement", (t) => {
  const f = fixture(t);
  assert.equal(f.run("preflight").status, 0);
  const file = path.join(f.stack, "container.json");
  const state = JSON.parse(readFileSync(file));
  state.Id = "2".repeat(64);
  writeFileSync(file, JSON.stringify(state));
  f.maintenance();
  assert.notEqual(f.cutover().status, 0);
  assert.equal(
    f.commands().some((x) => x.args[0] === "stop"),
    false,
  );
});

for (const running of [true, false]) {
  test(`occupied global headless name (${running ? "running" : "stopped"}) blocks before any next operation`, (t) => {
    const f = fixture(t);
    writeFileSync(
      path.join(f.stack, "oneshot.json"),
      JSON.stringify({
        State: { Running: running },
        Config: {
          Labels: {
            "org.schoolorbit.rollout.release": sha,
            "org.schoolorbit.rollout.image": digest,
            "org.schoolorbit.rollout.operation": "attest-admin",
          },
        },
      }),
    );
    const result = f.run("preflight");
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /headless_operation_exists/);
    assert.match(result.stderr, /"name":"schoolorbit-school-rollout"/);
    assert.match(
      result.stderr,
      /"release":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/,
    );
    assert.match(result.stderr, /"operation":"attest-admin"/);
    assert.equal(existsSync(path.join(f.stack, "executed")), false);
    assert.equal(
      f.commands().some((x) => x.args[0] === "stop" || x.args[0] === "rm"),
      false,
    );
    assert.ok(existsSync(path.join(f.stack, "oneshot.json")));
    assert.doesNotMatch(result.stdout + result.stderr, /private-sentinel/);
  });
}
for (const operation of ["migrate-tenants", "attest-admin"]) {
  test(`orphaned ${operation} survives controller loss and globally refuses later releases/start`, (t) => {
    const f = fixture(t, {
      attest: operation === "attest-admin",
      orphanOperation: operation,
    });
    assert.equal(f.run("preflight").status, 0);
    f.maintenance();
    const first = f.cutover();
    assert.notEqual(first.status, 0);
    assert.match(first.stderr, /headless_operation_exists/);
    assert.ok(existsSync(path.join(f.stack, "oneshot.json")));
    const accepted = readFileSync(path.join(f.stack, "executed"), "utf8");
    const next = f.run("preflight", { RELEASE_SHA: "2".repeat(40) });
    assert.notEqual(next.status, 0);
    assert.match(next.stderr, /headless_operation_exists/);
    const same = f.cutover();
    assert.notEqual(same.status, 0);
    assert.match(same.stderr, /headless_operation_exists/);
    assert.equal(
      readFileSync(path.join(f.stack, "executed"), "utf8"),
      accepted,
    );
    assert.equal(existsSync(path.join(f.stack, "started")), false);
    assert.equal(
      f
        .commands()
        .some(
          (x) =>
            ["stop", "rm"].includes(x.args[0]) &&
            x.args.at(-1) === "schoolorbit-school-rollout",
        ),
      false,
    );
  });
}
test("unknown occupied owner remains blocked and hostile labels never enter diagnostics", (t) => {
  const f = fixture(t);
  writeFileSync(
    path.join(f.stack, "oneshot.json"),
    JSON.stringify({
      Config: {
        Labels: {
          "org.schoolorbit.rollout.release": "private-sentinel\nforged",
          "org.schoolorbit.rollout.operation": "private-sentinel",
        },
      },
    }),
  );
  const r = f.run("preflight");
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /"release":"unknown","operation":"unknown"/);
  assert.match(r.stderr, /headless_operation_exists/);
  assert.doesNotMatch(r.stdout + r.stderr, /private-sentinel|forged/);
  assert.equal(
    f.commands().some((x) => ["rm", "stop"].includes(x.args[0])),
    false,
  );
});
