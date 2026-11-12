import { readdir, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const locales = ["en-US", "zh-CN"];
const catalogs = new Map(
  await Promise.all(
    locales.map(async (locale) => [
      locale,
      JSON.parse(await readFile(path.join(root, "locales", `${locale}.json`), "utf8")),
    ]),
  ),
);

function keysOf(catalog) {
  return Object.keys(catalog)
    .filter((key) => !key.startsWith("$"))
    .sort();
}

const reference = keysOf(catalogs.get("en-US"));
const errors = [];

for (const locale of locales) {
  const catalog = catalogs.get(locale);
  const keys = keysOf(catalog);
  const missing = reference.filter((key) => !keys.includes(key));
  const extra = keys.filter((key) => !reference.includes(key));
  if (missing.length) {
    errors.push(`${locale} missing: ${missing.join(", ")}`);
  }
  if (extra.length) {
    errors.push(`${locale} extra: ${extra.join(", ")}`);
  }
  for (const key of keys) {
    if (typeof catalog[key] !== "string" || catalog[key].trim() === "") {
      errors.push(`${locale} has an empty value for ${key}`);
    }
    const sourcePlaceholders = new Set(
      [...String(catalogs.get("en-US")[key]).matchAll(/\{(\w+)\}/g)].map((match) => match[1]),
    );
    const localePlaceholders = new Set(
      [...String(catalog[key]).matchAll(/\{(\w+)\}/g)].map((match) => match[1]),
    );
    for (const placeholder of sourcePlaceholders) {
      if (!localePlaceholders.has(placeholder)) {
        errors.push(`${locale} is missing placeholder {${placeholder}} in ${key}`);
      }
    }
  }
}

async function sourceFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await sourceFiles(fullPath)));
    } else if (entry.name.endsWith(".ts") || entry.name.endsWith(".tsx")) {
      files.push(fullPath);
    }
  }
  return files;
}

const referenceKeys = new Set(reference);
for (const file of await sourceFiles(path.join(root, "src"))) {
  const source = await readFile(file, "utf8");
  for (const match of source.matchAll(/\bt\(\s*["']([^"']+)["']/g)) {
    if (!referenceKeys.has(match[1])) {
      errors.push(`${path.relative(root, file)} references missing key ${match[1]}`);
    }
  }
}

if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}

// The egui popup falls back to its own compiled bundle for standalone builds,
// but the shared settings catalog is canonical. Keep the two in lockstep so a
// merged key can never silently drift.
const popupRoot = path.resolve(root, "../../ditto-popup/locales");
const popupCatalogs = new Map(
  await Promise.all(
    locales.map(async (locale) => [
      locale,
      JSON.parse(await readFile(path.join(popupRoot, `${locale}.json`), "utf8")),
    ]),
  ),
);
const popupReference = keysOf(popupCatalogs.get("en-US"));
for (const locale of locales) {
  const catalog = popupCatalogs.get(locale);
  const keys = keysOf(catalog);
  const missing = popupReference.filter((key) => !keys.includes(key));
  const extra = keys.filter((key) => !popupReference.includes(key));
  if (missing.length) {
    errors.push(`popup ${locale} missing: ${missing.join(", ")}`);
  }
  if (extra.length) {
    errors.push(`popup ${locale} extra: ${extra.join(", ")}`);
  }
  for (const key of keys) {
    if (!referenceKeys.has(key)) {
      errors.push(`popup ${locale} key ${key} is not in the shared catalog`);
    } else if (catalog[key] !== catalogs.get(locale)[key]) {
      errors.push(`popup ${locale} value for ${key} differs from the shared catalog`);
    }
  }
}

if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log(`Locale catalogs agree on ${reference.length} shared and ${popupReference.length} popup keys.`);
