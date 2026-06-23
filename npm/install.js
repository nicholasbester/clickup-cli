#!/usr/bin/env node

const { execFileSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const https = require("https");

const VERSION = require("./package.json").version;
const REPO = "nicholasbester/clickup-cli";

const PLATFORM_MAP = {
  "darwin-arm64": "clickup-macos-arm64",
  "darwin-x64": "clickup-macos-x86_64",
  "linux-x64": "clickup-linux-x86_64",
  "linux-arm64": "clickup-linux-arm64",
  "win32-x64": "clickup-windows-x86_64",
};

function getPlatformKey() {
  return `${process.platform}-${process.arch}`;
}

function getDownloadUrl() {
  const key = getPlatformKey();
  const name = PLATFORM_MAP[key];
  if (!name) {
    console.error(`Unsupported platform: ${key}`);
    console.error(`Supported: ${Object.keys(PLATFORM_MAP).join(", ")}`);
    process.exit(1);
  }
  const ext = process.platform === "win32" ? "zip" : "tar.gz";
  return `https://github.com/${REPO}/releases/download/v${VERSION}/${name}.${ext}`;
}

function download(url, redirects = 0) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          if (redirects >= 5) {
            return reject(new Error("Download failed: too many redirects"));
          }
          return download(res.headers.location, redirects + 1)
            .then(resolve)
            .catch(reject);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`Download failed: HTTP ${res.statusCode}`));
        }
        const chunks = [];
        res.on("data", (chunk) => chunks.push(chunk));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      })
      .on("error", reject);
  });
}

// Retry the download a few times with exponential backoff. Release assets can
// briefly 404 right after a version is published (asset propagation lag), and
// GitHub may return transient 5xx/403/429 under load — none of which should
// permanently break `npm install`.
async function downloadWithRetry(url, attempts = 4) {
  let lastErr;
  for (let i = 0; i < attempts; i++) {
    try {
      return await download(url);
    } catch (err) {
      lastErr = err;
      if (i < attempts - 1) {
        const delayMs = 1000 * Math.pow(2, i); // 1s, 2s, 4s
        console.log(
          `Download attempt ${i + 1} failed (${err.message}); retrying in ${delayMs / 1000}s...`
        );
        await new Promise((r) => setTimeout(r, delayMs));
      }
    }
  }
  throw lastErr;
}

// Binary names shipped from 0.11.0 onwards. `clickup-cli` is canonical;
// `clkup` is the short alias. Both are included in every release archive.
const BIN_NAMES = ["clickup-cli", "clkup"];

function binFile(name) {
  return process.platform === "win32" ? `${name}.exe` : name;
}

// The version of the binary currently vendored in bin/vendor/, or null if the
// marker is absent/unreadable. Written only after a successful download+extract.
function readVersionMarker(versionFile) {
  try {
    return fs.readFileSync(versionFile, "utf8").trim();
  } catch {
    return null;
  }
}

// The package's `bin` entries are small Node launchers (bin/clickup-cli,
// bin/clkup) so npm can bin-link them uniformly on every platform — including
// Windows, where npm generates a `node <launcher>` shim. The launchers re-exec
// the real platform binary, which postinstall downloads into bin/vendor/. The
// launchers themselves are permanent (never overwritten); only the vendored
// binaries are fetched here. See bin/launch.js.

function tryGenerateCompletions(binPath) {
  // Shell completions only make sense for global installs. Skip local
  // installs (the CLI isn't on PATH anyway) and Windows (no uniform
  // completion story).
  if (process.platform === "win32") return;
  if (process.env.npm_config_global !== "true") return;

  const shell = path.basename(process.env.SHELL || "");
  if (!["bash", "zsh", "fish"].includes(shell)) return;

  try {
    const output = execFileSync(binPath, ["completions", shell], {
      encoding: "utf8",
    });
    const compDir = path.join(__dirname, "completions");
    fs.mkdirSync(compDir, { recursive: true });
    const ext = shell === "fish" ? "fish" : shell;
    const compFile = path.join(compDir, `clickup-cli.${ext}`);
    fs.writeFileSync(compFile, output);

    const instructions = {
      bash: `source ${compFile}   # add this to ~/.bashrc to persist`,
      zsh: `source ${compFile}   # add this to ~/.zshrc to persist`,
      fish: `ln -sf ${compFile} ~/.config/fish/completions/clickup-cli.fish   # load on next shell`,
    };

    console.log(`\nShell completions (${shell}) written to ${compFile}`);
    console.log(`Enable now:\n  ${instructions[shell]}\n`);
  } catch (e) {
    // Completion generation is best-effort; never fail the install.
  }
}

// Single-quote a path for safe interpolation into a PowerShell command. Inside
// a PowerShell single-quoted string a literal `'` is escaped by doubling it;
// spaces and backslashes are already safe. Without this, an install path
// containing an apostrophe (e.g. C:\Users\O'Brien\) breaks Expand-Archive.
function psQuote(p) {
  return "'" + p.replace(/'/g, "''") + "'";
}

async function main() {
  const binDir = path.join(__dirname, "bin");
  const vendorDir = path.join(binDir, "vendor");
  const primaryBin = path.join(vendorDir, binFile(BIN_NAMES[0]));
  const versionFile = path.join(vendorDir, ".version");

  // Skip only if EVERY vendored binary is present AND matches this package
  // version. Checking all names — not just the primary — repairs a partial
  // prior install; the version marker forces a re-download if a stale binary
  // from an earlier version somehow survives (so the launcher never runs a
  // binary that disagrees with the installed package version).
  const allCurrent =
    BIN_NAMES.every((name) =>
      fs.existsSync(path.join(vendorDir, binFile(name)))
    ) && readVersionMarker(versionFile) === VERSION;
  if (allCurrent) {
    return;
  }

  const url = getDownloadUrl();
  console.log(`Downloading clickup-cli v${VERSION}...`);

  const tmpFile = path.join(
    vendorDir,
    process.platform === "win32" ? "tmp.zip" : "tmp.tar.gz"
  );

  try {
    const buffer = await downloadWithRetry(url);
    fs.mkdirSync(vendorDir, { recursive: true });
    fs.writeFileSync(tmpFile, buffer);

    if (process.platform === "win32") {
      execFileSync("powershell", [
        "-command",
        `Expand-Archive -Path ${psQuote(tmpFile)} -DestinationPath ${psQuote(vendorDir)} -Force`,
      ]);
    } else {
      execFileSync("tar", ["xzf", tmpFile, "-C", vendorDir]);
    }

    if (process.platform !== "win32") {
      for (const name of BIN_NAMES) {
        const p = path.join(vendorDir, binFile(name));
        if (fs.existsSync(p)) fs.chmodSync(p, 0o755);
      }
    }

    // Stamp the version last, so a download/extract that fails partway leaves
    // no marker and the next run re-downloads rather than trusting a partial
    // install.
    fs.writeFileSync(versionFile, VERSION);

    console.log(`clickup-cli v${VERSION} installed successfully (binaries: ${BIN_NAMES.join(", ")})`);
    tryGenerateCompletions(primaryBin);
  } catch (err) {
    console.error(`Failed to install clickup-cli: ${err.message}`);
    console.error(
      "Install manually: https://github.com/nicholasbester/clickup-cli/releases"
    );
    process.exit(1);
  } finally {
    // Always clean up the temp archive, even if extraction threw.
    try {
      fs.unlinkSync(tmpFile);
    } catch {
      // not created yet, or already removed — nothing to do.
    }
  }
}

main();
