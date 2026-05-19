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

function download(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          return download(res.headers.location).then(resolve).catch(reject);
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

// Binary names shipped from 0.11.0 onwards. `clickup-cli` is canonical;
// `clkup` is the short alias. Both are included in every release tarball.
const BIN_NAMES = ["clickup-cli", "clkup"];

function binFile(name) {
  return process.platform === "win32" ? `${name}.exe` : name;
}

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

async function main() {
  const binDir = path.join(__dirname, "bin");
  const primaryBin = path.join(binDir, binFile(BIN_NAMES[0]));

  // Skip if the primary binary already exists (e.g. previous install).
  if (fs.existsSync(primaryBin)) {
    return;
  }

  const url = getDownloadUrl();
  console.log(`Downloading clickup-cli v${VERSION}...`);

  try {
    const buffer = await download(url);
    fs.mkdirSync(binDir, { recursive: true });

    const tmpFile = path.join(binDir, process.platform === "win32" ? "tmp.zip" : "tmp.tar.gz");
    fs.writeFileSync(tmpFile, buffer);

    if (process.platform === "win32") {
      execFileSync("powershell", [
        "-command",
        `Expand-Archive -Path '${tmpFile}' -DestinationPath '${binDir}' -Force`,
      ]);
    } else {
      execFileSync("tar", ["xzf", tmpFile, "-C", binDir]);
    }

    fs.unlinkSync(tmpFile);

    if (process.platform !== "win32") {
      for (const name of BIN_NAMES) {
        const p = path.join(binDir, binFile(name));
        if (fs.existsSync(p)) fs.chmodSync(p, 0o755);
      }
    }

    console.log(`clickup-cli v${VERSION} installed successfully (binaries: ${BIN_NAMES.join(", ")})`);
    tryGenerateCompletions(primaryBin);
  } catch (err) {
    console.error(`Failed to install clickup-cli: ${err.message}`);
    console.error(
      "Install manually: https://github.com/nicholasbester/clickup-cli/releases"
    );
    process.exit(1);
  }
}

main();
