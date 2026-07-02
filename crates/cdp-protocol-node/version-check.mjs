// Fail if the npm package version and the core crate version disagree.
// Runs in prepublishOnly and in CI so a release can't ship mismatched versions.
import { readFileSync } from 'node:fs'

const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url)))
const cargo = readFileSync(new URL('../cdp-protocol/Cargo.toml', import.meta.url), 'utf8')

const m = cargo.match(/^\s*version\s*=\s*"([^"]+)"/m)
const crateVersion = m?.[1]

if (!crateVersion) {
  console.error('version-check: could not read version from ../cdp-protocol/Cargo.toml')
  process.exit(1)
}

if (pkg.version !== crateVersion) {
  console.error(
    `version-check: mismatch\n  npm   package.json: ${pkg.version}\n  crate Cargo.toml:   ${crateVersion}`,
  )
  process.exit(1)
}

console.log(`version-check: ok (${pkg.version})`)
