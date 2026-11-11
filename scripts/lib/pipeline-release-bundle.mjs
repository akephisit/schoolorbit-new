import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  appendFileSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
export function releaseBundleFiles(root, sha) {
  const bootstrap = "scripts/lib/pipeline-remote/install-release-bundle.sh";
  const source = sha
    ? execFileSync("git", ["show", `${sha}:${bootstrap}`], { cwd: root })
    : readFileSync(path.join(root, bootstrap));
  return JSON.parse(
    execFileSync("bash", [], {
      input: source,
      env: {
        ...process.env,
        BUNDLE_OPERATION: "describe",
        BUNDLE_INSTALL: "true",
      },
      encoding: "utf8",
    }),
  );
}
function tarEntry(name, bytes, mode) {
  const header = Buffer.alloc(512);
  const field = (offset, width, value) =>
    header.write(value, offset, width, "ascii");
  const octal = (offset, width, value) =>
    field(offset, width, value.toString(8).padStart(width - 1, "0") + "\0");
  if (Buffer.byteLength(name) > 100)
    throw new Error("Bundle path exceeds fixed TAR protocol");
  field(0, 100, name);
  octal(100, 8, mode);
  octal(108, 8, 0);
  octal(116, 8, 0);
  octal(124, 12, bytes.length);
  octal(136, 12, 0);
  field(148, 8, "        ");
  field(156, 1, "0");
  field(257, 6, "ustar\0");
  field(263, 2, "00");
  octal(
    148,
    8,
    header.reduce((sum, byte) => sum + byte, 0),
  );
  return Buffer.concat([
    header,
    bytes,
    Buffer.alloc((512 - (bytes.length % 512)) % 512),
  ]);
}
export function createReleaseBundle(root, sha) {
  if (!/^[0-9a-f]{40}$/.test(sha)) throw new Error("Invalid release SHA");
  const head = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: root,
    encoding: "utf8",
  }).trim();
  if (head !== sha) throw new Error("Bundle checkout differs from release SHA");
  const files = releaseBundleFiles(root, sha).map((name) => {
    const entry = execFileSync("git", ["ls-tree", sha, "--", name], {
      cwd: root,
      encoding: "utf8",
    });
    if (!/^100(?:644|755) blob [0-9a-f]{40}\t/.test(entry))
      throw new Error("Bundle input is not a tracked regular file");
    const bytes = execFileSync("git", ["show", `${sha}:${name}`], {
      cwd: root,
      maxBuffer: 2 * 1024 * 1024,
    });
    return { path: name, bytes, mode: name.endsWith(".sh") ? 0o555 : 0o444 };
  });
  const manifest = Buffer.from(
    JSON.stringify({
      schemaVersion: 1,
      releaseSha: sha,
      files: files.map(({ path: name, bytes, mode }) => ({
        path: name,
        mode,
        size: bytes.length,
        sha256: hash(bytes),
      })),
    }) + "\n",
  );
  const archive = Buffer.concat([
    ...files.map((file) => tarEntry(file.path, file.bytes, file.mode)),
    tarEntry("release-bundle.json", manifest, 0o444),
    Buffer.alloc(1024),
  ]);
  if (archive.length > 16 * 1024 * 1024)
    throw new Error("Release bundle exceeds transport bound");
  return { archive, digest: hash(archive) };
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const directory =
    process.env.BUNDLE_DIRECTORY ??
    path.join(process.env.RUNNER_TEMP, "release-bundle");
  if (process.argv[2] === "reconstruct") {
    const bundle = createReleaseBundle(process.cwd(), process.env.GITHUB_SHA);
    if (
      !/^[0-9a-f]{64}$/.test(process.env.RELEASE_BUNDLE_SHA256 ?? "") ||
      bundle.digest !== process.env.RELEASE_BUNDLE_SHA256
    )
      throw new Error("Reconstructed bundle differs from public identity");
    mkdirSync(directory, { recursive: true });
    writeFileSync(path.join(directory, "release-bundle.tar"), bundle.archive);
  } else if (process.argv[2] === "verify") {
    if (
      !/^[0-9a-f]{64}$/.test(process.env.RELEASE_BUNDLE_SHA256 ?? "") ||
      hash(readFileSync(path.join(directory, "release-bundle.tar"))) !==
        process.env.RELEASE_BUNDLE_SHA256
    )
      throw new Error("Release bundle transport digest mismatch");
  } else if (!process.argv[2]) {
    const bundle = createReleaseBundle(process.cwd(), process.env.GITHUB_SHA);
    mkdirSync(directory, { recursive: true });
    writeFileSync(path.join(directory, "release-bundle.tar"), bundle.archive);
    appendFileSync(process.env.GITHUB_OUTPUT, `digest=${bundle.digest}\n`);
  } else throw new Error("Unknown bundle operation");
}
