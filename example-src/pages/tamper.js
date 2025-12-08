// Test page that tries to tamper with SSR internals

export async function render(props) {
  const results = [];

  // Try to access the cache
  results.push("renderCache in globalThis: " + ("__renderCache__" in globalThis));
  results.push("renderErrors in globalThis: " + ("__renderErrors__" in globalThis));

  // Try to overwrite the internal render function
  try {
    globalThis.__ssr_internal_render__ = () => "PWNED";
    results.push("Overwrite __ssr_internal_render__: SUCCESS (BAD!)");
  } catch (e) {
    results.push("Overwrite __ssr_internal_render__: BLOCKED - " + e.message);
  }

  // Try to delete it
  try {
    delete globalThis.__ssr_internal_render__;
    if (globalThis.__ssr_internal_render__) {
      results.push("Delete __ssr_internal_render__: BLOCKED (still exists)");
    } else {
      results.push("Delete __ssr_internal_render__: SUCCESS (BAD!)");
    }
  } catch (e) {
    results.push("Delete __ssr_internal_render__: BLOCKED - " + e.message);
  }

  // Check if it's enumerable
  results.push("__ssr_internal_render__ enumerable: " + Object.keys(globalThis).includes("__ssr_internal_render__"));

  return `<main>
    <h2>Tamper Test</h2>
    <pre>${results.join("\n")}</pre>
  </main>`;
}
