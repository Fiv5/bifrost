const { platform, arch } = process;
const path = require("path");
const fs = require("fs");
const child_process = require("child_process");

const PLATFORM_MAP = {
  "linux-x64-glibc": "@bifrost-proxy/bifrost-linux-x64",
  "linux-x64-musl": "@bifrost-proxy/bifrost-linux-x64-musl",
  "linux-arm64-glibc": "@bifrost-proxy/bifrost-linux-arm64",
  "linux-arm64-musl": "@bifrost-proxy/bifrost-linux-arm64-musl",
  "linux-arm-glibc": "@bifrost-proxy/bifrost-linux-arm",
  "darwin-x64": "@bifrost-proxy/bifrost-darwin-x64",
  "darwin-arm64": "@bifrost-proxy/bifrost-darwin-arm64",
  "win32-x64": "@bifrost-proxy/bifrost-win32-x64",
  "win32-arm64": "@bifrost-proxy/bifrost-win32-arm64",
};

function detectLibc() {
  if (platform !== "linux") return null;
  try {
    const lddOutput = child_process.execSync("ldd --version 2>&1 || true", {
      stdio: ["pipe", "pipe", "pipe"],
      encoding: "utf8",
    });
    if (/musl/i.test(lddOutput)) return "musl";
    if (/GLIBC|GNU libc/i.test(lddOutput)) return "glibc";
  } catch {}
  try {
    if (fs.existsSync("/lib/ld-musl-x86_64.so.1") ||
        fs.existsSync("/lib/ld-musl-aarch64.so.1") ||
        fs.existsSync("/lib/ld-musl-armhf.so.1")) {
      return "musl";
    }
  } catch {}
  return "glibc";
}

function getBinaryPath() {
  let key = `${platform}-${arch}`;
  if (platform === "linux") {
    const libc = detectLibc();
    key = `${key}-${libc}`;
  }
  const packageName = PLATFORM_MAP[key];

  if (!packageName) {
    throw new Error(
      `Unsupported platform: ${platform}-${arch}. ` +
        `Supported platforms: ${Object.keys(PLATFORM_MAP).join(", ")}`
    );
  }

  const binaryName = platform === "win32" ? "bifrost.exe" : "bifrost";

  try {
    const packageDir = path.dirname(require.resolve(`${packageName}/package.json`));
    const binPath = path.join(packageDir, "bin", binaryName);
    if (fs.existsSync(binPath)) return binPath;
  } catch {}

  const downloadedPath = path.join(
    __dirname,
    "..",
    `downloaded-${packageName.replace("/", "-").replace("@", "")}-${binaryName}`
  );
  if (fs.existsSync(downloadedPath)) return downloadedPath;

  throw new Error(
    `The platform-specific package ${packageName} is not installed. ` +
      `Please reinstall @bifrost-proxy/bifrost to fix this.`
  );
}

exports.getBinaryPath = getBinaryPath;
