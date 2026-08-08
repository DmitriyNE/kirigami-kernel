//! 3D rendering of the certified cone strip — diagnostics only.
//!
//! A [`CertifiedChart`] carries no floats; a browser needs vertices. This module samples the
//! chart's exact thickened surface `C(σ,μ,w)` over a `(σ,μ)` grid at the mid-surface `w = 0`,
//! flattens each point to `f64` through the quarantined [`approx`](crate::approx) bridge, and
//! packs the result into a [`StripMesh`] — grid vertices, two triangles per cell, the straight
//! ruling generators, and the certified `(σ,μ,w)` sub-box. [`cone_html`] wraps that mesh in a
//! self-contained Three.js viewer page (lit surface, orbit controls, ruling lines, a `(μ,w)`
//! parameter panel with the certified box highlighted). Nothing here ever feeds a predicate:
//! floats appear at the last moment, for display, exactly as [`approx`](crate::approx)
//! prescribes.
//!
//! # What is *not* here
//!
//! The rolled 3D strip is the **mid-surface** (`w = 0`) in its embedded pose. There is no
//! flat↔rolled morph: an exact isometric development is not yet implemented (`develop` is a
//! stub), so inventing a flat map for a slider would be a fiction, not a diagnostic. The true
//! wrap-onto-cone correspondence lands with the development layer.
//!
//! # Three.js delivery
//!
//! [`cone_html`] takes a [`ThreeSrc`]: [`ThreeSrc::Cdn`] emits a pinned `unpkg` import map (needs
//! the network), while [`ThreeSrc::Inline`] base64-encodes a vendored `three.module.js` +
//! `OrbitControls.js` into a `data:`-URL import map, so the page renders fully offline. The
//! `cone` example drives both (`--vendor <dir>` selects inline).

use crate::approx::{rat_to_f64, vec3_to_f64};
use geom::record::CertifiedChart;
use lattice::{Backend, Rat};

/// Default σ (rows) and μ (columns) sample counts for the `cone` example.
pub const DEFAULT_NSIG: usize = 48;
/// Default μ sample count (columns) for the `cone` example.
pub const DEFAULT_NMU: usize = 24;

/// The certified `(σ,μ,w)` sub-box, as `f64` display numbers — the region the chart's
/// regularity margins were proved over. Highlighted in the viewer (a gold band on the strip and
/// a filled rectangle in the `(μ,w)` panel).
#[derive(Clone, Copy, Debug)]
pub struct CertBox {
    /// The certified σ span `[lo, hi]`.
    pub sigma: [f64; 2],
    /// The certified ruling-offset μ box `[lo, hi]`.
    pub mu: [f64; 2],
    /// The certified thickness w box `[lo, hi]`.
    pub w: [f64; 2],
}

/// The device cone's mid-surface (`w = 0`) sampled to a float triangle mesh — a self-describing
/// bundle the Three.js viewer reads verbatim.
///
/// Vertices are row-major: index `i*ncols + j` is `(σ = sigmas[i], μ = mus[j])`, `i` over the σ
/// rows, `j` over the μ columns. The μ range is sampled *wider* than the certified μ box so the
/// certified region shows as a proper sub-band; σ is sampled over exactly the certified span.
#[derive(Clone, Debug)]
pub struct StripMesh {
    /// Grid vertices `[x, y, z]`, row-major (`i*ncols + j`).
    pub positions: Vec<[f64; 3]>,
    /// Two triangles per grid cell, as vertex-index triples.
    pub tris: Vec<[u32; 3]>,
    /// A decimated set of straight ruling generators (each a σ = const line), as endpoint pairs.
    pub rulings: Vec<[[f64; 3]; 2]>,
    /// The σ sample values (length `nrows`).
    pub sigmas: Vec<f64>,
    /// The μ sample values (length `ncols`).
    pub mus: Vec<f64>,
    /// Number of σ rows.
    pub nrows: usize,
    /// Number of μ columns.
    pub ncols: usize,
    /// The certified sub-box, highlighted in the viewer.
    pub certified: CertBox,
    /// The sampled μ extent `[lo, hi]` (wider than `certified.mu`).
    pub mu_range: [f64; 2],
    /// The displayed w extent `[lo, hi]` for the `(μ,w)` panel (wider than `certified.w`).
    pub w_range: [f64; 2],
    /// The mesh curvature cap `min(s_max, 1/κ₁)`, for the caption.
    pub kappa_cap: f64,
}

/// Sample the certified cone's mid-surface (`w = 0`) into a [`StripMesh`].
///
/// σ is sampled over the certified span `[sigma.lo, sigma.hi]` with `nsig` points; μ over the
/// certified μ box *padded by half its width on each side* with `nmu` points, so the certified
/// μ range is a centered sub-band of what is drawn. Each grid point is the exact surface value
/// `C(σ, μ, 0)` cast to `f64`. Panics if `nsig < 2` or `nmu < 2`.
///
/// ```
/// use export::mesh3d::sample_cone_strip;
/// use fixtures::devices::certified_cone;
///
/// let cert = certified_cone();
/// let mesh = sample_cone_strip(&cert, 6, 5);
/// assert_eq!(mesh.positions.len(), 30); // 6 σ rows × 5 μ columns
/// assert_eq!(mesh.tris.len(), 2 * 5 * 4); // two triangles per (5×4) cell
/// ```
pub fn sample_cone_strip<B: Backend>(
    cert: &CertifiedChart<B>,
    nsig: usize,
    nmu: usize,
) -> StripMesh {
    assert!(nsig >= 2 && nmu >= 2, "need at least a 2×2 grid");
    let chart = cert.chart();
    let domain = cert.domain();

    // σ over the certified span; μ over the certified box padded by half its width each side.
    let two = Rat::from_i128(2);
    let mu_pad = domain.mu.1.sub(&domain.mu.0).div(&two);
    let mu_lo = domain.mu.0.sub(&mu_pad);
    let mu_hi = domain.mu.1.add(&mu_pad);
    let sigmas_q = linspace(&domain.sigma.lo, &domain.sigma.hi, nsig);
    let mus_q = linspace(&mu_lo, &mu_hi, nmu);

    // Build each μ-column's σ-parametric surface once, then evaluate down the σ rows. The cone's
    // denominator is a power of 97(1+σ²) — never zero on ℝ — so every sample is defined.
    let w0 = Rat::from_i128(0);
    let columns: Vec<_> = mus_q.iter().map(|m| chart.surface(m, &w0)).collect();
    let (nrows, ncols) = (nsig, nmu);
    let mut positions = Vec::with_capacity(nrows * ncols);
    for s in &sigmas_q {
        for col in &columns {
            let p = col.eval(s).expect("cone surface is defined on σ ∈ [0, 1]");
            positions.push(vec3_to_f64(&p));
        }
    }

    // Two triangles per cell (consistent CCW winding; the viewer recomputes normals and lights
    // both faces, so winding only sets a convention here).
    let mut tris = Vec::with_capacity(2 * (nrows - 1) * (ncols - 1));
    for i in 0..nrows - 1 {
        for j in 0..ncols - 1 {
            let a = (i * ncols + j) as u32;
            let b = (i * ncols + j + 1) as u32;
            let c = ((i + 1) * ncols + j) as u32;
            let d = ((i + 1) * ncols + j + 1) as u32;
            tris.push([a, c, b]);
            tris.push([b, c, d]);
        }
    }

    // Rulings are the σ = const generators (straight, since the surface is affine in μ). Decimate
    // to a readable count; each is the full μ-span segment of its row.
    let nrul = nrows.min(13);
    let mut rulings = Vec::with_capacity(nrul);
    for k in 0..nrul {
        let i = k * (nrows - 1) / (nrul - 1);
        rulings.push([positions[i * ncols], positions[i * ncols + (ncols - 1)]]);
    }

    // The certified sub-box, and the wider display ranges the panel/band sit inside.
    let certified = CertBox {
        sigma: [rat_to_f64(&domain.sigma.lo), rat_to_f64(&domain.sigma.hi)],
        mu: [rat_to_f64(&domain.mu.0), rat_to_f64(&domain.mu.1)],
        w: [rat_to_f64(&domain.w.0), rat_to_f64(&domain.w.1)],
    };
    let w_pad = domain.w.1.sub(&domain.w.0).div(&two);
    let w_range = [
        rat_to_f64(&domain.w.0.sub(&w_pad)),
        rat_to_f64(&domain.w.1.add(&w_pad)),
    ];

    StripMesh {
        positions,
        tris,
        rulings,
        sigmas: sigmas_q.iter().map(rat_to_f64).collect(),
        mus: mus_q.iter().map(rat_to_f64).collect(),
        nrows,
        ncols,
        certified,
        mu_range: [rat_to_f64(&mu_lo), rat_to_f64(&mu_hi)],
        w_range,
        kappa_cap: rat_to_f64(cert.kappa_cap()),
    }
}

/// `n` exact samples from `lo` to `hi` inclusive (endpoints exact). `n ≥ 2` by the caller.
fn linspace<B: Backend>(lo: &Rat<B>, hi: &Rat<B>, n: usize) -> Vec<Rat<B>> {
    let span = hi.sub(lo);
    let denom = Rat::from_i128((n - 1) as i128);
    (0..n)
        .map(|i| {
            let frac = Rat::from_i128(i as i128).div(&denom);
            lo.add(&span.mul(&frac))
        })
        .collect()
}

// --- JSON payload ------------------------------------------------------------------------

/// A finite `f64` as JSON; non-finite (never produced for the cone) degrades to `0`.
fn num(x: f64) -> String {
    if x.is_finite() {
        format!("{x}")
    } else {
        "0".to_string()
    }
}

fn vec3(p: &[f64; 3]) -> String {
    format!("[{},{},{}]", num(p[0]), num(p[1]), num(p[2]))
}

fn join<T>(xs: &[T], f: impl Fn(&T) -> String) -> String {
    xs.iter().map(&f).collect::<Vec<_>>().join(",")
}

/// Serialize a [`StripMesh`] to the compact JSON the viewer reads (all display floats).
pub fn strip_json(m: &StripMesh) -> String {
    let positions = join(&m.positions, vec3);
    let tris = join(&m.tris, |t| format!("[{},{},{}]", t[0], t[1], t[2]));
    let rulings = join(&m.rulings, |seg| {
        format!("[{},{}]", vec3(&seg[0]), vec3(&seg[1]))
    });
    let sigmas = join(&m.sigmas, |s| num(*s));
    let mus = join(&m.mus, |s| num(*s));
    let cb = &m.certified;
    format!(
        "{{\"positions\":[{positions}],\"tris\":[{tris}],\"rulings\":[{rulings}],\
         \"sigmas\":[{sigmas}],\"mus\":[{mus}],\"nrows\":{nrows},\"ncols\":{ncols},\
         \"certified\":{{\"sigma\":[{cs0},{cs1}],\"mu\":[{cm0},{cm1}],\"w\":[{cw0},{cw1}]}},\
         \"muRange\":[{mr0},{mr1}],\"wRange\":[{wr0},{wr1}],\"kappaCap\":{kappa}}}",
        nrows = m.nrows,
        ncols = m.ncols,
        cs0 = num(cb.sigma[0]),
        cs1 = num(cb.sigma[1]),
        cm0 = num(cb.mu[0]),
        cm1 = num(cb.mu[1]),
        cw0 = num(cb.w[0]),
        cw1 = num(cb.w[1]),
        mr0 = num(m.mu_range[0]),
        mr1 = num(m.mu_range[1]),
        wr0 = num(m.w_range[0]),
        wr1 = num(m.w_range[1]),
        kappa = num(m.kappa_cap),
    )
}

// --- HTML page ---------------------------------------------------------------------------

/// Where the viewer loads Three.js from.
pub enum ThreeSrc {
    /// A pinned `unpkg` import map — smallest page, needs the network.
    Cdn,
    /// Fully inline via a `data:`-URL import map — a larger but offline page. Supply the verbatim
    /// contents of `three.module.js` and `examples/jsm/controls/OrbitControls.js`.
    Inline {
        /// Contents of `three.module.js`.
        three_module: String,
        /// Contents of `OrbitControls.js` (imports the bare specifier `three`, resolved by the map).
        orbit_controls: String,
    },
}

/// The pinned CDN version, used by [`ThreeSrc::Cdn`].
const THREE_VERSION: &str = "0.160.0";

/// Escape a string for HTML text/attribute context.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Standard base64 (RFC 4648) — used to inline JS modules as `data:` URLs. Integer-only.
fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// The import map for a [`ThreeSrc`] — CDN URLs or inline `data:` URLs.
fn import_map(three: &ThreeSrc) -> String {
    match three {
        ThreeSrc::Cdn => format!(
            "<script type=\"importmap\">{{\"imports\":{{\
             \"three\":\"https://unpkg.com/three@{v}/build/three.module.js\",\
             \"three/addons/\":\"https://unpkg.com/three@{v}/examples/jsm/\"}}}}</script>",
            v = THREE_VERSION,
        ),
        ThreeSrc::Inline {
            three_module,
            orbit_controls,
        } => format!(
            "<script type=\"importmap\">{{\"imports\":{{\
             \"three\":\"data:text/javascript;base64,{three}\",\
             \"three/addons/controls/OrbitControls.js\":\"data:text/javascript;base64,{orbit}\"}}}}</script>",
            three = b64(three_module.as_bytes()),
            orbit = b64(orbit_controls.as_bytes()),
        ),
    }
}

/// A 2D `(μ,w)` parameter panel: the sampled display box with the certified sub-box filled.
fn mu_w_svg(m: &StripMesh) -> String {
    let (w_px, h_px, pad) = (240.0_f64, 150.0_f64, 30.0_f64);
    let [mu0, mu1] = m.mu_range;
    let [w0, w1] = m.w_range;
    let mapx = |mu: f64| pad + (mu - mu0) / (mu1 - mu0) * (w_px - 2.0 * pad);
    let mapy = |w: f64| (h_px - pad) - (w - w0) / (w1 - w0) * (h_px - 2.0 * pad);
    // Outer display box.
    let (ox, oy) = (mapx(mu0), mapy(w1));
    let (ow, oh) = (mapx(mu1) - ox, mapy(w0) - oy);
    // Certified sub-box.
    let cb = &m.certified;
    let (cx, cy) = (mapx(cb.mu[0]), mapy(cb.w[1]));
    let (cw, ch) = (mapx(cb.mu[1]) - cx, mapy(cb.w[0]) - cy);
    let midy = mapy(0.0); // the sampled mid-surface w = 0
    format!(
        "<svg viewBox=\"0 0 {w_px} {h_px}\" width=\"100%\" role=\"img\" \
         aria-label=\"mu-w parameter box with certified sub-box\">\
         <rect x=\"{ox:.1}\" y=\"{oy:.1}\" width=\"{ow:.1}\" height=\"{oh:.1}\" \
         fill=\"#fff\" stroke=\"#cbd5e1\"/>\
         <rect x=\"{cx:.1}\" y=\"{cy:.1}\" width=\"{cw:.1}\" height=\"{ch:.1}\" \
         fill=\"#ffd166\" fill-opacity=\"0.55\" stroke=\"#d98a00\"/>\
         <line x1=\"{ox:.1}\" y1=\"{midy:.1}\" x2=\"{oxr:.1}\" y2=\"{midy:.1}\" \
         stroke=\"#5b8def\" stroke-dasharray=\"4 3\"/>\
         <text x=\"{txm:.1}\" y=\"{h_px}\" font-size=\"11\" text-anchor=\"middle\" \
         fill=\"#475569\">μ ∈ [{cm0:.2}, {cm1:.2}]</text>\
         <text x=\"8\" y=\"{tyw:.1}\" font-size=\"11\" fill=\"#475569\" \
         transform=\"rotate(-90 8 {tyw:.1})\">w ∈ [{cw0:.2}, {cw1:.2}]</text>\
         </svg>",
        oxr = ox + ow,
        txm = ox + ow / 2.0,
        tyw = oy + oh / 2.0,
        cm0 = cb.mu[0],
        cm1 = cb.mu[1],
        cw0 = cb.w[0],
        cw1 = cb.w[1],
    )
}

/// The viewer module: builds the scene from the embedded JSON, lights the strip, draws rulings,
/// fits the camera, and wires the display toggles. Reads `#strip-data`; no server needed.
const VIEWER_JS: &str = r#"
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

const D = JSON.parse(document.getElementById('strip-data').textContent);
const canvas = document.getElementById('view');

const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0f1117);

// --- surface geometry ---
const pos = new Float32Array(D.positions.length * 3);
D.positions.forEach((p, i) => { pos[3*i] = p[0]; pos[3*i+1] = p[1]; pos[3*i+2] = p[2]; });
const idx = new Uint32Array(D.tris.length * 3);
D.tris.forEach((t, i) => { idx[3*i] = t[0]; idx[3*i+1] = t[1]; idx[3*i+2] = t[2]; });

const geo = new THREE.BufferGeometry();
geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
geo.setIndex(new THREE.BufferAttribute(idx, 1));
geo.computeVertexNormals();
geo.computeBoundingSphere();

// Per-vertex color: gold inside the certified (σ,μ) band, blue elsewhere.
const base = new THREE.Color(0x5b8def), hot = new THREE.Color(0xffd166);
const [cs0, cs1] = D.certified.sigma, [cm0, cm1] = D.certified.mu;
const eps = 1e-9;
const banded = new Float32Array(D.positions.length * 3);
const plain = new Float32Array(D.positions.length * 3);
for (let v = 0; v < D.positions.length; v++) {
  const i = Math.floor(v / D.ncols), j = v % D.ncols;
  const inBand = D.sigmas[i] >= cs0 - eps && D.sigmas[i] <= cs1 + eps
              && D.mus[j]   >= cm0 - eps && D.mus[j]   <= cm1 + eps;
  const c = inBand ? hot : base;
  banded[3*v] = c.r; banded[3*v+1] = c.g; banded[3*v+2] = c.b;
  plain[3*v] = base.r; plain[3*v+1] = base.g; plain[3*v+2] = base.b;
}
geo.setAttribute('color', new THREE.BufferAttribute(banded, 3));

const mat = new THREE.MeshStandardMaterial({
  vertexColors: true, metalness: 0.1, roughness: 0.6, side: THREE.DoubleSide,
});
const mesh = new THREE.Mesh(geo, mat);
scene.add(mesh);

// --- ruling generators ---
const rpos = new Float32Array(D.rulings.length * 6);
D.rulings.forEach((s, i) => {
  rpos[6*i] = s[0][0]; rpos[6*i+1] = s[0][1]; rpos[6*i+2] = s[0][2];
  rpos[6*i+3] = s[1][0]; rpos[6*i+4] = s[1][1]; rpos[6*i+5] = s[1][2];
});
const rgeo = new THREE.BufferGeometry();
rgeo.setAttribute('position', new THREE.BufferAttribute(rpos, 3));
const rulings = new THREE.LineSegments(
  rgeo, new THREE.LineBasicMaterial({ color: 0xe8ecf4, transparent: true, opacity: 0.55 }));
scene.add(rulings);

// --- lights ---
scene.add(new THREE.HemisphereLight(0xffffff, 0x2a3444, 0.7));
const key = new THREE.DirectionalLight(0xffffff, 0.9); key.position.set(4, 5, 6); scene.add(key);
const fill = new THREE.DirectionalLight(0xbcd0ff, 0.4); fill.position.set(-5, -3, -4); scene.add(fill);

// --- camera + controls, fit to the strip ---
const bs = geo.boundingSphere;
const cam = new THREE.PerspectiveCamera(45, 1, 0.01, 1000);
const r = Math.max(bs.radius, 1e-3);
cam.position.set(bs.center.x + r * 1.8, bs.center.y + r * 1.3, bs.center.z + r * 2.2);
const controls = new OrbitControls(cam, canvas);
controls.target.copy(bs.center);
controls.enableDamping = true;

function resize() {
  const w = canvas.clientWidth, h = canvas.clientHeight;
  if (canvas.width !== w || canvas.height !== h) {
    renderer.setSize(w, h, false);
    cam.aspect = w / Math.max(h, 1); cam.updateProjectionMatrix();
  }
}
new ResizeObserver(resize).observe(canvas);
resize();

function tick() {
  controls.update();
  renderer.render(scene, cam);
  requestAnimationFrame(tick);
}
tick();

// --- toggles ---
const bind = (id, fn) => document.getElementById(id).addEventListener('change', e => fn(e.target.checked));
bind('rulings', on => { rulings.visible = on; });
bind('wire', on => { mat.wireframe = on; });
bind('highlight', on => {
  geo.setAttribute('color', new THREE.BufferAttribute(on ? banded : plain, 3));
  geo.attributes.color.needsUpdate = true;
});
"#;

/// Minimal self-contained page CSS — a 3D canvas beside a parameter/legend sidebar.
const CSS: &str = "\
:root{color-scheme:light}\
*{box-sizing:border-box}\
body{font:15px/1.5 system-ui,sans-serif;margin:0;color:#1a1a1a;background:#fafafc}\
header{padding:1rem 1.25rem .25rem}\
h1{font-size:1.25rem;margin:0}\
.lede{max-width:52rem;color:#555;font-size:.9rem;margin:.35rem 0 0}\
main{display:flex;gap:1rem;padding:1rem 1.25rem;flex-wrap:wrap}\
.stage{flex:1 1 26rem;min-height:65vh;border:1px solid #e2e2ea;border-radius:10px;\
background:#0f1117;overflow:hidden}\
#view{width:100%;height:100%;display:block}\
aside{flex:0 0 17rem;display:flex;flex-direction:column;gap:1rem}\
.card{border:1px solid #e2e2ea;border-radius:10px;padding:1rem;background:#fff}\
.card h2{font-size:1rem;margin:0 0 .5rem}\
dl{margin:0;display:grid;grid-template-columns:auto 1fr;gap:.15rem .5rem;font-size:.85rem}\
dt{color:#667}dd{margin:0;font-variant-numeric:tabular-nums}\
label{display:block;font-size:.9rem;margin:.15rem 0}\
code{background:#eef;padding:0 .2em;border-radius:3px}\
.swatch{display:inline-block;width:.8em;height:.8em;border-radius:2px;vertical-align:baseline}";

/// Assemble the self-contained cone-strip viewer page: a lit Three.js render of the certified
/// cone's mid-surface with its ruling generators, beside a `(μ,w)` panel and legend that mark the
/// certified sub-box. Returns a complete HTML document.
pub fn cone_html(title: &str, mesh: &StripMesh, three: &ThreeSrc) -> String {
    let cb = &mesh.certified;
    let panel = mu_w_svg(mesh);
    let json = strip_json(mesh);
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title}</title><style>{CSS}</style>{map}</head><body>\
         <header><h1>{title}</h1><p class=\"lede\">The device cone's certified mid-surface \
         (<code>w = 0</code>), sampled exactly through the quarantined exact→<code>f64</code> \
         bridge and rolled into its embedded 3D pose. The \
         <span class=\"swatch\" style=\"background:#ffd166\"></span> gold band and box mark the \
         <strong>certified</strong> sub-domain. The flat↔rolled morph lands in a later phase.</p>\
         </header><main>\
         <div class=\"stage\"><canvas id=\"view\"></canvas></div>\
         <aside>\
         <section class=\"card\"><h2>(μ, w) domain</h2>{panel}</section>\
         <section class=\"card\"><h2>Certified sub-box</h2><dl>\
         <dt>σ</dt><dd>[{cs0}, {cs1}]</dd>\
         <dt>μ</dt><dd>[{cm0}, {cm1}]</dd>\
         <dt>w</dt><dd>[{cw0}, {cw1}]</dd>\
         <dt>κ-cap</dt><dd>{kappa:.5}</dd></dl></section>\
         <section class=\"card\"><h2>Display</h2>\
         <label><input type=\"checkbox\" id=\"rulings\" checked> ruling generators</label>\
         <label><input type=\"checkbox\" id=\"highlight\" checked> certified band</label>\
         <label><input type=\"checkbox\" id=\"wire\"> wireframe</label></section>\
         </aside></main>\
         <script type=\"application/json\" id=\"strip-data\">{json}</script>\
         <script type=\"module\">{VIEWER_JS}</script>\
         </body></html>",
        title = esc(title),
        map = import_map(three),
        cs0 = cb.sigma[0],
        cs1 = cb.sigma[1],
        cm0 = cb.mu[0],
        cm1 = cb.mu[1],
        cw0 = cb.w[0],
        cw1 = cb.w[1],
        kappa = mesh.kappa_cap,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::devices::certified_cone;

    fn mesh(nsig: usize, nmu: usize) -> StripMesh {
        sample_cone_strip(&certified_cone(), nsig, nmu)
    }

    #[test]
    fn grid_shape_is_rows_by_cols() {
        let m = mesh(5, 4);
        assert_eq!((m.nrows, m.ncols), (5, 4));
        assert_eq!(m.positions.len(), 20);
        assert_eq!(m.sigmas.len(), 5);
        assert_eq!(m.mus.len(), 4);
        assert_eq!(m.tris.len(), 2 * 4 * 3); // two per (4×3) cell
        assert!(!m.rulings.is_empty());
        // Every triangle index is in range.
        for t in &m.tris {
            for &v in t {
                assert!((v as usize) < m.positions.len());
            }
        }
    }

    #[test]
    fn certified_box_matches_device_domain() {
        let m = mesh(6, 6);
        // Device cone: σ∈[0,1], μ∈[−1,−1/2], w∈[−1/4,1/4], κ-cap = 65/194.
        assert_eq!(m.certified.sigma, [0.0, 1.0]);
        assert_eq!(m.certified.mu, [-1.0, -0.5]);
        assert_eq!(m.certified.w, [-0.25, 0.25]);
        assert!((m.kappa_cap - 65.0 / 194.0).abs() < 1e-12);
        // The certified μ box is a proper sub-band of the sampled μ range.
        assert!(m.mu_range[0] < m.certified.mu[0] && m.certified.mu[1] < m.mu_range[1]);
        // The displayed w range strictly contains the certified w box.
        assert!(m.w_range[0] < m.certified.w[0] && m.certified.w[1] < m.w_range[1]);
    }

    #[test]
    fn sampled_points_are_finite_and_nondegenerate() {
        let m = mesh(24, 12);
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in &m.positions {
            for k in 0..3 {
                assert!(p[k].is_finite(), "surface point must be finite");
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        // The rolled strip has real extent in every axis (it is not flat or collapsed).
        for k in 0..3 {
            assert!(hi[k] - lo[k] > 1e-6, "axis {k} is degenerate");
        }
    }

    #[test]
    fn rulings_are_straight_row_segments() {
        let m = mesh(8, 6);
        // Each ruling endpoint pair spans the μ extent of a row: distinct, nonzero length.
        for [a, b] in &m.rulings {
            let d2: f64 = (0..3).map(|k| (a[k] - b[k]).powi(2)).sum();
            assert!(d2 > 1e-9, "ruling must have nonzero length");
        }
    }

    #[test]
    fn json_carries_the_mesh() {
        let m = mesh(4, 3);
        let j = strip_json(&m);
        assert!(j.starts_with('{') && j.ends_with('}'));
        for key in [
            "\"positions\"",
            "\"tris\"",
            "\"rulings\"",
            "\"sigmas\"",
            "\"mus\"",
            "\"certified\"",
            "\"kappaCap\"",
        ] {
            assert!(j.contains(key), "JSON missing {key}");
        }
        assert!(
            !j.contains("NaN") && !j.contains("inf"),
            "JSON must be finite"
        );
    }

    #[test]
    fn cdn_html_is_self_contained_and_annotated() {
        let m = mesh(6, 5);
        let html = cone_html("cone", &m, &ThreeSrc::Cdn);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("type=\"importmap\""));
        assert!(html.contains("unpkg.com/three@"));
        assert!(html.contains("OrbitControls.js"));
        assert!(html.contains("id=\"view\""));
        assert!(html.contains("id=\"strip-data\""));
        assert!(html.contains("Certified sub-box"));
        assert!(html.contains("<svg")); // the (μ,w) panel
    }

    #[test]
    fn inline_html_embeds_data_urls_not_cdn() {
        let m = mesh(4, 3);
        let html = cone_html(
            "cone",
            &m,
            &ThreeSrc::Inline {
                three_module: "export const THREE_STUB = 1;".to_string(),
                orbit_controls: "import 'three'; export const OC = 2;".to_string(),
            },
        );
        assert!(html.contains("data:text/javascript;base64,"));
        assert!(
            !html.contains("unpkg.com"),
            "inline page must not reach the CDN"
        );
        assert!(html.contains("OrbitControls.js"));
    }

    #[test]
    fn base64_matches_known_vectors() {
        // RFC 4648 test vectors (padding included).
        assert_eq!(b64(b"Man"), "TWFu");
        assert_eq!(b64(b"Ma"), "TWE=");
        assert_eq!(b64(b"M"), "TQ==");
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn mu_w_panel_is_well_formed() {
        let m = mesh(5, 5);
        let svg = mu_w_svg(&m);
        assert!(svg.starts_with("<svg"));
        assert_eq!(svg.matches("<rect").count(), 2, "outer box + certified box");
        assert!(svg.contains("μ ∈") && svg.contains("w ∈"));
    }
}
