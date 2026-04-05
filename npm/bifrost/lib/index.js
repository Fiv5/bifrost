const { platform, arch } = process;
const path = require("path");

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
    return path.join(packageDir, "bin", binaryName);
  } catch {
    throw new Error(
      `The platform-specific package ${packageName} is not installed. ` +
        `Please reinstall @bifrost-proxy/bifrost to fix this.`
    );
  }
}

exports.getBinaryPath = getBinaryPath;
