import { createHash } from "node:crypto";
import {
  appendFileSync,
  cpSync,
  existsSync,
  linkSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const source = fileURLToPath(import.meta.url);
const applications = {
  "frontend-school": ["wrangler", "playwright", "glob"],
  "frontend-admin": ["wrangler"],
};

function resolvePackage(packages, owner, name) {
  if (!/^(?:@[a-z0-9._-]+\/)?[a-z0-9._-]+$/i.test(name))
    throw new Error("Invalid locked dependency name");
  let directory = owner;
  while (directory && directory !== ".") {
    const candidate = `${directory}/node_modules/${name}`;
    if (packages[candidate]) return candidate;
    directory = path.posix.dirname(directory);
  }
  const root = `node_modules/${name}`;
  return packages[root] ? root : undefined;
}

export function deploymentLock(lock, component) {
  const roots = applications[component];
  if (!roots || lock.lockfileVersion !== 3 || !lock.packages?.[""])
    throw new Error("Unsupported application lockfile");
  // Optionality belongs to edges reachable from these deployment roots. A node
  // required elsewhere in the full app can be optional in this smaller closure.
  const selected = new Map();
  const visit = (owner, name, optional = false) => {
    const location = resolvePackage(lock.packages, owner, name);
    if (!location) {
      if (optional) return;
      throw new Error(`Missing locked deployment dependency: ${name}`);
    }
    const required = !optional;
    if (selected.get(location) === true || selected.get(location) === required)
      return;
    const item = lock.packages[location];
    if (
      item.link ||
      !item.version ||
      !item.integrity ||
      !/^https:\/\//.test(item.resolved || "")
    )
      throw new Error(`Nonportable deployment dependency: ${name}`);
    selected.set(location, required);
    for (const [child] of Object.entries(item.dependencies || {}))
      visit(
        location,
        child,
        optional || Object.hasOwn(item.optionalDependencies || {}, child),
      );
    for (const [child] of Object.entries(item.optionalDependencies || {}))
      visit(location, child, true);
    for (const [child] of Object.entries(item.peerDependencies || {}))
      visit(
        location,
        child,
        optional || item.peerDependenciesMeta?.[child]?.optional === true,
      );
  };
  const dependencies = {};
  for (const name of roots) {
    visit("", name);
    dependencies[name] = lock.packages[`node_modules/${name}`].version;
  }
  const manifest = {
    name: `schoolorbit-${component}-deployment`,
    version: "1.0.0",
    private: true,
    dependencies,
  };
  const packages = {
    "": { name: manifest.name, version: manifest.version, dependencies },
  };
  for (const location of [...selected.keys()].sort()) {
    const item = lock.packages[location];
    const optional = !selected.get(location);
    packages[location] =
      Boolean(item.optional) === optional ? item : { ...item, optional };
  }
  return {
    manifest,
    lock: {
      name: manifest.name,
      version: manifest.version,
      lockfileVersion: 3,
      requires: true,
      packages,
    },
  };
}

function context(component, prefix) {
  if (!applications[component] || !prefix)
    throw new Error("Use a supported frontend and an owned temporary prefix");
  const app = path.resolve(component);
  const directory = path.resolve(prefix, component);
  if (directory === app || directory.startsWith(app + path.sep))
    throw new Error(
      "Deployment tooling must be outside the application checkout",
    );
  const subset = deploymentLock(
    JSON.parse(readFileSync(path.join(app, "package-lock.json"), "utf8")),
    component,
  );
  const image = (process.env.ImageOS || process.platform).replace(
    /[^a-z0-9_-]/gi,
    "_",
  );
  const digest = createHash("sha256")
    .update(JSON.stringify(subset))
    .update(readFileSync(source))
    .digest("hex");
  const identity = `deploy-tools-v1-${image}-${process.arch}-node${process.versions.node}-${digest}`;
  return {
    app,
    directory,
    subset,
    identity,
    marker: path.join(directory, "node_modules/.schoolorbit-deployment.json"),
  };
}

function supportedPlatform(item) {
  const matches = (values, current) =>
    !values ||
    (!values.includes(`!${current}`) &&
      (values.every((v) => v.startsWith("!")) || values.includes(current)));
  return matches(item.os, process.platform) && matches(item.cpu, process.arch);
}

function installed(c, base = c.directory) {
  return (
    Object.entries(c.subset.lock.packages).every(([location, item]) => {
      if (!location || !supportedPlatform(item)) return true;
      const directory = path.join(base, location);
      if (!existsSync(directory)) return item.optional === true;
      try {
        return (
          JSON.parse(readFileSync(path.join(directory, "package.json"), "utf8"))
            .version === item.version
        );
      } catch {
        return false;
      }
    }) &&
    applications[path.basename(c.app)].every((name) => {
      const item = c.subset.lock.packages[`node_modules/${name}`];
      const bins =
        typeof item.bin === "string"
          ? { [path.basename(name)]: item.bin }
          : item.bin || {};
      return (
        existsSync(path.join(base, "node_modules", name, "package.json")) &&
        Object.entries(bins).every(([bin, executable]) => {
          if (
            !/^[a-z0-9._-]+$/i.test(bin) ||
            path.isAbsolute(executable) ||
            executable.split("/").includes("..")
          )
            return false;
          return (
            existsSync(path.join(base, "node_modules", name, executable)) &&
            (base === c.app ||
              existsSync(path.join(base, "node_modules/.bin", bin)))
          );
        })
      );
    })
  );
}

function output(name, value) {
  if (/[\r\n]/.test(String(value)))
    throw new Error("Invalid deployment tooling output");
  if (process.env.GITHUB_OUTPUT)
    appendFileSync(process.env.GITHUB_OUTPUT, `${name}=${value}\n`);
  else console.log(`${name}=${value}`);
}

export function run(command, component, prefix) {
  const c = context(component, prefix);
  if (command === "prepare") {
    mkdirSync(c.directory, { recursive: true });
    writeFileSync(
      path.join(c.directory, "package.json"),
      JSON.stringify(c.subset.manifest),
    );
    writeFileSync(
      path.join(c.directory, "package-lock.json"),
      JSON.stringify(c.subset.lock),
    );
    output("key", c.identity);
    output("directory", c.directory);
    output("packages", Object.keys(c.subset.lock.packages).length - 1);
  } else if (command === "validate") {
    let valid = false;
    try {
      valid =
        JSON.parse(readFileSync(c.marker, "utf8")).identity === c.identity &&
        installed(c);
    } catch {}
    output("valid", valid);
  } else if (command === "seed") {
    if (!installed(c, c.app)) {
      output("seeded", false);
      return;
    }
    rmSync(path.join(c.directory, "node_modules"), {
      recursive: true,
      force: true,
    });
    mkdirSync(path.join(c.directory, "node_modules"), { recursive: true });
    const copiedLinks = new Map();
    // Copy only the locked deployment closure, never workspace links or app secrets.
    for (const [location, item] of Object.entries(c.subset.lock.packages)) {
      if (
        !location ||
        !supportedPlatform(item) ||
        !existsSync(path.join(c.app, location))
      )
        continue;
      const from = path.join(c.app, location);
      cpSync(from, path.join(c.directory, location), {
        recursive: true,
        dereference: true,
        filter: (file, target) => {
          if (
            path.relative(from, file).split(path.sep).includes("node_modules")
          )
            return false;
          const stat = statSync(file);
          if (stat.isFile() && stat.nlink > 1) {
            const key = `${stat.dev}:${stat.ino}`;
            if (copiedLinks.has(key)) {
              mkdirSync(path.dirname(target), { recursive: true });
              linkSync(copiedLinks.get(key), target);
              return false;
            }
            copiedLinks.set(key, target);
          }
          return true;
        },
      });
    }
    for (const [location, item] of Object.entries(c.subset.lock.packages)) {
      if (!location || !existsSync(path.join(c.directory, location))) continue;
      const binDirectory = path.join(
        c.directory,
        location.slice(0, location.lastIndexOf("node_modules/") + 12),
        ".bin",
      );
      mkdirSync(binDirectory, { recursive: true });
      const bins =
        typeof item.bin === "string"
          ? { [path.basename(location)]: item.bin }
          : item.bin || {};
      for (const [name, executable] of Object.entries(bins)) {
        if (
          !/^[a-z0-9._-]+$/i.test(name) ||
          path.isAbsolute(executable) ||
          executable.split("/").includes("..")
        )
          throw new Error("Invalid locked deployment executable");
        const target = path.join(binDirectory, name);
        if (!existsSync(target))
          symlinkSync(
            path.relative(
              binDirectory,
              path.join(c.directory, location, executable),
            ),
            target,
          );
      }
    }
    output("seeded", true);
  } else if (command === "mark") {
    if (!installed(c))
      throw new Error(
        "Deployment tooling installation does not match its application lock",
      );
    writeFileSync(c.marker, JSON.stringify({ identity: c.identity }));
  } else if (command === "link") {
    if (!installed(c)) throw new Error("Missing locked deployment tooling");
    const target = path.join(c.app, "node_modules");
    if (existsSync(target)) {
      if (
        realpathSync(target) !==
        realpathSync(path.join(c.directory, "node_modules"))
      )
        throw new Error("Refusing to replace another dependency owner");
    } else symlinkSync(path.join(c.directory, "node_modules"), target, "dir");
  } else throw new Error("Unknown deployment tooling command");
}

if (process.argv[1] && path.resolve(process.argv[1]) === source) {
  try {
    run(...process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
