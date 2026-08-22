import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const IDENTIFIER = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const GLOB_SYNTAX = /[*?[\]{}!]/;
const VALID_RISKS = new Set(["normal", "high"]);
const VALID_AGENT_PROFILES = new Set([
  "schoolorbit_implementer",
  "schoolorbit_high_risk_implementer",
]);
const HIGH_RISK_PROFILE = "schoolorbit_high_risk_implementer";

export const PROTECTED_PATH_POLICIES = Object.freeze([
  {
    resource: "migration-timeline",
    risk: "high",
    prefixes: [
      "backend-admin/migrations/",
      "backend-school/migrations/",
      "backend-school/migrations_legacy/",
    ],
  },
  {
    resource: "permission-contract",
    risk: "high",
    exact: [
      "contracts/permissions.json",
      "contracts/permissions.lock.json",
      "contracts/permissions.schema.json",
      "backend-school/src/permissions/registry.rs",
      "backend-school/src/permissions/registry_generated.rs",
      "frontend-school/src/lib/permissions/registry.ts",
      "frontend-school/src/lib/permissions/registry.generated.ts",
    ],
  },
  {
    resource: "api-contract",
    risk: "high",
    exact: ["backend-school/src/api_contract.rs"],
    prefixes: ["contracts/openapi/", "frontend-school/src/lib/api/generated/"],
  },
  {
    resource: "dependency-lockfile",
    basenames: ["Cargo.lock", "package-lock.json"],
  },
  {
    resource: "route-registry",
    exact: ["backend-school/src/modules/system/handlers/register_routes.rs"],
  },
  {
    resource: "deployment-owner",
    risk: "high",
    exact: ["podman-compose.yml", "scripts/schoolorbit-installer"],
    prefixes: [
      ".github/workflows/",
      "nginx-configs/",
      "scripts/lib/schoolorbit-installer/",
    ],
  },
  {
    resource: "security-identity",
    risk: "high",
    exact: [
      "backend-admin/src/handlers/auth.rs",
      "backend-admin/src/middleware/auth.rs",
      "backend-admin/src/services/auth_service.rs",
      "backend-school/src/middleware/session.rs",
      "backend-school/src/modules/auth.rs",
      "backend-school/src/modules/consent.rs",
      "frontend-school/src/lib/api/session-security.ts",
    ],
    prefixes: [
      "backend-admin/src/auth/",
      "backend-school/src/modules/auth/",
      "backend-school/src/modules/consent/",
      "frontend-school/src/lib/components/consent/",
      "frontend-school/src/lib/features/session-security/",
      "frontend-school/src/lib/realtime/",
      "frontend-school/src/routes/privacy-policy/",
    ],
    fragments: [
      "/auth",
      "/permission",
      "/consent",
      "/realtime",
      "/websocket",
      "/timetable-socket",
      "/session-security",
      "/national_id",
      "/national-id",
      "/pdpa/",
      "/privacy",
    ],
  },
]);

export function normalizeOwnedPath(value) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("must be a non-empty path");
  }
  if (value.includes("\\")) throw new Error("must use POSIX separators");
  if (value.includes("\0")) throw new Error("must not contain NUL");
  if (path.posix.isAbsolute(value))
    throw new Error("must be repository-relative");
  if (value.split("/").includes(".."))
    throw new Error("must not contain parent traversal");
  if (GLOB_SYNTAX.test(value)) throw new Error("must not contain glob syntax");
  const normalized = path.posix.normalize(value);
  if (normalized === ".") throw new Error("must not own the repository root");
  if (normalized !== value) throw new Error("must be lexically normalized");
  return normalized;
}

export function pathsOverlap(left, right) {
  return (
    left === right ||
    left.startsWith(`${right}/`) ||
    right.startsWith(`${left}/`)
  );
}

function matchesPathPolicy(value, policy) {
  return Boolean(
    policy.exact?.some((owned) => pathsOverlap(value, owned)) ||
    policy.prefixes?.some((prefix) =>
      pathsOverlap(value, prefix.slice(0, -1)),
    ) ||
    policy.basenames?.includes(path.posix.basename(value)) ||
    policy.fragments?.some((fragment) => value.includes(fragment)),
  );
}

export function classifyOwnedPath(value) {
  return PROTECTED_PATH_POLICIES.filter((policy) =>
    matchesPathPolicy(value, policy),
  );
}

function isObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function displayValue(value) {
  return JSON.stringify(value) ?? String(value);
}

function result(errors, taskCount, waveCount) {
  const uniqueErrors = [...new Set(errors)];
  return {
    valid: uniqueErrors.length === 0,
    errors: uniqueErrors,
    taskCount,
    waveCount,
  };
}

function taskLabel(task, index) {
  return typeof task?.id === "string" && IDENTIFIER.test(task.id)
    ? `task ${task.id}`
    : `task at index ${index}`;
}

function validIdentifierArray(value, field, label, errors) {
  if (!Array.isArray(value)) {
    errors.push(`${label} ${field} must be an array`);
    return [];
  }

  const valid = [];
  for (const item of value) {
    if (typeof item !== "string" || !IDENTIFIER.test(item)) {
      errors.push(`${label} has invalid ${field} value ${displayValue(item)}`);
      continue;
    }
    valid.push(item);
  }
  return valid;
}

export function validateWorkGraph(graph) {
  const errors = [];
  if (!isObject(graph)) {
    return result(["work graph must be an object"], 0, 0);
  }

  if (graph.version !== 1) errors.push("work graph version must equal 1");
  if (!Array.isArray(graph.tasks)) {
    errors.push("work graph tasks must be an array");
    return result(errors, 0, 0);
  }
  if (graph.tasks.length === 0) {
    errors.push("work graph requires at least one task");
    return result(errors, 0, 0);
  }

  const records = [];
  const tasksById = new Map();

  for (const [index, task] of graph.tasks.entries()) {
    if (!isObject(task)) {
      errors.push(`task at index ${index} must be an object`);
      continue;
    }

    const label = taskLabel(task, index);
    const validId = typeof task.id === "string" && IDENTIFIER.test(task.id);
    if (!validId) {
      errors.push(`${label} id must use lowercase kebab-case`);
    } else if (tasksById.has(task.id)) {
      errors.push(`duplicate task id ${task.id}`);
    }

    const validWave = Number.isInteger(task.wave) && task.wave > 0;
    if (!validWave) errors.push(`${label} wave must be a positive integer`);

    const dependencies = validIdentifierArray(
      task.dependencies,
      "dependency",
      label,
      errors,
    );
    const protectedResources = validIdentifierArray(
      task.protectedResources,
      "protected resource",
      label,
      errors,
    );

    const normalizedPaths = [];
    if (!Array.isArray(task.ownedPaths)) {
      errors.push(`${label} ownedPaths must be an array`);
    } else {
      if (task.ownedPaths.length === 0) {
        errors.push(`${label} requires at least one owned path`);
      }
      for (const ownedPath of task.ownedPaths) {
        try {
          normalizedPaths.push(normalizeOwnedPath(ownedPath));
        } catch (error) {
          errors.push(
            `${label} owned path ${displayValue(ownedPath)} ${error.message}`,
          );
        }
      }
    }

    if (!VALID_RISKS.has(task.risk)) {
      errors.push(`${label} risk must be normal or high`);
    }
    if (!VALID_AGENT_PROFILES.has(task.agentProfile)) {
      errors.push(
        `${label} has invalid agent profile ${displayValue(task.agentProfile)}`,
      );
    }

    if (
      !Array.isArray(task.verification) ||
      !task.verification.some(
        (command) => typeof command === "string" && command.trim().length > 0,
      )
    ) {
      errors.push(`${label} requires at least one verification command`);
    } else {
      for (const command of task.verification) {
        if (typeof command !== "string" || command.trim().length === 0) {
          errors.push(`${label} has an invalid verification command`);
        }
      }
    }

    const record = {
      index,
      id: validId ? task.id : null,
      label,
      wave: validWave ? task.wave : null,
      dependencies,
      normalizedPaths,
      protectedResources,
      risk: task.risk,
      agentProfile: task.agentProfile,
      inferredHighRisk: false,
    };
    records.push(record);
    if (record.id !== null && !tasksById.has(record.id))
      tasksById.set(record.id, record);
  }

  for (const record of records) {
    const resourceSet = new Set(record.protectedResources);
    for (const ownedPath of record.normalizedPaths) {
      if (
        ownedPath === "backend-school/migrations_legacy" ||
        ownedPath.startsWith("backend-school/migrations_legacy/")
      ) {
        errors.push(`${record.label} must not own a legacy migration path`);
      }

      for (const policy of classifyOwnedPath(ownedPath)) {
        if (!resourceSet.has(policy.resource)) {
          errors.push(
            `${record.label} requires protected resource ${policy.resource}`,
          );
        }
        if (policy.risk === "high") record.inferredHighRisk = true;
      }
    }

    if (record.inferredHighRisk && record.risk === "normal") {
      errors.push(`${record.label} has a high-risk owned path`);
    }
    if (
      (record.risk === "high" || record.inferredHighRisk) &&
      record.agentProfile !== HIGH_RISK_PROFILE
    ) {
      errors.push(`${record.label} requires ${HIGH_RISK_PROFILE}`);
    }
  }

  for (const record of records) {
    for (const dependencyId of record.dependencies) {
      const dependency = tasksById.get(dependencyId);
      if (!dependency) {
        errors.push(`${record.label} has unknown dependency ${dependencyId}`);
        continue;
      }
      if (
        record.wave !== null &&
        dependency.wave !== null &&
        dependency.wave >= record.wave
      ) {
        errors.push(
          `${record.label} dependency ${dependencyId} must run in an earlier wave`,
        );
      }
    }
  }

  const waves = new Map();
  for (const record of records) {
    if (record.wave === null) continue;
    const wave = waves.get(record.wave) ?? [];
    wave.push(record);
    waves.set(record.wave, wave);
  }

  for (const waveNumber of [...waves.keys()].sort(
    (left, right) => left - right,
  )) {
    const wave = waves
      .get(waveNumber)
      .toSorted(
        (left, right) =>
          (left.id ?? left.label).localeCompare(right.id ?? right.label) ||
          left.index - right.index,
      );
    if (wave.length > 3) {
      errors.push(
        `wave ${waveNumber} has ${wave.length} tasks and exceeds the concurrency limit of 3`,
      );
    }

    for (let leftIndex = 0; leftIndex < wave.length; leftIndex += 1) {
      for (
        let rightIndex = leftIndex + 1;
        rightIndex < wave.length;
        rightIndex += 1
      ) {
        const left = wave[leftIndex];
        const right = wave[rightIndex];
        for (const leftPath of left.normalizedPaths.toSorted()) {
          for (const rightPath of right.normalizedPaths.toSorted()) {
            if (pathsOverlap(leftPath, rightPath)) {
              errors.push(
                `wave ${waveNumber} tasks ${left.label} and ${right.label} have overlapping owned paths ${displayValue(leftPath)} and ${displayValue(rightPath)}`,
              );
            }
          }
        }

        const rightResources = new Set(right.protectedResources);
        for (const resource of [...new Set(left.protectedResources)].sort()) {
          if (rightResources.has(resource)) {
            errors.push(
              `wave ${waveNumber} tasks ${left.label} and ${right.label} have shared protected resource ${resource}`,
            );
          }
        }
      }
    }
  }

  return result(errors, graph.tasks.length, waves.size);
}

function printErrors(errors) {
  for (const error of errors) console.error(`- ${error}`);
}

export async function main(argv = process.argv.slice(2)) {
  if (argv.length !== 1) {
    printErrors(["usage: node validate-work-graph.mjs <graph.json>"]);
    process.exitCode = 1;
    return;
  }

  let source;
  try {
    source = await readFile(argv[0], "utf8");
  } catch {
    printErrors(["unable to read work graph file"]);
    process.exitCode = 1;
    return;
  }

  let graph;
  try {
    graph = JSON.parse(source);
  } catch {
    printErrors(["invalid work graph JSON"]);
    process.exitCode = 1;
    return;
  }

  const validation = validateWorkGraph(graph);
  if (!validation.valid) {
    printErrors(validation.errors);
    process.exitCode = 1;
    return;
  }

  console.log(
    `Valid work graph: ${validation.taskCount} tasks across ${validation.waveCount} waves`,
  );
  process.exitCode = 0;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  await main();
}
