// API availability test - check what's available in the sandbox

export async function render(props) {
  const apis = [];

  // Check each API
  const checks = [
    // Encoding
    ["TextEncoder", () => typeof TextEncoder !== "undefined"],
    ["TextDecoder", () => typeof TextDecoder !== "undefined"],
    ["atob/btoa", () => typeof atob !== "undefined" && typeof btoa !== "undefined"],

    // URL
    ["URL", () => typeof URL !== "undefined"],
    ["URLSearchParams", () => typeof URLSearchParams !== "undefined"],

    // Intl
    ["Intl", () => typeof Intl !== "undefined"],
    ["Intl.DateTimeFormat", () => typeof Intl?.DateTimeFormat !== "undefined"],
    ["Intl.NumberFormat", () => typeof Intl?.NumberFormat !== "undefined"],
    ["Intl.Collator", () => typeof Intl?.Collator !== "undefined"],
    ["Intl.PluralRules", () => typeof Intl?.PluralRules !== "undefined"],
    ["Intl.RelativeTimeFormat", () => typeof Intl?.RelativeTimeFormat !== "undefined"],
    ["Intl.ListFormat", () => typeof Intl?.ListFormat !== "undefined"],
    ["Intl.Segmenter", () => typeof Intl?.Segmenter !== "undefined"],

    // Crypto
    ["crypto", () => typeof crypto !== "undefined"],
    ["crypto.subtle", () => typeof crypto?.subtle !== "undefined"],
    ["crypto.randomUUID", () => typeof crypto?.randomUUID !== "undefined"],
    ["crypto.getRandomValues", () => typeof crypto?.getRandomValues !== "undefined"],

    // Timers
    ["setTimeout", () => typeof setTimeout !== "undefined"],
    ["setInterval", () => typeof setInterval !== "undefined"],
    ["clearTimeout", () => typeof clearTimeout !== "undefined"],
    ["clearInterval", () => typeof clearInterval !== "undefined"],
    ["requestAnimationFrame", () => typeof requestAnimationFrame !== "undefined"],
    ["cancelAnimationFrame", () => typeof cancelAnimationFrame !== "undefined"],
    ["requestIdleCallback", () => typeof requestIdleCallback !== "undefined"],
    ["cancelIdleCallback", () => typeof cancelIdleCallback !== "undefined"],
    ["queueMicrotask", () => typeof queueMicrotask !== "undefined"],

    // Streams
    ["ReadableStream", () => typeof ReadableStream !== "undefined"],
    ["WritableStream", () => typeof WritableStream !== "undefined"],
    ["TransformStream", () => typeof TransformStream !== "undefined"],

    // Collections
    ["Map", () => typeof Map !== "undefined"],
    ["Set", () => typeof Set !== "undefined"],
    ["WeakMap", () => typeof WeakMap !== "undefined"],
    ["WeakSet", () => typeof WeakSet !== "undefined"],
    ["WeakRef", () => typeof WeakRef !== "undefined"],

    // Typed Arrays
    ["ArrayBuffer", () => typeof ArrayBuffer !== "undefined"],
    ["Uint8Array", () => typeof Uint8Array !== "undefined"],
    ["DataView", () => typeof DataView !== "undefined"],

    // Async
    ["Promise", () => typeof Promise !== "undefined"],
    ["AsyncGenerator", () => typeof (async function*(){}).constructor !== "undefined"],

    // JSON
    ["JSON", () => typeof JSON !== "undefined"],

    // Error types
    ["Error", () => typeof Error !== "undefined"],
    ["AggregateError", () => typeof AggregateError !== "undefined"],

    // Reflect/Proxy
    ["Proxy", () => typeof Proxy !== "undefined"],
    ["Reflect", () => typeof Reflect !== "undefined"],

    // Other Web APIs
    ["fetch", () => typeof fetch !== "undefined"],
    ["Request", () => typeof Request !== "undefined"],
    ["Response", () => typeof Response !== "undefined"],
    ["Headers", () => typeof Headers !== "undefined"],
    ["AbortController", () => typeof AbortController !== "undefined"],
    ["AbortSignal", () => typeof AbortSignal !== "undefined"],
    ["Blob", () => typeof Blob !== "undefined"],
    ["File", () => typeof File !== "undefined"],
    ["FormData", () => typeof FormData !== "undefined"],
    ["Event", () => typeof Event !== "undefined"],
    ["EventTarget", () => typeof EventTarget !== "undefined"],
    ["CustomEvent", () => typeof CustomEvent !== "undefined"],

    // Console
    ["console", () => typeof console !== "undefined"],

    // Performance
    ["performance", () => typeof performance !== "undefined"],
    ["performance.now", () => typeof performance?.now !== "undefined"],

    // WebAssembly
    ["WebAssembly", () => typeof WebAssembly !== "undefined"],

    // Structured Clone
    ["structuredClone", () => typeof structuredClone !== "undefined"],

    // Compression
    ["CompressionStream", () => typeof CompressionStream !== "undefined"],
    ["DecompressionStream", () => typeof DecompressionStream !== "undefined"],
  ];

  for (const [name, check] of checks) {
    try {
      const available = check();
      apis.push({ name, available });
    } catch (e) {
      apis.push({ name, available: false, error: e.message });
    }
  }

  const available = apis.filter(a => a.available);
  const missing = apis.filter(a => !a.available);

  return `<main>
    <h2>API Availability in Sandbox</h2>

    <h3>Available (${available.length})</h3>
    <pre style="background:#1a1a1a;color:#0f0;padding:1em;max-height:300px;overflow:auto;">${available.map(a => `✓ ${a.name}`).join("\n")}</pre>

    <h3>Missing (${missing.length})</h3>
    <pre style="background:#1a1a1a;color:#f66;padding:1em;max-height:300px;overflow:auto;">${missing.map(a => `✗ ${a.name}${a.error ? ` (${a.error})` : ""}`).join("\n")}</pre>
  </main>`;
}
