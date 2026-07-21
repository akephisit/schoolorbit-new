import assert from "node:assert/strict";
import {
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  commitGeneratedOutputs,
  generateApiContracts,
} from "../generate-api-contracts.mjs";

const document = {
  openapi: "3.1.0",
  info: { title: "School API", version: "0.1.0" },
  paths: {},
  components: { schemas: {} },
};

const exportSchoolDocument = async () => document;
const generateSchoolTypes = async () =>
  "// generated\nexport interface paths {}\n";

async function fixture(t) {
  const root = await mkdtemp(
    path.join(os.tmpdir(), "schoolorbit-api-contract-"),
  );
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(path.join(root, "contracts/openapi"), { recursive: true });
  await mkdir(path.join(root, "frontend-school/src/lib/api/generated"), {
    recursive: true,
  });
  return root;
}

test("generation is deterministic", async (t) => {
  const repositoryRoot = await fixture(t);
  const options = { repositoryRoot, exportSchoolDocument, generateSchoolTypes };

  await generateApiContracts(options);
  const openApiPath = path.join(
    repositoryRoot,
    "contracts/openapi/school-api.json",
  );
  const typesPath = path.join(
    repositoryRoot,
    "frontend-school/src/lib/api/generated/school-api.ts",
  );
  const first = [
    await readFile(openApiPath, "utf8"),
    await readFile(typesPath, "utf8"),
  ];

  await generateApiContracts(options);

  assert.deepEqual(
    [await readFile(openApiPath, "utf8"), await readFile(typesPath, "utf8")],
    first,
  );
  assert.ok(first[0].endsWith("\n"));
  assert.ok(first[1].startsWith("// @generated"));
});

test("check mode reports stale output without writing", async (t) => {
  const repositoryRoot = await fixture(t);
  const openApiPath = path.join(
    repositoryRoot,
    "contracts/openapi/school-api.json",
  );
  const typesPath = path.join(
    repositoryRoot,
    "frontend-school/src/lib/api/generated/school-api.ts",
  );
  await writeFile(openApiPath, "old openapi\n");
  await writeFile(typesPath, "old types\n");

  await assert.rejects(
    generateApiContracts({
      repositoryRoot,
      check: true,
      exportSchoolDocument,
      generateSchoolTypes,
    }),
    /stale generated API contract artifacts/,
  );
  assert.equal(await readFile(openApiPath, "utf8"), "old openapi\n");
  assert.equal(await readFile(typesPath, "utf8"), "old types\n");
});

test("type-generation failure leaves both tracked outputs untouched", async (t) => {
  const repositoryRoot = await fixture(t);
  const openApiPath = path.join(
    repositoryRoot,
    "contracts/openapi/school-api.json",
  );
  const typesPath = path.join(
    repositoryRoot,
    "frontend-school/src/lib/api/generated/school-api.ts",
  );
  await writeFile(openApiPath, "old openapi\n");
  await writeFile(typesPath, "old types\n");

  await assert.rejects(
    generateApiContracts({
      repositoryRoot,
      exportSchoolDocument,
      generateSchoolTypes: async () => {
        throw new Error("type generation failed");
      },
    }),
    /type generation failed/,
  );
  assert.equal(await readFile(openApiPath, "utf8"), "old openapi\n");
  assert.equal(await readFile(typesPath, "utf8"), "old types\n");
});

test("replacement failure rolls back every tracked output", async (t) => {
  const repositoryRoot = await fixture(t);
  const openApiPath = path.join(
    repositoryRoot,
    "contracts/openapi/school-api.json",
  );
  const typesPath = path.join(
    repositoryRoot,
    "frontend-school/src/lib/api/generated/school-api.ts",
  );
  await writeFile(openApiPath, "old openapi\n");
  await writeFile(typesPath, "old types\n");

  let replacements = 0;
  const failSecondReplacement = async (source, destination) => {
    if (source.endsWith(".tmp")) {
      replacements += 1;
      if (replacements === 2) throw new Error("second replacement failed");
    }
    await rename(source, destination);
  };

  await assert.rejects(
    commitGeneratedOutputs(
      [
        [openApiPath, "new openapi\n"],
        [typesPath, "new types\n"],
      ],
      { renameFile: failSecondReplacement },
    ),
    /second replacement failed/,
  );

  assert.equal(await readFile(openApiPath, "utf8"), "old openapi\n");
  assert.equal(await readFile(typesPath, "utf8"), "old types\n");
  for (const directory of [
    path.dirname(openApiPath),
    path.dirname(typesPath),
  ]) {
    assert.deepEqual(
      (await readdir(directory)).filter((name) =>
        name.includes(".api-contract-"),
      ),
      [],
    );
  }
});
