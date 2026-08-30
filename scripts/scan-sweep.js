// Can the shipped reader read the code the shipped encoder draws?
//
// **This is the one half of the brand tier that could be measured and was not
// repeatable.** The figure "210 of 210 across 7 scales and 30 angles" is quoted
// in four documents, in `chaos scan`'s own refusal text, and in a test that
// asserts the refusal quotes it -- and the harness that produced it was nowhere
// in the repository. A number no one can re-run is a claim, not a measurement.
//
// What this does, with no camera, no browser and no canvas:
//
//   1. takes a payload, and the module grid for it from `chaos-qr --ascii`
//      (the Rust encoder that ships in every binary), or from --grid;
//   2. rasterises that grid to a greyscale buffer at a given scale and angle,
//      with its own inverse-rotation sampler -- so the reader is fed pixels it
//      has never seen rather than the grid it would sample anyway;
//   3. calls `readFrame(gray, w, h)` from `assets/grimoire/scanner.html` -- the
//      **shipped** detector, the same code the phone's SCAN button and the
//      browser's /scan route run -- extracted from the file, not reimplemented;
//   4. reports pass / not-read / **wrong string** separately.
//
// That third column is the whole point. Decoding fails by returning a plausible
// wrong string rather than an error, which CLAUDE.md names as the worst shape a
// bug can take in this project. "Declined to read" is safe. "Read something
// else" is not, and the two must never be summed into one number.
//
//   node scripts/scan-sweep.js                      # the default sweep
//   node scripts/scan-sweep.js --text "http://..."  # a payload of your own
//   node scripts/scan-sweep.js --grid path.txt      # rows of 0/1, no quiet zone
//   node scripts/scan-sweep.js --scales 3,4,6 --angles 0,15,30
//
// Exit 0 when every case at 3 px per module and above reads correctly and
// nothing anywhere returns a wrong string; 1 otherwise.

const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..");

// ---- arguments -------------------------------------------------------------

function arg(name, dflt) {
  const i = process.argv.indexOf("--" + name);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : dflt;
}
const TEXT = arg("text", "http://192.168.1.105:8232");
const GRID_FILE = arg("grid", null);
const SCALES = arg("scales", "2,3,4,5,6,8,10,12").split(",").map(Number);
const ANGLES = arg("angles", null)
  ? arg("angles").split(",").map(Number)
  : Array.from({ length: 30 }, (_, i) => i * 3);
// Below this many pixels per module the specification gives no promise, so a
// miss is not a defect. Stated here rather than discovered in the output.
const USABLE_FROM = 3;

// ---- the reader, taken out of the page that ships it -----------------------

function loadReader() {
  const file = path.join(ROOT, "assets", "grimoire", "scanner.html");
  const html = fs.readFileSync(file, "utf8");
  const blocks = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].map((m) => m[1]);
  if (!blocks.length) throw new Error("no <script> block in scanner.html");
  const src = blocks[blocks.length - 1];
  for (const needed of ["function readFrame(", "function decodeGrid("]) {
    if (!src.includes(needed)) {
      throw new Error(
        "scanner.html no longer contains " + needed + " -- this harness reads " +
          "the shipped source, so it has to follow a rename rather than guess."
      );
    }
  }
  // **The script is run whole and unmodified.** An earlier attempt cut it at
  // `startCamera()` and appended an export, which left the file's own
  // `(function () {` unclosed inside a wrapper of mine -- "Unexpected end of
  // input" from a page that parses perfectly. There is no need for any of that:
  // the page already ends by assigning `window.__scry`, put there for exactly
  // this ("exposed so it can be tested without a camera"), and it does so
  // before it starts the camera. So run the real thing against a window stub
  // and read the hook the product itself provides.
  const body = src;
  // A DOM stub, because the page's top level touches document and navigator
  // while defining things. Nothing here is drawn or displayed.
  const noop = () => {};
  const elementStub = () => ({
    getContext: () => ({
      fillRect: noop, drawImage: noop, clearRect: noop, save: noop, restore: noop,
      translate: noop, rotate: noop, beginPath: noop, stroke: noop, fill: noop,
      moveTo: noop, lineTo: noop, arc: noop, closePath: noop, setTransform: noop,
      getImageData: () => ({ data: new Uint8ClampedArray(4) }),
    }),
    addEventListener: noop, removeEventListener: noop, appendChild: noop,
    setAttribute: noop, style: {}, classList: { add: noop, remove: noop, toggle: noop },
    querySelector: () => elementStub(), querySelectorAll: () => [],
    getBoundingClientRect: () => ({ x: 0, y: 0, width: 0, height: 0, top: 0, left: 0 }),
    width: 0, height: 0, textContent: "", value: "",
  });
  const documentStub = {
    getElementById: () => elementStub(),
    querySelector: () => elementStub(),
    querySelectorAll: () => [],
    createElement: () => elementStub(),
    addEventListener: noop, removeEventListener: noop,
    documentElement: elementStub(), body: elementStub(),
    hidden: false, visibilityState: "visible",
  };
  const windowStub = {
    addEventListener: noop, removeEventListener: noop,
    location: { href: "http://127.0.0.1:8231/scan", protocol: "http:", hostname: "127.0.0.1" },
    matchMedia: () => ({ matches: false, addEventListener: noop, addListener: noop }),
    devicePixelRatio: 1, innerWidth: 800, innerHeight: 600,
    requestAnimationFrame: noop, cancelAnimationFrame: noop,
    setInterval: () => 0, clearInterval: noop, setTimeout: () => 0, clearTimeout: noop,
    localStorage: { getItem: () => null, setItem: noop, removeItem: noop },
  };
  const navigatorStub = { mediaDevices: undefined, userAgent: "node" };
  const make = new Function(
    "window", "document", "navigator", "requestAnimationFrame",
    "cancelAnimationFrame", "setInterval", "clearInterval", "BarcodeDetector",
    "localStorage", "location", "screen", "matchMedia", "getComputedStyle",
    body
  );
  try {
    make(
      windowStub, documentStub, navigatorStub, noop, noop, () => 0, noop,
      undefined, windowStub.localStorage, windowStub.location,
      { orientation: {} }, windowStub.matchMedia,
      () => ({ getPropertyValue: () => "" })
    );
  } catch (e) {
    // Starting the camera is the last thing the page does and there is no
    // camera here. The hook is assigned before that, so a throw from the tail
    // is expected; a missing hook is not, and is reported below.
    if (!windowStub.__scry) throw e;
  }
  const scry = windowStub.__scry;
  if (!scry || typeof scry.readFrame !== "function") {
    throw new Error(
      "scanner.html did not expose window.__scry.readFrame. That hook is what " +
        "makes the reader testable without a camera; if it was removed, this " +
        "harness and the 210-of-210 figure both stop being reproducible."
    );
  }
  return scry;
}

// ---- the grid, from the encoder that ships -------------------------------

function gridFromEncoder(text) {
  const exe = process.platform === "win32" ? "chaos-qr.exe" : "chaos-qr";
  const bin = path.join(ROOT, "target", "release", exe);
  if (!fs.existsSync(bin)) {
    throw new Error(
      "no " + bin + " -- build it first:\n" +
        "  cargo build --release --bin chaos-qr\n" +
        "or pass a grid with --grid <file of 0/1 rows>."
    );
  }
  const out = execFileSync(bin, [text, "--ascii", "--quiet", "0"], { encoding: "utf8" });
  // Two characters per module. The code rows are the widest lines; the others
  // are the payload echo and the "version N, level Q" line.
  const lines = out.split("\n").map((l) => l.replace(/\r$/, ""));
  const widest = Math.max(...lines.map((l) => l.length));
  const rows = lines.filter((l) => l.length === widest);
  const grid = rows.map((r) => {
    let s = "";
    for (let i = 0; i < r.length; i += 2) s += r.slice(i, i + 2).trim() ? "1" : "0";
    return s;
  });
  if (!grid.length || grid.length !== grid[0].length) {
    throw new Error("chaos-qr output is not a square grid: " + grid.length + " rows");
  }
  return grid;
}

// ---- rasterise: grid -> greyscale pixels, rotated ------------------------

// Own sampler rather than a canvas, so this needs no DOM -- and so the reader
// is handed pixels rather than the grid it would otherwise sample directly.
function rasterise(grid, scale, degrees) {
  const n = grid.length, quiet = 4;
  const side = (n + 2 * quiet) * scale;
  const dim = Math.ceil(side * Math.SQRT2) + 2;
  const gray = new Uint8Array(dim * dim).fill(255);
  const rad = (-degrees * Math.PI) / 180;   // inverse rotation
  const cos = Math.cos(rad), sin = Math.sin(rad);
  const c = dim / 2, half = side / 2;
  for (let y = 0; y < dim; y++) {
    for (let x = 0; x < dim; x++) {
      const dx = x - c, dy = y - c;
      const sx = dx * cos - dy * sin + half;
      const sy = dx * sin + dy * cos + half;
      if (sx < 0 || sy < 0 || sx >= side || sy >= side) continue;
      const mx = Math.floor(sx / scale) - quiet;
      const my = Math.floor(sy / scale) - quiet;
      if (mx < 0 || my < 0 || mx >= n || my >= n) continue;
      if (grid[my][mx] === "1") gray[y * dim + x] = 0;
    }
  }
  return { gray, dim };
}

// ---- sweep ---------------------------------------------------------------

function main() {
  const reader = loadReader();
  const grid = GRID_FILE
    ? fs.readFileSync(GRID_FILE, "utf8").trim().split(/\r?\n/)
    : gridFromEncoder(TEXT);

  // The reader's own pure decoder must agree with the encoder's grid before any
  // pixels are involved. If this fails, nothing below means anything.
  const mod = grid.map((r) => [...r].map((ch) => ch === "1"));
  const pure = reader.decodeGrid(mod, grid.length);
  console.log("grid          " + grid.length + "x" + grid.length +
              "  (version " + (grid.length - 17) / 4 + ")");
  console.log("payload       " + JSON.stringify(TEXT));
  console.log("decodeGrid    " + JSON.stringify(pure) +
              (pure === TEXT || GRID_FILE ? "  <- reader agrees with encoder" : "  <- MISMATCH"));
  if (!GRID_FILE && pure !== TEXT) {
    console.error("\nthe reader and the encoder disagree on the grid itself, " +
                  "before any rasterising. Nothing else here is meaningful.");
    process.exit(1);
  }
  const want = GRID_FILE ? pure : TEXT;
  console.log("");

  let total = 0, pass = 0, notRead = 0, wrong = 0;
  const wrongCases = [], usableMisses = [];
  const header = "  px/mod   " + ANGLES.length + " angles      read   declined   WRONG";
  console.log(header);
  console.log("  " + "-".repeat(header.length - 2));
  for (const s of SCALES) {
    let ok = 0, no = 0, bad = 0;
    for (const a of ANGLES) {
      total++;
      const { gray, dim } = rasterise(grid, s, a);
      let got = null;
      try {
        const r = reader.readFrame(gray, dim, dim);
        got = r && r.text != null ? r.text : null;
      } catch (e) {
        got = "THREW: " + e.message;
      }
      if (got === want) { ok++; pass++; }
      else if (got == null) {
        no++; notRead++;
        if (s >= USABLE_FROM) usableMisses.push(s + "px/" + a + "deg");
      } else {
        bad++; wrong++;
        wrongCases.push(s + "px/" + a + "deg -> " + JSON.stringify(got));
      }
    }
    const mark = s < USABLE_FROM ? "  (below the usable floor)" : "";
    console.log(
      "  " + String(s).padStart(5) + "    " +
      String(ok + "/" + ANGLES.length).padStart(12) + "   " +
      String(ok).padStart(6) + "   " + String(no).padStart(8) + "   " +
      String(bad).padStart(5) + mark
    );
  }

  console.log("");
  console.log("total         " + pass + " of " + total + " read correctly");
  console.log("declined      " + notRead + "   (returned nothing -- the safe failure)");
  console.log("WRONG STRING  " + wrong + "   (returned something else -- never acceptable)");
  if (wrongCases.length) {
    console.log("");
    for (const c of wrongCases.slice(0, 20)) console.log("  " + c);
  }
  if (usableMisses.length) {
    console.log("");
    console.log("missed at or above " + USABLE_FROM + " px per module:");
    for (const c of usableMisses.slice(0, 30)) console.log("  " + c);
  }

  const bad = wrong > 0 || usableMisses.length > 0;
  console.log("");
  console.log(bad ? "FAIL" : "OK: every case at " + USABLE_FROM +
              " px per module and above read correctly, and nothing read wrong.");
  process.exit(bad ? 1 : 0);
}

main();
