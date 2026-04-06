const { platform, arch } = process;
const path = require("path");
const fs = require("fs");

const PLATFORM_MAP = {
  "linux-x64": "@bifrost-proxy/bifrost-linux-x64",
  "linux-arm64": "@bifrost-proxy/bifrost-linux-arm64",
  "linux-arm": "@bifrost-proxy/bifrost-linux-arm",
  "darwin-x64": "@bifrost-proxy/bifrost-darwin-x64",
  "darwin-arm64": "@bifrost-proxy/bifrost-darwin-arm64",
  "win32-x64": "@bifrost-proxy/bifrost-win32-x64",
  "win32-arm64": "@bifrost-proxy/bifrost-win32-arm64",
};

function getBinaryPath() {
  const key = `${platform}-${arch}`;
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
