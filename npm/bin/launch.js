"use strict";

// Shared launcher used by the `clickup-cli` and `clkup` bin entries.
//
// The npm package ships these tiny Node launchers as its `bin` targets so
// npm's cross-platform bin-linking (which generates a `node <file>` shim on
// Windows and a symlink on Unix) always has a valid Node script to point at.
// The actual platform binary is downloaded by `install.js` (postinstall) into
// `bin/vendor/<name>[.exe]`; this launcher locates it and re-execs it,
// forwarding argv, stdio, and the exit code.
//
// This is what makes the package work on Windows: npm cannot link a command
// name directly to a downloaded `clickup-cli.exe` (the static `bin` map points
// at an extension-less path), but it can link to this Node launcher, which
// then spawns the `.exe`.

const { spawnSync } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

module.exports = function launch(name) {
  const ext = process.platform === "win32" ? ".exe" : "";
  const bin = path.join(__dirname, "vendor", name + ext);

  if (!fs.existsSync(bin)) {
    process.stderr.write(
      `clickup-cli: native binary not found at ${bin}\n` +
        "The postinstall download may have failed or was skipped.\n" +
        "Reinstall with: npm rebuild @nick.bester/clickup-cli\n" +
        "Or download manually: https://github.com/nicholasbester/clickup-cli/releases\n"
    );
    process.exit(1);
  }

  const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });

  if (result.error) {
    process.stderr.write(
      `clickup-cli: failed to launch ${bin}: ${result.error.message}\n`
    );
    process.exit(1);
  }

  // If the child was killed by a signal (e.g. Ctrl-C at an interactive
  // prompt), it has no numeric status — surface the conventional 128+signal
  // exit code so shells and scripts can detect the interruption.
  if (result.signal) {
    const signo = os.constants.signals[result.signal];
    process.exit(typeof signo === "number" ? 128 + signo : 1);
  }
  process.exit(result.status === null ? 1 : result.status);
};
