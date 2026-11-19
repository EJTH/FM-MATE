import cargoManifest from "../../src-tauri/Cargo.toml?raw";

const packageSection = cargoManifest.match(
  /^\[package\][^\S\r\n]*\r?\n([\s\S]*?)(?=^\[|(?![\s\S]))/m,
);
const version = packageSection?.[1]?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

if (!version)
  throw new Error("Could not read the app version from Cargo.toml.");

export const appVersion: string = version;

export function isNewerAppVersion(current: string, previous: string): boolean {
  const parse = (value: string) =>
    value.match(
      /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/,
    );
  const next = parse(current);
  const old = parse(previous);
  if (!next || !old) throw new Error("Cannot compare invalid app versions.");

  for (let i = 1; i <= 3; i++) {
    const difference = Number(next[i]) - Number(old[i]);
    if (difference !== 0) return difference > 0;
  }
  if (!next[4] || !old[4]) return !next[4] && !!old[4];

  const nextParts = next[4].split(".");
  const oldParts = old[4].split(".");
  for (let i = 0; i < Math.max(nextParts.length, oldParts.length); i++) {
    const a = nextParts[i];
    const b = oldParts[i];
    if (a === undefined) return false;
    if (b === undefined) return true;
    if (a === b) continue;
    const aNumeric = /^\d+$/.test(a);
    const bNumeric = /^\d+$/.test(b);
    if (aNumeric && bNumeric) return Number(a) > Number(b);
    if (aNumeric !== bNumeric) return bNumeric;
    return a > b;
  }
  return false;
}
