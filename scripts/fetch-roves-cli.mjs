/**
 * Puts the roves-cli sidecar where Tauri expects `bundle.externalBin` ("binaries/roves"):
 * `src-tauri/binaries/roves-<target-triple>[.exe]`. It downloads the standalone executable the
 * roves-cli release of the pinned version (src/lib/roves-cli-version.txt) publishes, then checks
 * that it runs and reports that version.
 *
 *   npm run sidecar                       # the pinned release, for this host's Rust target
 *   npm run sidecar -- --from <file>      # a local build (roves-cli's `npm run build:sea`)
 *   npm run sidecar -- --target <triple>  # another Rust target triple
 */
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = readFileSync(join(root, "src/lib/roves-cli-version.txt"), "utf8").trim();

function option(name) {
    const at = process.argv.indexOf(name);
    return at >= 0 ? process.argv[at + 1] : undefined;
}

/** The host's Rust target triple, as Tauri names sidecars. */
function hostTriple() {
    try {
        const output = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
        const host = /^host: (\S+)$/m.exec(output)?.[1];
        if (host) return host;
    } catch {
        // No Rust toolchain: derive it from Node's platform and CPU.
    }
    const cpu = { x64: "x86_64", arm64: "aarch64" }[process.arch];
    const os = {
        win32: "pc-windows-msvc",
        darwin: "apple-darwin",
        linux: "unknown-linux-gnu",
    }[process.platform];
    if (!cpu || !os) throw new Error(`no roves-cli build for ${process.platform}/${process.arch}`);
    return `${cpu}-${os}`;
}

const triple = option("--target") ?? hostTriple();
const name = `roves-${triple}${triple.includes("windows") ? ".exe" : ""}`;
const dest = join(root, "src-tauri", "binaries", name);
mkdirSync(dirname(dest), { recursive: true });

const from = option("--from");
if (from) {
    copyFileSync(from, dest);
    console.log(`Copied ${from} to ${dest}`);
} else {
    const url = `https://github.com/DRincs-Productions/roves-cli/releases/download/v${version}/${name}`;
    console.log(`Downloading ${url}`);
    const response = await fetch(url, { redirect: "follow", headers: { "User-Agent": "roves-packmaster" } });
    if (!response.ok) {
        throw new Error(`downloading ${url}: HTTP ${response.status} (is roves-cli v${version} released with ${name}?)`);
    }
    writeFileSync(dest, Buffer.from(await response.arrayBuffer()));
}
chmodSync(dest, 0o755);

if (triple === hostTriple()) {
    const reported = execFileSync(dest, ["--version"], { encoding: "utf8" }).trim();
    if (!from && reported !== version) {
        throw new Error(`${dest} reports version ${reported}, expected ${version}`);
    }
    console.log(`${dest}: roves-cli ${reported}`);
}
