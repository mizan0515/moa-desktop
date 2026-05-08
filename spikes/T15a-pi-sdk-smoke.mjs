const packageName = "@earendil-works/pi-coding-agent";
const expectedExports = [
  "createAgentSession",
  "DefaultResourceLoader",
  "createEventBus",
  "ModelRegistry",
  "SessionManager",
];

try {
  const sdk = await import(packageName);
  const exports = Object.keys(sdk).sort();
  const missing = expectedExports.filter((name) => !(name in sdk));

  console.log(JSON.stringify({
    result: missing.length === 0 ? "PASS" : "FAIL",
    packageName,
    expectedExports,
    missing,
    exports,
  }, null, 2));
} catch (error) {
  const code = error && typeof error === "object" && "code" in error ? error.code : undefined;
  console.log(JSON.stringify({
    result: "UNVERIFIED",
    reason: code === "ERR_MODULE_NOT_FOUND" ? "package-not-installed" : "import-error",
    packageName,
    expectedExports,
    errorCode: code,
    message: error instanceof Error ? error.message : String(error),
  }, null, 2));
}
