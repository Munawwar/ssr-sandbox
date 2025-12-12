// Bootstrap script - sets up additional APIs for SSR
// Deno extensions provide: console, URL, URLSearchParams, TextEncoder, TextDecoder,
// atob, btoa, crypto (randomUUID, getRandomValues, subtle)
//
// This script adds: fetch, console capture, SSR render function

// Import deno extension modules and set up globals
// Order matters: dependencies first
import "ext:deno_webidl/00_webidl.js";
import "ext:deno_console/01_console.js";
import { URL, URLSearchParams } from "ext:deno_url/00_url.js";
import "ext:deno_url/01_urlpattern.js";
import "ext:deno_web/00_infra.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import "ext:deno_web/01_mimesniff.js";
import {
  Event,
  EventTarget,
  CustomEvent,
  ErrorEvent,
  CloseEvent,
  MessageEvent,
  ProgressEvent,
  PromiseRejectionEvent,
} from "ext:deno_web/02_event.js";
import { structuredClone } from "ext:deno_web/02_structured_clone.js";
import "ext:deno_web/02_timers.js";
import { AbortController, AbortSignal } from "ext:deno_web/03_abort_signal.js";
import "ext:deno_web/04_global_interfaces.js";
import { atob, btoa } from "ext:deno_web/05_base64.js";
import {
  ReadableStream,
  TransformStream,
  WritableStream,
  ByteLengthQueuingStrategy,
  CountQueuingStrategy,
} from "ext:deno_web/06_streams.js";
import {
  TextEncoder,
  TextDecoder,
  TextEncoderStream,
  TextDecoderStream,
} from "ext:deno_web/08_text_encoding.js";
import { Blob, File } from "ext:deno_web/09_file.js";
import { FileReader } from "ext:deno_web/10_filereader.js";
import "ext:deno_web/12_location.js";
import { MessageChannel, MessagePort } from "ext:deno_web/13_message_port.js";
import { CompressionStream, DecompressionStream } from "ext:deno_web/14_compression.js";
import { Performance, performance } from "ext:deno_web/15_performance.js";
import { ImageData } from "ext:deno_web/16_image_data.js";
import { crypto, Crypto, CryptoKey, SubtleCrypto } from "ext:deno_crypto/00_crypto.js";

// Assign to globalThis
globalThis.URL = URL;
globalThis.URLSearchParams = URLSearchParams;
globalThis.DOMException = DOMException;
globalThis.Event = Event;
globalThis.EventTarget = EventTarget;
globalThis.CustomEvent = CustomEvent;
globalThis.ErrorEvent = ErrorEvent;
globalThis.CloseEvent = CloseEvent;
globalThis.MessageEvent = MessageEvent;
globalThis.ProgressEvent = ProgressEvent;
globalThis.PromiseRejectionEvent = PromiseRejectionEvent;
globalThis.structuredClone = structuredClone;
globalThis.AbortController = AbortController;
globalThis.AbortSignal = AbortSignal;
globalThis.atob = atob;
globalThis.btoa = btoa;
globalThis.ReadableStream = ReadableStream;
globalThis.TransformStream = TransformStream;
globalThis.WritableStream = WritableStream;
globalThis.ByteLengthQueuingStrategy = ByteLengthQueuingStrategy;
globalThis.CountQueuingStrategy = CountQueuingStrategy;
globalThis.TextEncoder = TextEncoder;
globalThis.TextDecoder = TextDecoder;
globalThis.TextEncoderStream = TextEncoderStream;
globalThis.TextDecoderStream = TextDecoderStream;
globalThis.Blob = Blob;
globalThis.File = File;
globalThis.FileReader = FileReader;
globalThis.MessageChannel = MessageChannel;
globalThis.MessagePort = MessagePort;
globalThis.CompressionStream = CompressionStream;
globalThis.DecompressionStream = DecompressionStream;
globalThis.Performance = Performance;
globalThis.performance = performance;
globalThis.ImageData = ImageData;
globalThis.crypto = crypto;
globalThis.Crypto = Crypto;
globalThis.CryptoKey = CryptoKey;
globalThis.SubtleCrypto = SubtleCrypto;

const {
  op_console_log,
  op_console_warn,
  op_console_error,
  op_fetch,
} = Deno.core.ops;

// ============================================================================
// Console Capture - wrap the deno_console to capture output
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

// Save original console methods from deno_console
const originalConsole = globalThis.console;

// Override console to capture output while still using deno_console formatting
globalThis.console = {
  log: (...args) => {
    op_console_log(formatArgs(args));
    // Also call original for any side effects
  },
  warn: (...args) => {
    op_console_warn(formatArgs(args));
  },
  error: (...args) => {
    op_console_error(formatArgs(args));
  },
  info: (...args) => op_console_log(formatArgs(args)),
  debug: (...args) => op_console_log(formatArgs(args)),
  trace: originalConsole?.trace?.bind(originalConsole) || (() => {}),
  dir: originalConsole?.dir?.bind(originalConsole) || (() => {}),
  table: originalConsole?.table?.bind(originalConsole) || (() => {}),
  time: originalConsole?.time?.bind(originalConsole) || (() => {}),
  timeEnd: originalConsole?.timeEnd?.bind(originalConsole) || (() => {}),
  timeLog: originalConsole?.timeLog?.bind(originalConsole) || (() => {}),
  group: originalConsole?.group?.bind(originalConsole) || (() => {}),
  groupCollapsed: originalConsole?.groupCollapsed?.bind(originalConsole) || (() => {}),
  groupEnd: originalConsole?.groupEnd?.bind(originalConsole) || (() => {}),
  clear: originalConsole?.clear?.bind(originalConsole) || (() => {}),
  assert: originalConsole?.assert?.bind(originalConsole) || (() => {}),
  count: originalConsole?.count?.bind(originalConsole) || (() => {}),
  countReset: originalConsole?.countReset?.bind(originalConsole) || (() => {}),
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
