// Bootstrap script - sets up Web APIs for SSR
// This runs before user code and provides standard Web APIs

const {
  op_console_log,
  op_console_warn,
  op_console_error,
  op_crypto_random_uuid,
  op_crypto_get_random_values,
  op_crypto_subtle_digest,
  op_btoa,
  op_atob,
  op_fetch,
} = Deno.core.ops;

// ============================================================================
// Console
// ============================================================================

function formatArgs(args) {
  return args
    .map((arg) => {
      if (typeof arg === "string") return arg;
      if (arg === null) return "null";
      if (arg === undefined) return "undefined";
      try {
        return JSON.stringify(arg, null, 2);
      } catch {
        return String(arg);
      }
    })
    .join(" ");
}

globalThis.console = {
  log: (...args) => op_console_log(formatArgs(args)),
  warn: (...args) => op_console_warn(formatArgs(args)),
  error: (...args) => op_console_error(formatArgs(args)),
  info: (...args) => op_console_log(formatArgs(args)),
  debug: (...args) => op_console_log(formatArgs(args)),
  trace: () => {},
  dir: () => {},
  table: () => {},
  time: () => {},
  timeEnd: () => {},
  group: () => {},
  groupEnd: () => {},
  clear: () => {},
  assert: () => {},
  count: () => {},
  countReset: () => {},
};

// ============================================================================
// TextEncoder / TextDecoder
// ============================================================================

globalThis.TextEncoder = class TextEncoder {
  constructor() {
    this.encoding = "utf-8";
  }

  encode(string) {
    const buf = new ArrayBuffer(string.length * 3);
    const u8 = new Uint8Array(buf);
    let written = 0;

    for (let i = 0; i < string.length; i++) {
      let c = string.charCodeAt(i);
      if (c < 0x80) {
        u8[written++] = c;
      } else if (c < 0x800) {
        u8[written++] = 0xc0 | (c >> 6);
        u8[written++] = 0x80 | (c & 0x3f);
      } else if (c < 0xd800 || c >= 0xe000) {
        u8[written++] = 0xe0 | (c >> 12);
        u8[written++] = 0x80 | ((c >> 6) & 0x3f);
        u8[written++] = 0x80 | (c & 0x3f);
      } else {
        // Surrogate pair
        i++;
        c = 0x10000 + (((c & 0x3ff) << 10) | (string.charCodeAt(i) & 0x3ff));
        u8[written++] = 0xf0 | (c >> 18);
        u8[written++] = 0x80 | ((c >> 12) & 0x3f);
        u8[written++] = 0x80 | ((c >> 6) & 0x3f);
        u8[written++] = 0x80 | (c & 0x3f);
      }
    }

    return u8.slice(0, written);
  }

  encodeInto(string, u8) {
    const encoded = this.encode(string);
    const len = Math.min(encoded.length, u8.length);
    u8.set(encoded.subarray(0, len));
    return { read: string.length, written: len };
  }
};

globalThis.TextDecoder = class TextDecoder {
  constructor(encoding = "utf-8") {
    this.encoding = encoding.toLowerCase();
    if (this.encoding !== "utf-8" && this.encoding !== "utf8") {
      throw new Error("Only UTF-8 encoding is supported");
    }
  }

  decode(buffer) {
    if (!buffer) return "";
    const u8 = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
    let result = "";
    let i = 0;

    while (i < u8.length) {
      const c = u8[i];
      if (c < 0x80) {
        result += String.fromCharCode(c);
        i++;
      } else if ((c & 0xe0) === 0xc0) {
        result += String.fromCharCode(((c & 0x1f) << 6) | (u8[i + 1] & 0x3f));
        i += 2;
      } else if ((c & 0xf0) === 0xe0) {
        result += String.fromCharCode(
          ((c & 0x0f) << 12) | ((u8[i + 1] & 0x3f) << 6) | (u8[i + 2] & 0x3f)
        );
        i += 3;
      } else if ((c & 0xf8) === 0xf0) {
        const codePoint =
          ((c & 0x07) << 18) |
          ((u8[i + 1] & 0x3f) << 12) |
          ((u8[i + 2] & 0x3f) << 6) |
          (u8[i + 3] & 0x3f);
        // Convert to surrogate pair
        const surrogate = codePoint - 0x10000;
        result += String.fromCharCode(
          0xd800 + (surrogate >> 10),
          0xdc00 + (surrogate & 0x3ff)
        );
        i += 4;
      } else {
        i++;
      }
    }

    return result;
  }
};

// ============================================================================
// URL / URLSearchParams
// ============================================================================

globalThis.URLSearchParams = class URLSearchParams {
  #params = [];

  constructor(init) {
    if (typeof init === "string") {
      const query = init.startsWith("?") ? init.slice(1) : init;
      for (const pair of query.split("&")) {
        if (!pair) continue;
        const [key, ...valueParts] = pair.split("=");
        const value = valueParts.join("=");
        this.#params.push([decodeURIComponent(key), decodeURIComponent(value || "")]);
      }
    } else if (Array.isArray(init)) {
      this.#params = init.map(([k, v]) => [String(k), String(v)]);
    } else if (init && typeof init === "object") {
      for (const [k, v] of Object.entries(init)) {
        this.#params.push([String(k), String(v)]);
      }
    }
  }

  append(name, value) {
    this.#params.push([String(name), String(value)]);
  }

  delete(name) {
    this.#params = this.#params.filter(([k]) => k !== name);
  }

  get(name) {
    const found = this.#params.find(([k]) => k === name);
    return found ? found[1] : null;
  }

  getAll(name) {
    return this.#params.filter(([k]) => k === name).map(([, v]) => v);
  }

  has(name) {
    return this.#params.some(([k]) => k === name);
  }

  set(name, value) {
    let found = false;
    this.#params = this.#params
      .map(([k, v]) => {
        if (k === name) {
          if (found) return null;
          found = true;
          return [k, String(value)];
        }
        return [k, v];
      })
      .filter(Boolean);
    if (!found) this.append(name, value);
  }

  sort() {
    this.#params.sort((a, b) => a[0].localeCompare(b[0]));
  }

  toString() {
    return this.#params
      .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v)}`)
      .join("&");
  }

  *entries() {
    yield* this.#params;
  }

  *keys() {
    for (const [k] of this.#params) yield k;
  }

  *values() {
    for (const [, v] of this.#params) yield v;
  }

  [Symbol.iterator]() {
    return this.entries();
  }

  forEach(callback, thisArg) {
    for (const [k, v] of this.#params) {
      callback.call(thisArg, v, k, this);
    }
  }
};

globalThis.URL = class URL {
  #protocol = "";
  #username = "";
  #password = "";
  #hostname = "";
  #port = "";
  #pathname = "/";
  #search = "";
  #hash = "";
  #searchParams = null;

  constructor(url, base) {
    let fullUrl = url;
    if (base) {
      const baseUrl = new URL(base);
      if (url.startsWith("/")) {
        fullUrl = `${baseUrl.origin}${url}`;
      } else if (!url.match(/^[a-z]+:\/\//i)) {
        const basePath = baseUrl.pathname.replace(/\/[^/]*$/, "/");
        fullUrl = `${baseUrl.origin}${basePath}${url}`;
      }
    }

    const match = fullUrl.match(
      /^([a-z][a-z0-9+.-]*):\/\/(?:([^:@]*)(?::([^@]*))?@)?([^/:?#]*)(?::(\d+))?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/i
    );

    if (!match) throw new TypeError(`Invalid URL: ${url}`);

    this.#protocol = match[1].toLowerCase() + ":";
    this.#username = match[2] ? decodeURIComponent(match[2]) : "";
    this.#password = match[3] ? decodeURIComponent(match[3]) : "";
    this.#hostname = match[4].toLowerCase();
    this.#port = match[5] || "";
    this.#pathname = match[6] || "/";
    this.#search = match[7] || "";
    this.#hash = match[8] || "";
    this.#searchParams = new URLSearchParams(this.#search);
  }

  get protocol() { return this.#protocol; }
  set protocol(v) { this.#protocol = v.endsWith(":") ? v : v + ":"; }

  get username() { return this.#username; }
  set username(v) { this.#username = v; }

  get password() { return this.#password; }
  set password(v) { this.#password = v; }

  get hostname() { return this.#hostname; }
  set hostname(v) { this.#hostname = v; }

  get port() { return this.#port; }
  set port(v) { this.#port = String(v); }

  get pathname() { return this.#pathname; }
  set pathname(v) { this.#pathname = v.startsWith("/") ? v : "/" + v; }

  get search() { return this.#search; }
  set search(v) {
    this.#search = v.startsWith("?") ? v : v ? "?" + v : "";
    this.#searchParams = new URLSearchParams(this.#search);
  }

  get hash() { return this.#hash; }
  set hash(v) { this.#hash = v.startsWith("#") ? v : v ? "#" + v : ""; }

  get host() {
    return this.#port ? `${this.#hostname}:${this.#port}` : this.#hostname;
  }

  get origin() {
    return `${this.#protocol}//${this.host}`;
  }

  get href() {
    let auth = "";
    if (this.#username) {
      auth = this.#password
        ? `${encodeURIComponent(this.#username)}:${encodeURIComponent(this.#password)}@`
        : `${encodeURIComponent(this.#username)}@`;
    }
    return `${this.#protocol}//${auth}${this.host}${this.#pathname}${this.#search}${this.#hash}`;
  }

  get searchParams() {
    return this.#searchParams;
  }

  toString() {
    return this.href;
  }

  toJSON() {
    return this.href;
  }
};

// ============================================================================
// Base64: atob / btoa
// ============================================================================

globalThis.atob = op_atob;
globalThis.btoa = op_btoa;

// ============================================================================
// Crypto
// ============================================================================

const cryptoSubtle = {
  async digest(algorithm, data) {
    const alg = typeof algorithm === "string" ? algorithm : algorithm.name;
    const buffer = data instanceof ArrayBuffer ? new Uint8Array(data) : data;
    const result = op_crypto_subtle_digest(alg, buffer);
    return result.buffer;
  },
};

globalThis.crypto = {
  randomUUID: op_crypto_random_uuid,

  getRandomValues(typedArray) {
    if (!(typedArray instanceof Uint8Array) &&
        !(typedArray instanceof Uint16Array) &&
        !(typedArray instanceof Uint32Array) &&
        !(typedArray instanceof Int8Array) &&
        !(typedArray instanceof Int16Array) &&
        !(typedArray instanceof Int32Array)) {
      throw new TypeError("Expected typed array");
    }
    const u8 = new Uint8Array(typedArray.buffer, typedArray.byteOffset, typedArray.byteLength);
    op_crypto_get_random_values(u8);
    return typedArray;
  },

  subtle: cryptoSubtle,
};

// ============================================================================
// Fetch API (Headers, Request, Response, fetch)
// ============================================================================

globalThis.Headers = class Headers {
  #headers = new Map();

  constructor(init) {
    if (init instanceof Headers) {
      for (const [key, value] of init) {
        this.append(key, value);
      }
    } else if (Array.isArray(init)) {
      for (const [key, value] of init) {
        this.append(key, value);
      }
    } else if (init && typeof init === "object") {
      for (const [key, value] of Object.entries(init)) {
        this.append(key, value);
      }
    }
  }

  append(name, value) {
    const key = name.toLowerCase();
    const existing = this.#headers.get(key);
    if (existing) {
      this.#headers.set(key, existing + ", " + value);
    } else {
      this.#headers.set(key, String(value));
    }
  }

  delete(name) {
    this.#headers.delete(name.toLowerCase());
  }

  get(name) {
    return this.#headers.get(name.toLowerCase()) ?? null;
  }

  has(name) {
    return this.#headers.has(name.toLowerCase());
  }

  set(name, value) {
    this.#headers.set(name.toLowerCase(), String(value));
  }

  *entries() {
    yield* this.#headers.entries();
  }

  *keys() {
    yield* this.#headers.keys();
  }

  *values() {
    yield* this.#headers.values();
  }

  [Symbol.iterator]() {
    return this.entries();
  }

  forEach(callback, thisArg) {
    for (const [key, value] of this.#headers) {
      callback.call(thisArg, value, key, this);
    }
  }
};

globalThis.Request = class Request {
  #url;
  #method;
  #headers;
  #body;

  constructor(input, init = {}) {
    if (input instanceof Request) {
      this.#url = input.url;
      this.#method = init.method || input.method;
      this.#headers = new Headers(init.headers || input.headers);
      this.#body = init.body ?? input.#body;
    } else {
      this.#url = String(input);
      this.#method = (init.method || "GET").toUpperCase();
      this.#headers = new Headers(init.headers);
      this.#body = init.body ?? null;
    }
  }

  get url() { return this.#url; }
  get method() { return this.#method; }
  get headers() { return this.#headers; }
  get body() { return this.#body; }

  clone() {
    return new Request(this);
  }

  async text() {
    return this.#body ? String(this.#body) : "";
  }

  async json() {
    return JSON.parse(await this.text());
  }
};

globalThis.Response = class Response {
  #body;
  #init;
  #headers;

  constructor(body, init = {}) {
    this.#body = body ?? null;
    this.#init = init;
    this.#headers = new Headers(init.headers);
  }

  get ok() {
    const status = this.#init.status ?? 200;
    return status >= 200 && status < 300;
  }

  get status() {
    return this.#init.status ?? 200;
  }

  get statusText() {
    return this.#init.statusText ?? "";
  }

  get headers() {
    return this.#headers;
  }

  get url() {
    return this.#init.url ?? "";
  }

  get bodyUsed() {
    return false; // Simplified
  }

  clone() {
    return new Response(this.#body, this.#init);
  }

  async text() {
    return this.#body ? String(this.#body) : "";
  }

  async json() {
    return JSON.parse(await this.text());
  }

  async arrayBuffer() {
    const text = await this.text();
    return new TextEncoder().encode(text).buffer;
  }

  static json(data, init = {}) {
    return new Response(JSON.stringify(data), {
      ...init,
      headers: {
        "content-type": "application/json",
        ...init.headers,
      },
    });
  }

  static error() {
    return new Response(null, { status: 0, statusText: "" });
  }

  static redirect(url, status = 302) {
    return new Response(null, {
      status,
      headers: { location: url },
    });
  }
};

globalThis.fetch = async function fetch(input, init = {}) {
  let url, method, headers, body;

  if (input instanceof Request) {
    url = input.url;
    method = init.method || input.method;
    headers = {};
    for (const [key, value] of (init.headers ? new Headers(init.headers) : input.headers)) {
      headers[key] = value;
    }
    body = init.body ?? (input.body ? await input.text() : null);
  } else {
    url = String(input);
    method = init.method || "GET";
    headers = {};
    if (init.headers) {
      const h = new Headers(init.headers);
      for (const [key, value] of h) {
        headers[key] = value;
      }
    }
    body = init.body ?? null;
  }

  // Call the Rust op (op_fetch returns a promise)
  const result = await op_fetch({
    url,
    method,
    headers: Object.keys(headers).length > 0 ? headers : null,
    body: body ? String(body) : null,
  });

  // Convert to Response object
  return new Response(result.body, {
    status: result.status,
    statusText: result.status_text,
    headers: result.headers,
    url: result.url,
  });
};

// ============================================================================
// Timer Stubs (no-op for SSR)
// ============================================================================

let timerId = 0;

// setTimeout/setInterval - stub, never fires
globalThis.setTimeout = (fn) => ++timerId;
globalThis.clearTimeout = () => {};
globalThis.setInterval = (fn) => ++timerId;
globalThis.clearInterval = () => {};

// requestAnimationFrame - browser-only, stub
globalThis.requestAnimationFrame = (fn) => ++timerId;
globalThis.cancelAnimationFrame = () => {};

// requestIdleCallback - browser-only, stub
globalThis.requestIdleCallback = (fn) => ++timerId;
globalThis.cancelIdleCallback = () => {};

// ============================================================================
// SSR Internal Render (cached, not accessible to user code)
// ============================================================================

{
  // Closure scope - these variables are NOT accessible from user code
  const renderCache = {};
  const renderErrors = {};

  const ssrInternalRender = async (entry, props) => {
    // Check if we previously failed to load this entry
    if (renderErrors[entry]) {
      throw new Error("Module previously failed to load: " + renderErrors[entry]);
    }

    // Load and cache render function if not already cached
    if (!renderCache[entry]) {
      try {
        const mod = await import(entry);
        const render = mod.default || mod.render;
        if (typeof render !== "function") {
          const err = "Module must export a default function or render function";
          renderErrors[entry] = err;
          throw new Error(err);
        }
        renderCache[entry] = render;
      } catch (e) {
        // Cache the error so we don't retry failed imports
        renderErrors[entry] = e.message || String(e);
        throw e;
      }
    }

    // Call the cached render function
    try {
      return await renderCache[entry](props);
    } catch (e) {
      throw new Error("Render error: " + (e.message || String(e)));
    }
  };

  // Freeze the function so user code cannot replace it
  Object.defineProperty(globalThis, "__ssr_internal_render__", {
    value: ssrInternalRender,
    writable: false,
    configurable: false,
    enumerable: false,
  });
}

// ============================================================================
// Cleanup - remove Deno namespace
// ============================================================================

delete globalThis.Deno;
