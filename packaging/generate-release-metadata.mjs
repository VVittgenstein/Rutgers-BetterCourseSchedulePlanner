#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const GENERATOR_VERSION = "1";
const PACKAGING_ROOT = path.dirname(fileURLToPath(import.meta.url));
const GENERATED_FILES = new Set([
  "BUILD-PROVENANCE.json",
  "MANIFEST.json",
  "SBOM.cdx.json",
  "SHA256SUMS",
  "THIRD-PARTY-NOTICES.txt",
]);
const PACKAGE_BINARIES = new Map([
  ["WINDOWS_LOCAL_RELEASE_ARCHIVE", "RBCSP.exe"],
  ["LINUX_PUBLIC_DEPLOYMENT_PACKAGE", "bin/bcsp-server"],
]);
const OPTION_NAMES = new Set([
  "source-root",
  "stage-root",
  "package-id",
  "target",
  "binary",
  "source-commit",
  "source-date-epoch",
  "rustc-version",
  "cargo-version",
  "node-version",
  "npm-version",
  "cargo-cyclonedx-version",
  "cargo-about-version",
  "raw-cargo-sbom",
  "raw-cargo-about",
]);

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function compareOrdinal(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function uniqueSorted(values) {
  return [...new Set(values)].sort(compareOrdinal);
}

function parseArguments(argv) {
  if (argv.length === 1 && (argv[0] === "--help" || argv[0] === "-h")) {
    process.stdout.write(
      "Usage: node packaging/generate-release-metadata.mjs " +
        [...OPTION_NAMES].map((name) => `--${name} <value>`).join(" ") +
        "\n",
    );
    process.exit(0);
  }

  const options = Object.create(null);
  for (let index = 0; index < argv.length; index += 2) {
    const token = argv[index];
    const value = argv[index + 1];
    assert(token?.startsWith("--"), `expected an option at argument ${index + 1}`);
    const name = token.slice(2);
    assert(OPTION_NAMES.has(name), `unknown option: --${name}`);
    assert(value !== undefined && !value.startsWith("--"), `missing value for --${name}`);
    assert(options[name] === undefined, `duplicate option: --${name}`);
    options[name] = value;
  }
  for (const name of OPTION_NAMES) {
    assert(options[name] !== undefined, `missing required option: --${name}`);
  }
  return options;
}

function readJson(file, label = file) {
  let value;
  try {
    value = JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    fail(`${label} is not readable JSON: ${error.message}`);
  }
  assert(isObject(value), `${label} must contain a JSON object`);
  return value;
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function writeJson(file, value) {
  fs.writeFileSync(file, stableJson(value), { encoding: "utf8" });
}

function sha256Buffer(value) {
  return crypto.createHash("sha256").update(value).digest("hex");
}

function sha256File(file) {
  return sha256Buffer(fs.readFileSync(file));
}

function normalizeText(value) {
  assert(typeof value === "string", "expected text input");
  const normalized = value
    .replace(/^\uFEFF/, "")
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((line) => line.replace(/[\t ]+$/u, ""))
    .join("\n")
    .trimEnd();
  return `${normalized}\n`;
}

function toSlash(value) {
  return value.split(path.sep).join("/");
}

function validateRelativePath(value, label) {
  assert(typeof value === "string" && value.length > 0, `${label} must be a non-empty string`);
  assert(!value.includes("\\"), `${label} must use forward slashes: ${value}`);
  assert(!value.startsWith("/"), `${label} must be relative: ${value}`);
  assert(path.posix.normalize(value) === value, `${label} is not canonical: ${value}`);
  assert(!value.split("/").includes(".."), `${label} escapes its root: ${value}`);
  return value;
}

function pathInside(root, candidate, label) {
  const relative = path.relative(root, candidate);
  assert(relative !== "" && !relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative), `${label} must be inside ${root}`);
  return toSlash(relative);
}

function scanTree(root) {
  const files = [];
  const directories = [];

  function visit(absoluteDirectory, relativeDirectory) {
    const entries = fs
      .readdirSync(absoluteDirectory, { withFileTypes: true })
      .sort((left, right) => compareOrdinal(left.name, right.name));
    for (const entry of entries) {
      const absolute = path.join(absoluteDirectory, entry.name);
      const relative = relativeDirectory ? `${relativeDirectory}/${entry.name}` : entry.name;
      const stat = fs.lstatSync(absolute);
      assert(!stat.isSymbolicLink(), `release stage contains a symbolic link: ${relative}`);
      if (stat.isDirectory()) {
        directories.push(relative);
        visit(absolute, relative);
      } else if (stat.isFile()) {
        files.push(relative);
      } else {
        fail(`release stage contains a special file: ${relative}`);
      }
    }
  }

  visit(root, "");
  return {
    files: files.sort(compareOrdinal),
    directories: directories.sort(compareOrdinal),
  };
}

function expectedDirectories(allowlist) {
  const result = new Set();
  for (const file of allowlist) {
    const segments = file.split("/");
    for (let index = 1; index < segments.length; index += 1) {
      result.add(segments.slice(0, index).join("/"));
    }
  }
  return [...result].sort(compareOrdinal);
}

function assertSameSequence(actual, expected, label) {
  assert(actual.length === expected.length, `${label} count mismatch: expected ${expected.length}, got ${actual.length}`);
  for (let index = 0; index < expected.length; index += 1) {
    assert(actual[index] === expected[index], `${label} mismatch at ${index}: expected ${expected[index]}, got ${actual[index]}`);
  }
}

function assertStageShape(stageRoot, allowlist, requireGenerated) {
  const scan = scanTree(stageRoot);
  const allowed = new Set(allowlist);
  for (const file of scan.files) {
    assert(allowed.has(file), `release stage contains a file outside the package allowlist: ${file}`);
  }
  const required = allowlist.filter((file) => requireGenerated || !GENERATED_FILES.has(file));
  for (const file of required) {
    assert(scan.files.includes(file), `release stage is missing required file: ${file}`);
  }
  assertSameSequence(scan.directories, expectedDirectories(allowlist), "release stage directories");
  if (requireGenerated) assertSameSequence(scan.files, [...allowlist].sort(compareOrdinal), "release stage files");
  return scan;
}

function canonicalCargoPurl(component) {
  if (typeof component.purl === "string" && component.purl.startsWith("pkg:cargo/")) {
    return component.purl.split(/[?#]/u, 1)[0];
  }
  assert(typeof component.name === "string" && component.name.length > 0, "Cargo SBOM component is missing name");
  assert(typeof component.version === "string" && component.version.length > 0, `Cargo SBOM component ${component.name} is missing version`);
  return `pkg:cargo/${encodeURIComponent(component.name)}@${encodeURIComponent(component.version)}`;
}

function normalizeHashes(rawHashes, label) {
  if (rawHashes === undefined) return undefined;
  assert(Array.isArray(rawHashes), `${label} hashes must be an array`);
  const hashes = rawHashes.map((hash) => {
    assert(isObject(hash) && typeof hash.alg === "string" && typeof hash.content === "string", `${label} has an invalid hash`);
    assert(/^[0-9a-f]+$/iu.test(hash.content), `${label} has a non-hex hash`);
    return { alg: hash.alg.toUpperCase(), content: hash.content.toLowerCase() };
  });
  hashes.sort((left, right) => compareOrdinal(`${left.alg}:${left.content}`, `${right.alg}:${right.content}`));
  return hashes.length > 0 ? hashes : undefined;
}

function normalizeLicenses(rawLicenses, label) {
  assert(Array.isArray(rawLicenses) && rawLicenses.length > 0, `${label} is missing license choices`);
  const choices = [];
  for (const choice of rawLicenses) {
    assert(isObject(choice), `${label} has an invalid license choice`);
    if (typeof choice.expression === "string" && choice.expression.trim()) {
      choices.push({ expression: choice.expression.trim() });
      continue;
    }
    if (isObject(choice.license)) {
      if (typeof choice.license.id === "string" && choice.license.id.trim()) {
        choices.push({ license: { id: choice.license.id.trim() } });
        continue;
      }
      if (typeof choice.license.name === "string" && choice.license.name.trim()) {
        choices.push({ license: { name: choice.license.name.trim() } });
        continue;
      }
    }
    fail(`${label} has an unsupported license choice`);
  }
  const deduplicated = new Map(choices.map((choice) => [JSON.stringify(choice), choice]));
  return [...deduplicated.values()].sort((left, right) => compareOrdinal(JSON.stringify(left), JSON.stringify(right)));
}

function licenseLabel(licenses) {
  return licenses
    .map((choice) => choice.expression ?? choice.license?.id ?? choice.license?.name)
    .filter(Boolean)
    .join("; ");
}

function normalizeExternalReferences(rawReferences) {
  if (!Array.isArray(rawReferences)) return undefined;
  const references = rawReferences
    .filter(
      (reference) =>
        isObject(reference) &&
        typeof reference.type === "string" &&
        typeof reference.url === "string" &&
        /^https?:\/\//iu.test(reference.url),
    )
    .map((reference) => ({ type: reference.type, url: reference.url }));
  const deduplicated = new Map(references.map((reference) => [`${reference.type}\u0000${reference.url}`, reference]));
  const result = [...deduplicated.values()].sort((left, right) => compareOrdinal(`${left.type}:${left.url}`, `${right.type}:${right.url}`));
  return result.length > 0 ? result : undefined;
}

function cargoAboutCrateInventory(raw) {
  assert(Array.isArray(raw.crates), "cargo-about JSON is missing crates");
  const inventory = new Set();
  for (const entry of raw.crates) {
    const krate = entry?.package;
    assert(isObject(krate) && typeof krate.name === "string" && typeof krate.version === "string", "cargo-about JSON has an invalid crate entry");
    inventory.add(`${krate.name}@${krate.version}`);
  }
  return inventory;
}

function normalizeCargoSbom(raw, expectedRootName, releaseVersion, cargoAboutInventory) {
  assert(raw.bomFormat === "CycloneDX" && raw.specVersion === "1.5", "raw cargo-cyclonedx input must be CycloneDX 1.5");
  assert(isObject(raw.metadata) && isObject(raw.metadata.component), "raw Cargo SBOM is missing metadata.component");
  assert(Array.isArray(raw.components) && Array.isArray(raw.dependencies), "raw Cargo SBOM is missing components or dependencies");

  const rawRoot = raw.metadata.component;
  assert(rawRoot.name === expectedRootName, `raw Cargo SBOM root mismatch: expected ${expectedRootName}, got ${rawRoot.name}`);
  assert(rawRoot.version === releaseVersion, `raw Cargo SBOM version mismatch: expected ${releaseVersion}, got ${rawRoot.version}`);
  assert(cargoAboutInventory.has(`${rawRoot.name}@${rawRoot.version}`), "cargo-about inventory does not contain the Cargo root component");

  const rawRefToCanonical = new Map();
  const componentsByRef = new Map();
  const externalRust = [];

  function normalizeComponent(component, root = false) {
    assert(isObject(component), "raw Cargo SBOM contains an invalid component");
    assert(typeof component["bom-ref"] === "string", `Cargo component ${component.name ?? "<unknown>"} is missing bom-ref`);
    assert(typeof component.name === "string" && typeof component.version === "string", "Cargo component is missing name or version");
    const purl = canonicalCargoPurl(component);
    const licenses = normalizeLicenses(component.licenses, `Cargo component ${component.name}@${component.version}`);
    const normalized = {
      type: root ? "application" : component.type || "library",
      "bom-ref": purl,
      name: component.name,
      version: component.version,
      scope: ["required", "optional", "excluded"].includes(component.scope) ? component.scope : "required",
      licenses,
      purl,
    };
    const hashes = normalizeHashes(component.hashes, `Cargo component ${component.name}@${component.version}`);
    if (hashes) normalized.hashes = hashes;
    const externalReferences = normalizeExternalReferences(component.externalReferences);
    if (externalReferences) normalized.externalReferences = externalReferences;
    const workspace = component["bom-ref"].startsWith("path+file:");
    normalized.properties = [
      { name: "rbcsp:ecosystem", value: "cargo" },
      { name: "rbcsp:sourceKind", value: workspace ? "workspace" : "external" },
    ];
    rawRefToCanonical.set(component["bom-ref"], purl);
    return { normalized, workspace };
  }

  const rootResult = normalizeComponent(rawRoot, true);
  for (const component of raw.components) {
    assert(isObject(component) && typeof component.name === "string" && typeof component.version === "string", "raw Cargo SBOM contains an invalid component");
    if (!cargoAboutInventory.has(`${component.name}@${component.version}`)) continue;
    const result = normalizeComponent(component, false);
    const reference = result.normalized["bom-ref"];
    assert(reference !== rootResult.normalized["bom-ref"], `Cargo dependency duplicates the root component: ${reference}`);
    assert(!componentsByRef.has(reference), `Cargo SBOM has a duplicate canonical component: ${reference}`);
    componentsByRef.set(reference, result.normalized);
    if (!result.workspace) {
      externalRust.push({
        key: `${result.normalized.name}@${result.normalized.version}`,
        name: result.normalized.name,
        version: result.normalized.version,
        license: licenseLabel(result.normalized.licenses),
      });
    }
  }

  const dependencies = new Map();
  for (const dependency of raw.dependencies) {
    assert(
      isObject(dependency) &&
        typeof dependency.ref === "string" &&
        (dependency.dependsOn === undefined || Array.isArray(dependency.dependsOn)),
      "raw Cargo SBOM has an invalid dependency record",
    );
    const reference = rawRefToCanonical.get(dependency.ref);
    if (!reference) continue;
    if (!dependencies.has(reference)) dependencies.set(reference, new Set());
    for (const rawTarget of dependency.dependsOn ?? []) {
      const target = rawRefToCanonical.get(rawTarget);
      if (!target) continue;
      dependencies.get(reference).add(target);
    }
  }
  if (!dependencies.has(rootResult.normalized["bom-ref"])) dependencies.set(rootResult.normalized["bom-ref"], new Set());
  for (const reference of componentsByRef.keys()) {
    if (!dependencies.has(reference)) dependencies.set(reference, new Set());
  }

  externalRust.sort((left, right) => compareOrdinal(`${left.name}@${left.version}`, `${right.name}@${right.version}`));
  return {
    root: rootResult.normalized,
    components: [...componentsByRef.values()].sort((left, right) => compareOrdinal(left["bom-ref"], right["bom-ref"])),
    dependencies,
    externalRust,
  };
}

function npmPackageName(lockKey, entry) {
  if (typeof entry.name === "string" && entry.name) return entry.name;
  const marker = "node_modules/";
  const index = lockKey.lastIndexOf(marker);
  assert(index >= 0, `cannot derive npm package name from lock key: ${lockKey}`);
  return lockKey.slice(index + marker.length);
}

function npmPurl(name, version) {
  if (name.startsWith("@")) {
    const slash = name.indexOf("/");
    assert(slash > 1 && slash < name.length - 1, `invalid scoped npm package name: ${name}`);
    return `pkg:npm/${encodeURIComponent(name.slice(0, slash))}/${encodeURIComponent(name.slice(slash + 1))}@${encodeURIComponent(version)}`;
  }
  return `pkg:npm/${encodeURIComponent(name)}@${encodeURIComponent(version)}`;
}

function npmTarget(target) {
  if (target === "x86_64-pc-windows-msvc") return { os: "win32", cpu: "x64" };
  if (target === "x86_64-unknown-linux-gnu") return { os: "linux", cpu: "x64" };
  fail(`unsupported release target: ${target}`);
}

function constraintAllows(value, actual) {
  if (value === undefined) return true;
  const values = Array.isArray(value) ? value : [value];
  assert(values.every((entry) => typeof entry === "string"), "npm os/cpu constraint must contain strings");
  if (values.includes(`!${actual}`)) return false;
  const positive = values.filter((entry) => !entry.startsWith("!"));
  return positive.length === 0 || positive.includes(actual);
}

function npmEntryAllowsTarget(entry, target) {
  const platform = npmTarget(target);
  return constraintAllows(entry.os, platform.os) && constraintAllows(entry.cpu, platform.cpu);
}

function resolveNpmLockKey(packages, parentKey, dependencyName) {
  if (parentKey) {
    let cursor = parentKey;
    while (cursor) {
      const candidate = `${cursor}/node_modules/${dependencyName}`;
      if (packages[candidate]) return candidate;
      const marker = cursor.lastIndexOf("/node_modules/");
      if (marker < 0) break;
      cursor = cursor.slice(0, marker);
    }
  }
  const rootCandidate = `node_modules/${dependencyName}`;
  return packages[rootCandidate] ? rootCandidate : undefined;
}

function integrityHash(integrity, label) {
  assert(typeof integrity === "string" && integrity.trim(), `${label} is missing npm integrity`);
  const candidates = integrity.trim().split(/\s+/u);
  const priority = ["sha512", "sha384", "sha256"];
  let selected;
  for (const algorithm of priority) {
    selected = candidates.find((candidate) => candidate.startsWith(`${algorithm}-`));
    if (selected) break;
  }
  assert(selected, `${label} has no supported npm integrity algorithm`);
  const separator = selected.indexOf("-");
  const algorithm = selected.slice(0, separator);
  const encoded = selected.slice(separator + 1).split("?", 1)[0];
  const bytes = Buffer.from(encoded, "base64");
  assert(bytes.length > 0, `${label} has invalid npm integrity data`);
  const names = { sha256: "SHA-256", sha384: "SHA-384", sha512: "SHA-512" };
  return { alg: names[algorithm], content: bytes.toString("hex") };
}

function loadFrontendGraph(sourceRoot, target, releaseVersion) {
  const frontendRoot = path.join(sourceRoot, "frontend");
  const lockPath = path.join(frontendRoot, "package-lock.json");
  const packagePath = path.join(frontendRoot, "package.json");
  const lock = readJson(lockPath, "frontend/package-lock.json");
  const packageJson = readJson(packagePath, "frontend/package.json");
  assert(lock.lockfileVersion === 3 && isObject(lock.packages) && isObject(lock.packages[""]), "frontend package-lock must be lockfileVersion 3");
  assert(packageJson.version === releaseVersion && lock.packages[""].version === releaseVersion, "frontend version does not match release version");
  assert(packageJson.name === lock.packages[""].name, "frontend package and lock root names differ");
  assert(isObject(lock.packages[""].dependencies), "frontend lock root is missing production dependencies");

  const packages = lock.packages;
  const edges = new Map([["", new Set()]]);
  const discovered = new Set();
  const queue = [];

  function addDependency(parentKey, dependencyName, optional) {
    const childKey = resolveNpmLockKey(packages, parentKey, dependencyName);
    if (!childKey) {
      if (optional) return;
      fail(`frontend lock cannot resolve ${dependencyName} from ${parentKey || "<root>"}`);
    }
    const child = packages[childKey];
    if (!npmEntryAllowsTarget(child, target)) {
      if (optional) return;
      fail(`required frontend package is incompatible with ${target}: ${childKey}`);
    }
    if (!edges.has(parentKey)) edges.set(parentKey, new Set());
    edges.get(parentKey).add(childKey);
    if (!discovered.has(childKey)) {
      discovered.add(childKey);
      queue.push(childKey);
    }
  }

  for (const dependencyName of Object.keys(lock.packages[""].dependencies).sort(compareOrdinal)) {
    addDependency("", dependencyName, false);
  }

  while (queue.length > 0) {
    const lockKey = queue.shift();
    const entry = packages[lockKey];
    if (!edges.has(lockKey)) edges.set(lockKey, new Set());
    const optionalNames = new Set(Object.keys(entry.optionalDependencies ?? {}));
    for (const dependencyName of Object.keys(entry.dependencies ?? {}).sort(compareOrdinal)) {
      if (!optionalNames.has(dependencyName)) addDependency(lockKey, dependencyName, false);
    }
    for (const dependencyName of [...optionalNames].sort(compareOrdinal)) {
      addDependency(lockKey, dependencyName, true);
    }
    for (const dependencyName of Object.keys(entry.peerDependencies ?? {}).sort(compareOrdinal)) {
      const optional = entry.peerDependenciesMeta?.[dependencyName]?.optional === true;
      if (!optional) addDependency(lockKey, dependencyName, false);
    }
  }

  const lockKeyToRef = new Map();
  const componentsByRef = new Map();
  const noticeEntriesByRef = new Map();
  for (const lockKey of [...discovered].sort(compareOrdinal)) {
    const entry = packages[lockKey];
    const name = npmPackageName(lockKey, entry);
    const version = entry.version;
    assert(typeof version === "string" && version, `npm package ${lockKey} is missing version`);
    const installedPackagePath = path.join(frontendRoot, ...lockKey.split("/"), "package.json");
    const installedPackage = readJson(installedPackagePath, `${lockKey}/package.json`);
    assert(installedPackage.name === name && installedPackage.version === version, `installed npm package does not match lock: ${lockKey}`);
    const declaredLicense = typeof entry.license === "string" ? entry.license : installedPackage.license;
    assert(typeof declaredLicense === "string" && declaredLicense.trim(), `npm package ${name}@${version} is missing a license`);
    const purl = npmPurl(name, version);
    lockKeyToRef.set(lockKey, purl);
    const component = {
      type: "library",
      "bom-ref": purl,
      name,
      version,
      scope: "required",
      hashes: [integrityHash(entry.integrity, `npm package ${name}@${version}`)],
      licenses: [{ expression: declaredLicense.trim() }],
      purl,
      properties: [
        { name: "rbcsp:ecosystem", value: "npm" },
        { name: "rbcsp:lockKey", value: lockKey },
      ],
    };
    if (typeof entry.resolved === "string" && /^https?:\/\//iu.test(entry.resolved)) {
      component.externalReferences = [{ type: "distribution", url: entry.resolved }];
    }
    if (componentsByRef.has(purl)) {
      const existing = componentsByRef.get(purl);
      assert(existing.name === component.name && existing.version === component.version, `conflicting npm component identity: ${purl}`);
    } else {
      componentsByRef.set(purl, component);
    }
    if (!noticeEntriesByRef.has(purl)) {
      noticeEntriesByRef.set(purl, {
        lockKey,
        name,
        version,
        license: declaredLicense.trim(),
        packageJson: installedPackage,
      });
    }
  }

  const rootName = packageJson.name;
  const rootRef = npmPurl(rootName, releaseVersion);
  const rootComponent = {
    type: "library",
    "bom-ref": rootRef,
    name: rootName,
    version: releaseVersion,
    scope: "required",
    licenses: [{ expression: packageJson.license }],
    purl: rootRef,
    properties: [
      { name: "rbcsp:ecosystem", value: "npm" },
      { name: "rbcsp:embeddedFrontend", value: "true" },
    ],
  };

  const dependencies = new Map();
  for (const [parentKey, childKeys] of edges) {
    const parentRef = parentKey === "" ? rootRef : lockKeyToRef.get(parentKey);
    assert(parentRef, `frontend dependency has an unknown parent lock key: ${parentKey}`);
    if (!dependencies.has(parentRef)) dependencies.set(parentRef, new Set());
    for (const childKey of childKeys) {
      const childRef = lockKeyToRef.get(childKey);
      assert(childRef, `frontend dependency has an unknown child lock key: ${childKey}`);
      dependencies.get(parentRef).add(childRef);
    }
  }
  for (const reference of componentsByRef.keys()) {
    if (!dependencies.has(reference)) dependencies.set(reference, new Set());
  }

  return {
    rootRef,
    components: [rootComponent, ...componentsByRef.values()].sort((left, right) => compareOrdinal(left["bom-ref"], right["bom-ref"])),
    dependencies,
    noticeEntries: [...noticeEntriesByRef.values()].sort((left, right) => compareOrdinal(`${left.name}@${left.version}`, `${right.name}@${right.version}`)),
    lockPath,
    packagePath,
  };
}

function addLicenseBlock(blocks, label, text, users) {
  const normalized = normalizeText(text);
  const hash = sha256Buffer(Buffer.from(normalized, "utf8"));
  if (!blocks.has(hash)) {
    blocks.set(hash, { hash, labels: new Set(), text: normalized, users: new Set() });
  }
  const block = blocks.get(hash);
  block.labels.add(label);
  for (const user of users) block.users.add(user);
}

function formatAuthor(author) {
  if (typeof author === "string" && author.trim()) return author.trim();
  if (isObject(author) && typeof author.name === "string" && author.name.trim()) {
    const details = [];
    if (typeof author.email === "string" && author.email.trim()) details.push(`<${author.email.trim()}>`);
    if (typeof author.url === "string" && author.url.trim()) details.push(`(${author.url.trim()})`);
    return `${author.name.trim()}${details.length ? ` ${details.join(" ")}` : ""}`;
  }
  return undefined;
}

function buildThirdPartyNotices({ sourceRoot, packageConfig, target, cargo, cargoAbout, npm }) {
  assert(Array.isArray(cargoAbout.licenses), "cargo-about JSON is missing licenses");
  const blocks = new Map();
  const rustKeys = new Set(cargo.externalRust.map((component) => component.key));
  const coveredRust = new Set();

  for (const license of cargoAbout.licenses) {
    assert(isObject(license) && typeof license.id === "string" && typeof license.text === "string" && Array.isArray(license.used_by), "cargo-about JSON has an invalid license entry");
    const users = [];
    for (const use of license.used_by) {
      const krate = use?.crate;
      if (!isObject(krate) || typeof krate.name !== "string" || typeof krate.version !== "string") continue;
      const key = `${krate.name}@${krate.version}`;
      if (rustKeys.has(key)) {
        users.push(`cargo:${key}`);
        coveredRust.add(key);
      }
    }
    if (users.length > 0) addLicenseBlock(blocks, `${license.id} [cargo-about]`, license.text, users);
  }
  for (const key of rustKeys) assert(coveredRust.has(key), `cargo-about did not provide license text for ${key}`);

  const fallbackTemplatePath = path.join(PACKAGING_ROOT, "licenses", "MIT.txt");
  const fallbackTemplate = fs.readFileSync(fallbackTemplatePath, "utf8");
  assert((fallbackTemplate.match(/<copyright holders>/gu) ?? []).length === 1, "MIT fallback template must contain one copyright placeholder");
  for (const component of npm.noticeEntries) {
    const packageDirectory = path.join(sourceRoot, "frontend", ...component.lockKey.split("/"));
    const licenseFiles = fs
      .readdirSync(packageDirectory, { withFileTypes: true })
      .filter((entry) => entry.isFile() && /^(?:LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)(?:[._-].*)?$/iu.test(entry.name))
      .map((entry) => entry.name)
      .sort(compareOrdinal);
    const user = `npm:${component.name}@${component.version}`;
    if (licenseFiles.length > 0) {
      for (const licenseFile of licenseFiles) {
        const text = fs.readFileSync(path.join(packageDirectory, licenseFile), "utf8");
        addLicenseBlock(blocks, `${component.license} [npm:${licenseFile}]`, text, [user]);
      }
      continue;
    }

    const exactFallback = component.name === "html-parse-stringify" && component.version === "3.0.1" && component.license === "MIT";
    assert(exactFallback, `npm package has no packaged license text and no approved fallback: ${component.name}@${component.version}`);
    const author = formatAuthor(component.packageJson.author);
    assert(author, `npm fallback package is missing author attribution: ${component.name}@${component.version}`);
    addLicenseBlock(blocks, "MIT [npm canonical fallback]", fallbackTemplate.replace("<copyright holders>", author), [user]);
  }

  const lines = [
    "THIRD-PARTY NOTICES",
    "===================",
    "",
    `Package: ${packageConfig.archiveName}`,
    `Version: ${packageConfig.archiveName.match(/-(\d+\.\d+\.\d+)\.(?:zip|tar\.gz)$/u)?.[1] ?? "unknown"}`,
    `Target: ${target}`,
    "",
    "This file covers the target-specific Rust dependency closure and the embedded frontend runtime dependency closure.",
    "Build-only and test-only frontend dependencies are not distributed and are not listed.",
    "",
    `Rust components (${cargo.externalRust.length})`,
    "----------------",
  ];
  for (const component of cargo.externalRust) lines.push(`- ${component.name} ${component.version} - ${component.license}`);
  lines.push("", `JavaScript components (${npm.noticeEntries.length})`, "---------------------");
  for (const component of npm.noticeEntries) lines.push(`- ${component.name} ${component.version} - ${component.license}`);
  lines.push("", "License texts", "=============", "");

  const sortedBlocks = [...blocks.values()].sort((left, right) => {
    const leftLabel = [...left.labels].sort(compareOrdinal).join(";");
    const rightLabel = [...right.labels].sort(compareOrdinal).join(";");
    return compareOrdinal(`${leftLabel}:${left.hash}`, `${rightLabel}:${right.hash}`);
  });
  for (const block of sortedBlocks) {
    lines.push("-------------------------------------------------------------------------------");
    lines.push(`License: ${[...block.labels].sort(compareOrdinal).join("; ")}`);
    lines.push(`Text SHA-256: ${block.hash}`);
    lines.push("Used by:");
    for (const user of [...block.users].sort(compareOrdinal)) lines.push(`- ${user}`);
    lines.push("", block.text.trimEnd(), "");
  }

  return normalizeText(lines.join("\n"));
}

function mergeDependencies(destination, source) {
  for (const [reference, targets] of source) {
    if (!destination.has(reference)) destination.set(reference, new Set());
    for (const target of targets) destination.get(reference).add(target);
  }
}

function buildFinalSbom({ cargo, npm, packageConfig, binaryInfo, sourceCommit, timestamp, target, tools }) {
  const root = {
    ...cargo.root,
    type: "application",
    hashes: [{ alg: "SHA-256", content: binaryInfo.sha256 }],
    properties: [
      { name: "rbcsp:embeddedFrontend", value: "true" },
      { name: "rbcsp:frontendTarget", value: packageConfig.frontendTarget },
      { name: "rbcsp:packageId", value: packageConfig.id },
      { name: "rbcsp:sourceCommit", value: sourceCommit },
      { name: "rbcsp:target", value: target },
    ],
  };
  const components = [...cargo.components, ...npm.components].sort((left, right) => compareOrdinal(left["bom-ref"], right["bom-ref"]));
  const componentRefs = new Set();
  for (const component of components) {
    assert(!componentRefs.has(component["bom-ref"]), `final SBOM has a duplicate component ref: ${component["bom-ref"]}`);
    componentRefs.add(component["bom-ref"]);
  }
  assert(!componentRefs.has(root["bom-ref"]), "final SBOM repeats its metadata component in components");

  const dependencyMap = new Map();
  mergeDependencies(dependencyMap, cargo.dependencies);
  mergeDependencies(dependencyMap, npm.dependencies);
  if (!dependencyMap.has(root["bom-ref"])) dependencyMap.set(root["bom-ref"], new Set());
  dependencyMap.get(root["bom-ref"]).add(npm.rootRef);
  for (const reference of componentRefs) {
    if (!dependencyMap.has(reference)) dependencyMap.set(reference, new Set());
  }
  const knownRefs = new Set([root["bom-ref"], ...componentRefs]);
  const dependencies = [...dependencyMap.entries()]
    .map(([reference, targets]) => {
      assert(knownRefs.has(reference), `final SBOM dependency has unknown ref: ${reference}`);
      const dependsOn = [...targets].sort(compareOrdinal);
      for (const targetRef of dependsOn) assert(knownRefs.has(targetRef), `final SBOM dependency points to unknown ref: ${targetRef}`);
      return { ref: reference, dependsOn };
    })
    .sort((left, right) => compareOrdinal(left.ref, right.ref));

  return {
    bomFormat: "CycloneDX",
    specVersion: "1.6",
    version: 1,
    metadata: {
      timestamp,
      lifecycles: [{ phase: "build" }],
      tools: [
        { vendor: "CycloneDX", name: "cargo-cyclonedx", version: tools.cargoCyclonedx },
        { vendor: "Embark Studios", name: "cargo-about", version: tools.cargoAbout },
        { vendor: "RBCSP", name: "generate-release-metadata.mjs", version: GENERATOR_VERSION },
      ],
      component: root,
      properties: [
        { name: "rbcsp:frontendRuntimeComponentCount", value: String(npm.noticeEntries.length) },
        { name: "rbcsp:rustExternalComponentCount", value: String(cargo.externalRust.length) },
      ],
    },
    components,
    dependencies,
  };
}

function inputHash(root, relativePath, recordedPath = relativePath) {
  const file = path.join(root, ...relativePath.split("/"));
  assert(fs.statSync(file).isFile(), `provenance input is missing: ${recordedPath}`);
  return { path: recordedPath, sha256: sha256File(file) };
}

function buildProvenance({ sourceRoot, packageConfig, releaseInputs, sourceCommit, sourceDateEpoch, timestamp, target, tools, binaryInfo }) {
  const inputs = [
    inputHash(sourceRoot, "Cargo.lock"),
    inputHash(sourceRoot, "frontend/package-lock.json"),
    inputHash(PACKAGING_ROOT, "about.toml", "packaging/about.toml"),
    inputHash(PACKAGING_ROOT, "generate-release-metadata.mjs", "packaging/generate-release-metadata.mjs"),
    inputHash(PACKAGING_ROOT, "licenses/MIT.txt", "packaging/licenses/MIT.txt"),
    inputHash(sourceRoot, "packaging/release-inputs.json"),
  ].sort((left, right) => compareOrdinal(left.path, right.path));
  return {
    schemaVersion: 1,
    packageId: packageConfig.id,
    archiveName: packageConfig.archiveName,
    version: releaseInputs.releaseVersion,
    source: {
      commit: sourceCommit,
      dateEpoch: sourceDateEpoch,
      timestamp,
    },
    build: {
      target,
      profile: releaseInputs.build.cargoProfile,
      cargoLocked: releaseInputs.build.cargoLocked,
      frontendTarget: packageConfig.frontendTarget,
      embeddedWebUi: releaseInputs.build.embeddedWebUi,
    },
    tools: {
      rustc: tools.rustc,
      cargo: tools.cargo,
      node: tools.node,
      npm: tools.npm,
      cargoCyclonedx: tools.cargoCyclonedx,
      cargoAbout: tools.cargoAbout,
      releaseMetadataGenerator: GENERATOR_VERSION,
    },
    inputs,
    artifact: binaryInfo,
  };
}

function buildManifest(stageRoot, packageConfig, releaseInputs, sourceCommit, sourceDateEpoch, target) {
  const scan = scanTree(stageRoot);
  const files = scan.files
    .filter((relativePath) => relativePath !== "MANIFEST.json" && relativePath !== "SHA256SUMS")
    .map((relativePath) => {
      const file = path.join(stageRoot, ...relativePath.split("/"));
      return {
        path: relativePath,
        sizeBytes: fs.statSync(file).size,
        sha256: sha256File(file),
      };
    });
  return {
    schemaVersion: 1,
    packageId: packageConfig.id,
    archiveName: packageConfig.archiveName,
    version: releaseInputs.releaseVersion,
    target,
    sourceCommit,
    sourceDateEpoch,
    excludedFiles: ["MANIFEST.json", "SHA256SUMS"],
    fileCount: files.length,
    files,
  };
}

function writeSha256Sums(stageRoot) {
  const files = scanTree(stageRoot).files.filter((relativePath) => relativePath !== "SHA256SUMS");
  const content = `${files
    .map((relativePath) => `${sha256File(path.join(stageRoot, ...relativePath.split("/")))}  ${relativePath}`)
    .join("\n")}\n`;
  fs.writeFileSync(path.join(stageRoot, "SHA256SUMS"), content, { encoding: "utf8" });
}

function verifyFinalOutputs({ stageRoot, allowlist, sourceRoot, rootRef }) {
  const scan = assertStageShape(stageRoot, allowlist, true);
  const manifest = readJson(path.join(stageRoot, "MANIFEST.json"), "MANIFEST.json");
  assert(manifest.schemaVersion === 1 && Array.isArray(manifest.files), "MANIFEST.json has an invalid schema");
  assert(manifest.fileCount === manifest.files.length, "MANIFEST.json fileCount mismatch");
  const manifestPaths = [];
  for (const entry of manifest.files) {
    assert(isObject(entry) && typeof entry.path === "string" && Number.isSafeInteger(entry.sizeBytes) && /^[0-9a-f]{64}$/u.test(entry.sha256), "MANIFEST.json has an invalid file entry");
    const file = path.join(stageRoot, ...entry.path.split("/"));
    assert(fs.statSync(file).size === entry.sizeBytes, `manifest size mismatch: ${entry.path}`);
    assert(sha256File(file) === entry.sha256, `manifest hash mismatch: ${entry.path}`);
    manifestPaths.push(entry.path);
  }
  const expectedManifestPaths = scan.files.filter((file) => file !== "MANIFEST.json" && file !== "SHA256SUMS");
  assertSameSequence(manifestPaths, expectedManifestPaths, "manifest paths");

  const sumsText = fs.readFileSync(path.join(stageRoot, "SHA256SUMS"), "utf8");
  assert(!sumsText.includes("\r"), "SHA256SUMS must use LF line endings");
  const sumLines = sumsText.trimEnd().split("\n");
  const sumPaths = [];
  for (const line of sumLines) {
    const match = /^([0-9a-f]{64})  (.+)$/u.exec(line);
    assert(match, `invalid SHA256SUMS line: ${line}`);
    const relativePath = validateRelativePath(match[2], "SHA256SUMS path");
    assert(sha256File(path.join(stageRoot, ...relativePath.split("/"))) === match[1], `SHA256SUMS hash mismatch: ${relativePath}`);
    sumPaths.push(relativePath);
  }
  assertSameSequence(sumPaths, scan.files.filter((file) => file !== "SHA256SUMS"), "SHA256SUMS paths");

  const sbom = readJson(path.join(stageRoot, "SBOM.cdx.json"), "SBOM.cdx.json");
  assert(sbom.bomFormat === "CycloneDX" && sbom.specVersion === "1.6" && sbom.version === 1, "SBOM.cdx.json format mismatch");
  assert(sbom.serialNumber === undefined, "SBOM.cdx.json must not contain a random serialNumber");
  assert(sbom.metadata?.component?.["bom-ref"] === rootRef, "SBOM.cdx.json root component mismatch");
  assert(Array.isArray(sbom.components) && Array.isArray(sbom.dependencies), "SBOM.cdx.json graph is missing");
  const references = new Set([rootRef]);
  for (const component of sbom.components) {
    assert(typeof component["bom-ref"] === "string" && !references.has(component["bom-ref"]), `SBOM.cdx.json duplicate component ref: ${component["bom-ref"]}`);
    references.add(component["bom-ref"]);
  }
  const dependencyRefs = [];
  for (const dependency of sbom.dependencies) {
    assert(references.has(dependency.ref) && Array.isArray(dependency.dependsOn), `SBOM.cdx.json has an unknown dependency ref: ${dependency.ref}`);
    dependencyRefs.push(dependency.ref);
    for (const target of dependency.dependsOn) assert(references.has(target), `SBOM.cdx.json dependency points to unknown ref: ${target}`);
  }
  assertSameSequence(dependencyRefs, [...references].sort(compareOrdinal), "SBOM dependency refs");

  const generatedText = ["BUILD-PROVENANCE.json", "MANIFEST.json", "SBOM.cdx.json", "THIRD-PARTY-NOTICES.txt"]
    .map((file) => fs.readFileSync(path.join(stageRoot, file), "utf8"))
    .join("\n");
  const forbiddenPaths = [sourceRoot, stageRoot].flatMap((value) => [value, value.replace(/\\/gu, "/")]);
  for (const forbidden of uniqueSorted(forbiddenPaths.filter((value) => value.length > 3))) {
    assert(!generatedText.toLowerCase().includes(forbidden.toLowerCase()), "generated metadata contains an absolute build path");
  }
  assert(!generatedText.toLowerCase().includes("file://"), "generated metadata contains a file URL");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const sourceRoot = fs.realpathSync(path.resolve(options["source-root"]));
  const stageRoot = fs.realpathSync(path.resolve(options["stage-root"]));
  assert(fs.statSync(sourceRoot).isDirectory(), "--source-root must be a directory");
  assert(fs.statSync(stageRoot).isDirectory(), "--stage-root must be a directory");

  const releaseInputsPath = path.join(sourceRoot, "packaging", "release-inputs.json");
  const releaseInputs = readJson(releaseInputsPath, "packaging/release-inputs.json");
  assert(releaseInputs.schemaVersion === 1 && releaseInputs.packageCount === 2 && Array.isArray(releaseInputs.packages), "release-inputs.json schema mismatch");
  const packageConfig = releaseInputs.packages.find((candidate) => candidate.id === options["package-id"]);
  assert(packageConfig, `unknown release package id: ${options["package-id"]}`);
  assert(PACKAGE_BINARIES.has(packageConfig.id), `unsupported release package id: ${packageConfig.id}`);
  assert(options.target === packageConfig.target, `target does not match release-inputs.json for ${packageConfig.id}`);
  assert(Array.isArray(packageConfig.allowlist), `release package ${packageConfig.id} is missing allowlist`);
  const allowlist = packageConfig.allowlist.map((file) => validateRelativePath(file, "package allowlist path"));
  assert(new Set(allowlist).size === allowlist.length, "package allowlist contains duplicates");
  for (const generated of GENERATED_FILES) assert(allowlist.includes(generated), `package allowlist is missing generated metadata: ${generated}`);
  assertStageShape(stageRoot, allowlist, false);

  const expectedBinary = PACKAGE_BINARIES.get(packageConfig.id);
  const binaryCandidate = path.isAbsolute(options.binary) ? options.binary : path.join(stageRoot, options.binary);
  const binaryAbsolute = fs.realpathSync(binaryCandidate);
  const binaryRelative = pathInside(stageRoot, binaryAbsolute, "--binary");
  assert(binaryRelative === expectedBinary, `binary path mismatch: expected ${expectedBinary}, got ${binaryRelative}`);
  assert(fs.statSync(binaryAbsolute).isFile(), "--binary must identify a regular file");

  const sourceCommit = options["source-commit"].toLowerCase();
  assert(/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/u.test(sourceCommit), "--source-commit must be a 40- or 64-character hexadecimal object id");
  assert(/^\d+$/u.test(options["source-date-epoch"]), "--source-date-epoch must be an integer");
  const sourceDateEpoch = Number(options["source-date-epoch"]);
  assert(Number.isSafeInteger(sourceDateEpoch) && sourceDateEpoch >= 315532800, "--source-date-epoch must be a safe Unix timestamp at or after 1980-01-01");
  const timestamp = new Date(sourceDateEpoch * 1000).toISOString();

  const tools = {
    rustc: options["rustc-version"],
    cargo: options["cargo-version"],
    node: options["node-version"],
    npm: options["npm-version"],
    cargoCyclonedx: options["cargo-cyclonedx-version"],
    cargoAbout: options["cargo-about-version"],
  };
  for (const [name, version] of Object.entries(tools)) {
    assert(typeof version === "string" && version.length > 0 && version.length <= 100 && !/[\r\n]/u.test(version), `invalid ${name} version`);
  }
  assert(tools.rustc === releaseInputs.lockedToolchain.rust, "rustc version does not match release-inputs.json");
  assert(tools.cargo === releaseInputs.lockedToolchain.rust, "Cargo version does not match the pinned Rust toolchain");
  assert(tools.node === releaseInputs.lockedToolchain.node, "Node version does not match release-inputs.json");
  assert(tools.npm === releaseInputs.lockedToolchain.npm, "npm version does not match release-inputs.json");
  assert(tools.cargoCyclonedx === "0.5.9" && tools.cargoAbout === "0.9.1", "release metadata Cargo tool versions are not the approved pins");

  const cargoSection = /\[workspace\.package\]([\s\S]*?)(?=\r?\n\[|$)/u.exec(fs.readFileSync(path.join(sourceRoot, "Cargo.toml"), "utf8"));
  const cargoVersion = cargoSection && /^version\s*=\s*"([^"]+)"\s*$/mu.exec(cargoSection[1])?.[1];
  const frontendPackage = readJson(path.join(sourceRoot, "frontend", "package.json"), "frontend/package.json");
  assert(cargoVersion === releaseInputs.releaseVersion && frontendPackage.version === releaseInputs.releaseVersion, "source versions do not match release-inputs.json");

  const rawCargoSbom = readJson(path.resolve(options["raw-cargo-sbom"]), "raw cargo-cyclonedx JSON");
  const rawCargoAbout = readJson(path.resolve(options["raw-cargo-about"]), "raw cargo-about JSON");
  const expectedCargoRoot = path.posix.basename(packageConfig.binary);
  const cargo = normalizeCargoSbom(
    rawCargoSbom,
    expectedCargoRoot,
    releaseInputs.releaseVersion,
    cargoAboutCrateInventory(rawCargoAbout),
  );
  const npm = loadFrontendGraph(sourceRoot, options.target, releaseInputs.releaseVersion);
  const binaryInfo = {
    path: binaryRelative,
    sizeBytes: fs.statSync(binaryAbsolute).size,
    sha256: sha256File(binaryAbsolute),
  };

  const sbom = buildFinalSbom({
    cargo,
    npm,
    packageConfig,
    binaryInfo,
    sourceCommit,
    timestamp,
    target: options.target,
    tools,
  });
  writeJson(path.join(stageRoot, "SBOM.cdx.json"), sbom);

  const notices = buildThirdPartyNotices({
    sourceRoot,
    packageConfig,
    target: options.target,
    cargo,
    cargoAbout: rawCargoAbout,
    npm,
  });
  fs.writeFileSync(path.join(stageRoot, "THIRD-PARTY-NOTICES.txt"), notices, { encoding: "utf8" });

  const provenance = buildProvenance({
    sourceRoot,
    packageConfig,
    releaseInputs,
    sourceCommit,
    sourceDateEpoch,
    timestamp,
    target: options.target,
    tools,
    binaryInfo,
  });
  writeJson(path.join(stageRoot, "BUILD-PROVENANCE.json"), provenance);

  const preManifestFiles = scanTree(stageRoot).files.filter((file) => file !== "MANIFEST.json" && file !== "SHA256SUMS");
  assertSameSequence(preManifestFiles, allowlist.filter((file) => file !== "MANIFEST.json" && file !== "SHA256SUMS").sort(compareOrdinal), "pre-manifest package files");
  writeJson(
    path.join(stageRoot, "MANIFEST.json"),
    buildManifest(stageRoot, packageConfig, releaseInputs, sourceCommit, sourceDateEpoch, options.target),
  );
  writeSha256Sums(stageRoot);
  verifyFinalOutputs({ stageRoot, allowlist, sourceRoot, rootRef: cargo.root["bom-ref"] });

  process.stdout.write(
    `${JSON.stringify({
      state: "PASS",
      packageId: packageConfig.id,
      archiveName: packageConfig.archiveName,
      rustExternalComponents: cargo.externalRust.length,
      frontendRuntimeComponents: npm.noticeEntries.length,
      files: allowlist.length,
    })}\n`,
  );
}

try {
  main();
} catch (error) {
  process.stderr.write(`release-metadata: ${error.message}\n`);
  process.exitCode = 1;
}
