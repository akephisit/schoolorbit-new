import { execFileSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import os from "node:os";

const component = process.argv[2] || process.env.COMPONENT;
if (!["frontend-admin", "frontend-school"].includes(component))
  throw new Error("Invalid frontend worker owner");
const directory = path.resolve(component);
const output = mkdtempSync(path.join(os.tmpdir(), "schoolorbit-worker-"));
const configPath = path.join(directory, ".wrangler", "pipeline-build.json");
mkdirSync(path.dirname(configPath), { recursive: true });
try {
  const config = JSON.parse(
    readFileSync(path.join(directory, "wrangler.json"), "utf8"),
  );
  delete config.build;
  config.main = path.resolve(directory, config.main);
  config.assets.directory = path.resolve(directory, config.assets.directory);
  writeFileSync(configPath, JSON.stringify(config));
  // Adapter output can import .svelte-kit/cloudflare-tmp. Resolve it once in
  // preparation; deploy runners consume self-contained code and existing assets.
  execFileSync(
    process.execPath,
    [
      path.join(directory, "node_modules/wrangler/bin/wrangler.js"),
      "deploy",
      "--dry-run",
      "--config",
      configPath,
      "--outdir",
      output,
    ],
    {
      cwd: directory,
      stdio: "inherit",
      env: { ...process.env, WRANGLER_SEND_METRICS: "false" },
    },
  );
  if (!existsSync(path.join(output, "index.js")))
    throw new Error("Wrangler did not produce the prepared Worker entry point");
  // Wrangler's README embeds the current time and its sourcemap embeds the
  // random output directory. Keep runtime output deterministic for bundle cache.
  rmSync(path.join(output, "README.md"), { force: true });
  rmSync(path.join(directory, "build/README.md"), { force: true });
  const mapPath = path.join(output, "index.js.map");
  if (existsSync(mapPath)) {
    const map = JSON.parse(readFileSync(mapPath, "utf8"));
    const sourceRoot = path.resolve(output, map.sourceRoot || ".");
    map.sources = map.sources.map(source =>
      path.relative(directory, path.resolve(sourceRoot, source)),
    );
    map.sourceRoot = "..";
    writeFileSync(mapPath, JSON.stringify(map));
  }
  cpSync(output, path.join(directory, "build"), { recursive: true });
  // Rehearse the deploy runner's filesystem: only the packaged build survives.
  // This must pass without adapter temporaries, source or node_modules nearby.
  const isolated = path.join(output, "artifact");
  mkdirSync(isolated);
  cpSync(path.join(directory, "build"), path.join(isolated, "build"), {
    recursive: true,
  });
  config.main = "./build/index.js";
  config.assets.directory = "./build/client";
  const isolatedConfig = path.join(isolated, "wrangler.json");
  writeFileSync(isolatedConfig, JSON.stringify(config));
  execFileSync(
    process.execPath,
    [
      path.join(directory, "node_modules/wrangler/bin/wrangler.js"),
      "deploy",
      "--dry-run",
      "--config",
      isolatedConfig,
      "--outdir",
      path.join(output, "verified"),
    ],
    {
      cwd: isolated,
      stdio: "inherit",
      env: { ...process.env, WRANGLER_SEND_METRICS: "false" },
    },
  );
} finally {
  rmSync(configPath, { force: true });
  rmSync(output, { recursive: true, force: true });
}
