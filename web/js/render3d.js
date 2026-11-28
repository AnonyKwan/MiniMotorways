// A small WebGL2 renderer for the town: instanced low-poly houses, shops, cars, trees, hills,
// roads, bridges and rails, all generated here in code. Original shapes, no outside assets.
import { GROUP_COLORS } from './model.js';
import { cellHash } from './render.js';

// ---------- tiny matrix library (column-major) ----------
const M = {
  persp(fovy, aspect, n, f) {
    const t = 1 / Math.tan(fovy / 2);
    return new Float32Array([t / aspect, 0, 0, 0, 0, t, 0, 0, 0, 0, (f + n) / (n - f), -1, 0, 0, (2 * f * n) / (n - f), 0]);
  },
  lookAt(e, c, u) {
    let zx = e[0] - c[0], zy = e[1] - c[1], zz = e[2] - c[2];
    let l = Math.hypot(zx, zy, zz) || 1; zx /= l; zy /= l; zz /= l;
    let xx = u[1] * zz - u[2] * zy, xy = u[2] * zx - u[0] * zz, xz = u[0] * zy - u[1] * zx;
    l = Math.hypot(xx, xy, xz) || 1; xx /= l; xy /= l; xz /= l;
    const yx = zy * xz - zz * xy, yy = zz * xx - zx * xz, yz = zx * xy - zy * xx;
    return new Float32Array([xx, yx, zx, 0, xy, yy, zy, 0, xz, yz, zz, 0, -(xx * e[0] + xy * e[1] + xz * e[2]), -(yx * e[0] + yy * e[1] + yz * e[2]), -(zx * e[0] + zy * e[1] + zz * e[2]), 1]);
  },
  mul(a, b) {
    const o = new Float32Array(16);
    for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) {
      let s = 0;
      for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k];
      o[c * 4 + r] = s;
    }
    return o;
  },
  inv(m) {
    const a = m, o = new Float32Array(16);
    const b00 = a[0] * a[5] - a[1] * a[4], b01 = a[0] * a[6] - a[2] * a[4], b02 = a[0] * a[7] - a[3] * a[4];
    const b03 = a[1] * a[6] - a[2] * a[5], b04 = a[1] * a[7] - a[3] * a[5], b05 = a[2] * a[7] - a[3] * a[6];
    const b06 = a[8] * a[13] - a[9] * a[12], b07 = a[8] * a[14] - a[10] * a[12], b08 = a[8] * a[15] - a[11] * a[12];
    const b09 = a[9] * a[14] - a[10] * a[13], b10 = a[9] * a[15] - a[11] * a[13], b11 = a[10] * a[15] - a[11] * a[14];
    let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
    if (!det) return o;
    det = 1 / det;
    o[0] = (a[5] * b11 - a[6] * b10 + a[7] * b09) * det; o[1] = (a[2] * b10 - a[1] * b11 - a[3] * b09) * det;
    o[2] = (a[13] * b05 - a[14] * b04 + a[15] * b03) * det; o[3] = (a[10] * b04 - a[9] * b05 - a[11] * b03) * det;
    o[4] = (a[6] * b08 - a[4] * b11 - a[7] * b07) * det; o[5] = (a[0] * b11 - a[2] * b08 + a[3] * b07) * det;
    o[6] = (a[14] * b02 - a[12] * b05 - a[15] * b01) * det; o[7] = (a[8] * b05 - a[10] * b02 + a[11] * b01) * det;
    o[8] = (a[4] * b10 - a[5] * b08 + a[7] * b06) * det; o[9] = (a[1] * b08 - a[0] * b10 - a[3] * b06) * det;
    o[10] = (a[12] * b04 - a[13] * b02 + a[15] * b00) * det; o[11] = (a[9] * b02 - a[8] * b04 - a[11] * b00) * det;
    o[12] = (a[5] * b07 - a[4] * b09 - a[6] * b06) * det; o[13] = (a[0] * b09 - a[1] * b07 + a[2] * b06) * det;
    o[14] = (a[13] * b01 - a[12] * b03 - a[14] * b00) * det; o[15] = (a[8] * b03 - a[9] * b01 + a[10] * b00) * det;
    return o;
  },
  apply(m, x, y, z, w = 1) {
    return [m[0] * x + m[4] * y + m[8] * z + m[12] * w, m[1] * x + m[5] * y + m[9] * z + m[13] * w, m[2] * x + m[6] * y + m[10] * z + m[14] * w, m[3] * x + m[7] * y + m[11] * z + m[15] * w];
  },
};

// ---------- unit meshes: interleaved position(3) + normal(3), base at y=0, height 1 ----------
function pushTri(out, a, b, c) {
  const ux = b[0] - a[0], uy = b[1] - a[1], uz = b[2] - a[2];
  const vx = c[0] - a[0], vy = c[1] - a[1], vz = c[2] - a[2];
  let nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
  const l = Math.hypot(nx, ny, nz) || 1; nx /= l; ny /= l; nz /= l;
  for (const p of [a, b, c]) out.push(p[0], p[1], p[2], nx, ny, nz);
}
const quad = (out, a, b, c, d) => { pushTri(out, a, b, c); pushTri(out, a, c, d); };

function boxMesh() {
  const o = [];
  const x = 0.5, z = 0.5;
  quad(o, [-x, 0, z], [x, 0, z], [x, 1, z], [-x, 1, z]);
  quad(o, [x, 0, -z], [-x, 0, -z], [-x, 1, -z], [x, 1, -z]);
  quad(o, [x, 0, z], [x, 0, -z], [x, 1, -z], [x, 1, z]);
  quad(o, [-x, 0, -z], [-x, 0, z], [-x, 1, z], [-x, 1, -z]);
  quad(o, [-x, 1, z], [x, 1, z], [x, 1, -z], [-x, 1, -z]);
  return o;
}
function gableMesh() { // a roof: ridge along x, 1 wide in z, 1 high
  const o = [];
  const x = 0.5, z = 0.5;
  quad(o, [-x, 0, z], [x, 0, z], [x, 1, 0], [-x, 1, 0]);
  quad(o, [x, 0, -z], [-x, 0, -z], [-x, 1, 0], [x, 1, 0]);
  pushTri(o, [x, 0, z], [x, 0, -z], [x, 1, 0]);
  pushTri(o, [-x, 0, -z], [-x, 0, z], [-x, 1, 0]);
  return o;
}
function coneMesh(n, rTop = 0) {
  const o = [];
  for (let i = 0; i < n; i++) {
    const a0 = (i / n) * Math.PI * 2 + (n === 4 ? Math.PI / 4 : 0), a1 = ((i + 1) / n) * Math.PI * 2 + (n === 4 ? Math.PI / 4 : 0);
    const p0 = [Math.cos(a0) * 0.5, 0, Math.sin(a0) * 0.5], p1 = [Math.cos(a1) * 0.5, 0, Math.sin(a1) * 0.5];
    const t0 = [Math.cos(a0) * rTop * 0.5, 1, Math.sin(a0) * rTop * 0.5], t1 = [Math.cos(a1) * rTop * 0.5, 1, Math.sin(a1) * rTop * 0.5];
    if (rTop === 0) pushTri(o, p1, p0, [0, 1, 0]);
    else { quad(o, p1, p0, t0, t1); pushTri(o, [0, 1, 0], t1, t0); }
  }
  return o;
}
const cylMesh = (n) => coneMesh(n, 1);
function domeMesh(lat = 4, lon = 10) {
  const o = [];
  const pt = (i, j) => {
    const th = (i / lat) * (Math.PI / 2), ph = (j / lon) * Math.PI * 2;
    return [Math.cos(th) * Math.cos(ph) * 0.5, Math.sin(th), Math.cos(th) * Math.sin(ph) * 0.5];
  };
  for (let i = 0; i < lat; i++) for (let j = 0; j < lon; j++) quad(o, pt(i, j + 1), pt(i, j), pt(i + 1, j), pt(i + 1, j + 1));
  return o;
}
function sphereMesh(lat = 6, lon = 10) {
  const o = [];
  const pt = (i, j) => {
    const th = -Math.PI / 2 + (i / lat) * Math.PI, ph = (j / lon) * Math.PI * 2;
    return [Math.cos(th) * Math.cos(ph) * 0.5, 0.5 + Math.sin(th) * 0.5, Math.cos(th) * Math.sin(ph) * 0.5];
  };
  for (let i = 0; i < lat; i++) for (let j = 0; j < lon; j++) quad(o, pt(i, j + 1), pt(i, j), pt(i + 1, j), pt(i + 1, j + 1));
  return o;
}

/** One roof shape per colour, so colour never carries the meaning alone. */
export const ROOF_KINDS = ['gable', 'flat', 'pyramid', 'dome', 'spire', 'tower'];

const VS = `#version 300 es
in vec3 aPos; in vec3 aNorm;
in vec4 aM0; in vec4 aM1; in vec4 aM2; in vec4 aM3; in vec4 aCol;
uniform mat4 uVP;
out vec3 vN; out vec4 vCol; out float vY; out vec2 vXZ;
void main() {
  mat4 m = mat4(aM0, aM1, aM2, aM3);
  vec4 w = m * vec4(aPos, 1.0);
  vN = normalize(mat3(m) * aNorm);
  vCol = aCol; vY = w.y; vXZ = w.xz;
  gl_Position = uVP * w;
}`;
const FS = `#version 300 es
precision mediump float;
in vec3 vN; in vec4 vCol; in float vY; in vec2 vXZ;
uniform vec4 uRegion; uniform vec3 uFog;
uniform vec3 uSun; uniform vec3 uSky; uniform vec3 uGround; uniform float uAmb; uniform float uSunI;
out vec4 o;
void main() {
  vec3 n = normalize(vN);
  float d = max(dot(n, normalize(uSun)), 0.0);
  vec3 hemi = mix(uGround, uSky, n.y * 0.5 + 0.5);
  float ao = mix(0.78, 1.0, clamp(vY * 1.6, 0.0, 1.0));
  vec3 c = vCol.rgb * (hemi * uAmb + vec3(1.0, 0.97, 0.9) * d * uSunI) * ao;
  float fd = min(min(vXZ.x - uRegion.x, uRegion.z - vXZ.x), min(vXZ.y - uRegion.y, uRegion.w - vXZ.y));
  c = mix(mix(c, uFog, 0.78), c, smoothstep(-0.5, 0.5, fd));
  o = vec4(c, vCol.a);
}`;
const GVS = `#version 300 es
in vec2 aP; uniform mat4 uVP; uniform vec2 uSize; out vec2 vUv; out vec2 vXZ;
void main() { vUv = aP; vXZ = aP * uSize; gl_Position = uVP * vec4(aP.x * uSize.x, 0.0, aP.y * uSize.y, 1.0); }`;
const GFS = `#version 300 es
precision mediump float;
in vec2 vUv; in vec2 vXZ; uniform sampler2D uTex; uniform float uLight; uniform vec4 uRegion; uniform vec3 uFog; out vec4 o;
void main() {
  vec3 c = texture(uTex, vUv).rgb * uLight;
  float d = min(min(vXZ.x - uRegion.x, uRegion.z - vXZ.x), min(vXZ.y - uRegion.y, uRegion.w - vXZ.y));
  c = mix(mix(c, uFog, 0.78), c, smoothstep(-0.5, 0.5, d));
  o = vec4(c, 1.0);
}`;

function compile(gl, type, src) {
  const s = gl.createShader(type);
  gl.shaderSource(s, src); gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
  return s;
}
function program(gl, vs, fs) {
  const p = gl.createProgram();
  gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, vs));
  gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  return p;
}

let webglProbe = null;
/** Checks once (and releases the probe context so browsers do not run out of them). */
export function webglAvailable() {
  if (webglProbe !== null) return webglProbe;
  try {
    const c = document.createElement('canvas');
    const gl = c.getContext('webgl2');
    webglProbe = Boolean(gl);
    if (gl) { const ext = gl.getExtension('WEBGL_lose_context'); if (ext) ext.loseContext(); }
  } catch (_) { webglProbe = false; }
  return webglProbe;
}

const hex = (h) => { const n = parseInt(h.slice(1), 16); return [(n >> 16) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255]; };
const shade = (c, k) => [Math.min(1, c[0] * k), Math.min(1, c[1] * k), Math.min(1, c[2] * k)];

/** Collects instances per mesh and uploads them in one go. */
class Batch {
  constructor() { this.lists = {}; }
  clear() { for (const k in this.lists) this.lists[k].length = 0; }
  add(mesh, x, y, z, sx, sy, sz, rot, col, a = 1) {
    const c = Math.cos(rot), s = Math.sin(rot);
    // model = T * Ry(rot) * S
    (this.lists[mesh] || (this.lists[mesh] = [])).push(
      c * sx, 0, -s * sx, 0, 0, sy, 0, 0, s * sz, 0, c * sz, 0, x, y, z, 1, col[0], col[1], col[2], a,
    );
  }
}

export class Renderer3D {
  constructor(canvas) {
    const gl = canvas.getContext('webgl2', { antialias: true, alpha: false, preserveDrawingBuffer: true });
    if (!gl) throw new Error('WebGL2 is not available');
    this.canvas = canvas; this.gl = gl;
    this.prog = program(gl, VS, FS);
    this.gprog = program(gl, GVS, GFS);
    this.u = {};
    for (const n of ['uVP', 'uSun', 'uSky', 'uGround', 'uAmb', 'uSunI', 'uRegion', 'uFog']) this.u[n] = gl.getUniformLocation(this.prog, n);
    this.gu = { uVP: gl.getUniformLocation(this.gprog, 'uVP'), uSize: gl.getUniformLocation(this.gprog, 'uSize'), uTex: gl.getUniformLocation(this.gprog, 'uTex'), uLight: gl.getUniformLocation(this.gprog, 'uLight'), uRegion: gl.getUniformLocation(this.gprog, 'uRegion'), uFog: gl.getUniformLocation(this.gprog, 'uFog') };
    const meshes = { box: boxMesh(), gable: gableMesh(), cyl: cylMesh(14), cone: coneMesh(10), pyramid: coneMesh(4), dome: domeMesh(), sphere: sphereMesh(), prism3: cylMesh(3) };
    this.meshes = {};
    this.instBuf = gl.createBuffer();
    for (const [name, data] of Object.entries(meshes)) {
      const vao = gl.createVertexArray();
      gl.bindVertexArray(vao);
      const vb = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, vb);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STATIC_DRAW);
      const aPos = gl.getAttribLocation(this.prog, 'aPos'), aNorm = gl.getAttribLocation(this.prog, 'aNorm');
      gl.enableVertexAttribArray(aPos); gl.vertexAttribPointer(aPos, 3, gl.FLOAT, false, 24, 0);
      gl.enableVertexAttribArray(aNorm); gl.vertexAttribPointer(aNorm, 3, gl.FLOAT, false, 24, 12);
      this.meshes[name] = { vao, count: data.length / 6, ibuf: gl.createBuffer() };
    }
    // ground quad
    this.gvao = gl.createVertexArray();
    gl.bindVertexArray(this.gvao);
    const gb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, gb);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1]), gl.STATIC_DRAW);
    const gp = gl.getAttribLocation(this.gprog, 'aP');
    gl.enableVertexAttribArray(gp); gl.vertexAttribPointer(gp, 2, gl.FLOAT, false, 8, 0);
    gl.bindVertexArray(null);
    this.tex = gl.createTexture();
    this.stat = new Batch(); this.dyn = new Batch(); this.shadow = new Batch(); this.shadowNature = new Batch();
    this.W = 1; this.H = 1; this.dpr = 1;
    this.vp = M.mul(M.persp(1, 1, 1, 10), M.lookAt([0, 5, 5], [0, 0, 0], [0, 1, 0]));
    this.region = [0, 0, 10, 10];
    this.zoom = 1; this.pan = [0, 0];
    this.top = 64; this.bottom = 96;
    this.themeName = 'light';
    this.fovy = 0.56; this.pitch = 0.92;
  }

  resize(W, H, dpr) {
    this.W = W; this.H = H; this.dpr = dpr;
    this.canvas.width = Math.round(W * dpr); this.canvas.height = Math.round(H * dpr);
  }

  /** Ground texture and the things that never move: hills and trees. */
  setTerrain(w, h, bytes, layerCanvas, themeName) {
    const gl = this.gl;
    this.w = w; this.h = h; this.terrain = bytes; this.themeName = themeName;
    gl.bindTexture(gl.TEXTURE_2D, this.tex);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, layerCanvas);
    gl.generateMipmap(gl.TEXTURE_2D);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    this.buildNature();
  }

  palette() {
    const dark = this.themeName === 'dark';
    return {
      hill: hex(dark ? '#4a4d55' : '#b9ad94'), hillDark: hex(dark ? '#33353b' : '#9b8f78'), snow: hex(dark ? '#aeb7c4' : '#fbf8f0'),
      trunk: hex(dark ? '#4b3b2c' : '#8a6b4a'), leaf: hex(dark ? '#2f5a46' : '#6fae7f'), leaf2: hex(dark ? '#3b6a55' : '#8fc79a'),
      asphalt: hex(dark ? '#4c5561' : '#59616d'), edge: hex(dark ? '#232830' : '#3d444e'), deck: hex(dark ? '#8b95a3' : '#f1ecde'),
      slab: hex(dark ? '#38414c' : '#cfcabb'), ink: hex(dark ? '#e9edf2' : '#2b2f36'), white: [1, 1, 1], shadow: [0, 0, 0],
      rail: hex(dark ? '#8b95a3' : '#a9a28e'), sleeper: hex(dark ? '#4b3b2c' : '#8a6b4a'), grass: hex(dark ? '#27382f' : '#a9c98a'),
    };
  }

  buildNature() {
    const { w, h, terrain } = this;
    const p = this.palette();
    this.shadowNature.clear();
    this.natureBatch = new Batch();
    const nb = this.natureBatch;
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const byte = terrain[y * w + x];
        if ((byte & 3) === 2) {
          const peaks = 1 + Math.floor(cellHash(x, y, 8) * 2.4);
          for (let k = 0; k < peaks; k++) {
            const px = x + 0.25 + 0.5 * cellHash(x, y, 9 + k), pz = y + 0.25 + 0.5 * cellHash(x, y, 12 + k);
            const r = 0.75 + 0.5 * cellHash(x, y, 15 + k), ht = 0.7 + 0.9 * cellHash(x, y, 18 + k);
            nb.add('cone', px, 0, pz, r, ht, r, cellHash(x, y, 30 + k) * 3, k % 2 ? p.hill : p.hillDark);
            if (ht > 1.15) nb.add('cone', px, ht * 0.7, pz, r * 0.42, ht * 0.3, r * 0.42, 0, p.snow);
          }
        } else if (byte & 4) {
          const px = x + 0.5 + 0.1 * (cellHash(x, y, 21) - 0.5), pz = y + 0.5;
          const r = 0.34 + 0.12 * cellHash(x, y, 22), kind = cellHash(x, y, 23);
          nb.add('cyl', px, 0, pz, 0.08, 0.28, 0.08, 0, p.trunk);
          if (kind < 0.5) nb.add('sphere', px, 0.2, pz, r * 1.6, r * 1.6, r * 1.6, 0, kind < 0.25 ? p.leaf : p.leaf2);
          else { nb.add('cone', px, 0.2, pz, r * 1.5, 0.55, r * 1.5, 0, p.leaf); nb.add('cone', px, 0.45, pz, r * 1.1, 0.45, r * 1.1, 0, p.leaf2); }
          this.shadowNature.add('cyl', px + 0.1, 0.004, pz + 0.12, r * 1.2, 0.001, r * 0.9, 0, p.shadow, 0.22);
        }
      }
    }
  }

  /** Roads, bridges, rails and junction furniture; rebuilt only when the town changes. */
  setStatics(statics) {
    this.statics = statics;
    const { w, terrain } = this;
    const p = this.palette();
    const s = this.stat;
    s.clear();
    this.shadow.clear();
    const kindAt = (c) => terrain[c] & 3;
    const xz = (c) => [(c % w) + 0.5, Math.floor(c / w) + 0.5];
    const nodes = new Set();
    for (const [a, b] of statics.roads || []) {
      const [ax, az] = xz(a), [bx, bz] = xz(b);
      const dx = bx - ax, dz = bz - az, len = Math.hypot(dx, dz), rot = Math.atan2(-dz, dx);
      const mx = (ax + bx) / 2, mz = (az + bz) / 2;
      const wet = kindAt(a) === 1 || kindAt(b) === 1, hill = kindAt(a) === 2 || kindAt(b) === 2;
      const y0 = wet ? 0.28 : 0;
      nodes.add(a); nodes.add(b);
      const half = (kindAt(a) === 1) !== (kindAt(b) === 1);
      if (wet) {
        if (half) {
          const landIsA = kindAt(a) !== 1;
          const [lx, lz] = landIsA ? [ax, az] : [bx, bz];
          const [wx2, wz2] = landIsA ? [bx, bz] : [ax, az];
          for (const [t0, t1, yy] of [[0, 0.5, 0.12], [0.5, 1, y0]]) {
            const m = (t0 + t1) / 2;
            const px = lx + (wx2 - lx) * m, pz = lz + (wz2 - lz) * m;
            s.add('box', px, yy - 0.04, pz, len * (t1 - t0) + 0.02, 0.1, 0.62, rot, p.deck);
            s.add('box', px, yy + 0.06, pz, len * (t1 - t0) + 0.02, 0.04, 0.46, rot, p.asphalt);
          }
        } else {
          s.add('box', mx, y0 - 0.04, mz, len + 0.04, 0.1, 0.62, rot, p.deck);
          s.add('box', mx, y0 + 0.06, mz, len + 0.02, 0.04, 0.46, rot, p.asphalt);
        }
        if (kindAt(a) === 1 && kindAt(b) === 1 || len > 1.2) {
          for (const k of [-0.5, 0.5]) {
            const rx = mx + Math.sin(rot) * 0.3 * k * 2, rz = mz + Math.cos(rot) * 0.3 * k * 2;
            s.add('box', rx, y0 + 0.06, rz, len, 0.1, 0.03, rot, p.deck);
          }
        }
        s.add('cyl', ax, 0, az, 0.12, y0 - 0.04, 0.12, 0, p.deck);
        this.shadow.add('box', mx + 0.1, 0.004, mz + 0.14, len, 0.001, 0.5, rot, p.shadow, 0.18);
      } else if (hill) {
        s.add('box', mx, 0, mz, len + 0.02, 0.1, 0.62, rot, p.edge);
        s.add('box', mx, 0.1, mz, len + 0.02, 0.02, 0.4, rot, p.asphalt);
      } else {
        s.add('box', mx, 0, mz, len + 0.02, 0.05, 0.56, rot, p.edge);
        s.add('box', mx, 0.05, mz, len + 0.02, 0.012, 0.42, rot, p.asphalt);
        s.add('box', mx, 0.062, mz, len * 0.9, 0.004, 0.025, rot, [1, 1, 1], 0.55);
      }
    }
    for (const c of nodes) {
      const [x, z] = xz(c);
      const wet = kindAt(c) === 1;
      const y0 = wet ? 0.28 : 0;
      if (wet) s.add('cyl', x, y0 + 0.02, z, 0.5, 0.06, 0.5, 0, p.asphalt);
      else { s.add('cyl', x, 0, z, 0.56, 0.05, 0.56, 0, p.edge); s.add('cyl', x, 0.05, z, 0.42, 0.012, 0.42, 0, p.asphalt); }
    }
    // motorways: raised ribbons on pillars
    for (const [a, b] of statics.mw || []) {
      const [ax, az] = xz(a), [bx, bz] = xz(b);
      const dx = bx - ax, dz = bz - az, len = Math.hypot(dx, dz), rot = Math.atan2(-dz, dx);
      const hgt = 0.95;
      s.add('box', (ax + bx) / 2, hgt - 0.12, (az + bz) / 2, len, 0.14, 0.62, rot, p.deck);
      s.add('box', (ax + bx) / 2, hgt + 0.02, (az + bz) / 2, len, 0.05, 0.48, rot, p.asphalt);
      const n = Math.max(2, Math.round(len / 1.6));
      for (let i = 0; i <= n; i++) {
        const t = i / n;
        s.add('cyl', ax + dx * t, 0, az + dz * t, 0.2, hgt - 0.12, 0.2, 0, p.deck);
      }
      for (const [x, z, sign] of [[ax, az, 1], [bx, bz, -1]]) s.add('cyl', x, 0, z, 0.5, 0.06, 0.5, 0, p.edge);
      this.shadow.add('box', (ax + bx) / 2 + 0.25, 0.004, (az + bz) / 2 + 0.35, len, 0.001, 0.55, rot, p.shadow, 0.2);
    }
    for (const c of statics.rounds || []) {
      const [x, z] = xz(c);
      s.add('cyl', x, 0, z, 1.5, 0.06, 1.5, 0, p.edge);
      s.add('cyl', x, 0.06, z, 1.38, 0.012, 1.38, 0, p.asphalt);
      s.add('cyl', x, 0.07, z, 0.72, 0.1, 0.72, 0, p.deck);
      s.add('cyl', x, 0.15, z, 0.62, 0.02, 0.62, 0, p.grass);
      s.add('sphere', x, 0.15, z, 0.3, 0.3, 0.3, 0, p.leaf);
    }
    for (const c of statics.lights || []) {
      const [x, z] = xz(c);
      s.add('cyl', x + 0.3, 0, z - 0.3, 0.05, 0.55, 0.05, 0, p.ink);
      s.add('box', x + 0.3, 0.5, z - 0.3, 0.14, 0.28, 0.12, 0, p.ink);
    }
    this.bumpStatic = (this.bumpStatic || 0) + 1;
  }

  // ---------- camera ----------
  setView(region, zoom, pan) { this.region = region; this.zoom = zoom; this.pan = pan; }

  buildCamera(shift = [0, 0]) {
    const [x0, y0, x1, y1] = this.region;
    const cx = (x0 + x1) / 2 + shift[0], cz = (y0 + y1) / 2 + shift[1];
    const aspect = this.W / this.H;
    const proj = M.persp(this.fovy, aspect, 0.5, 400);
    const corners = [[x0, y0], [x1, y0], [x0, y1], [x1, y1]];
    const eyeFor = (d) => [cx, Math.sin(this.pitch) * d, cz + Math.cos(this.pitch) * d];
    const fits = (d) => {
      const vp = M.mul(proj, M.lookAt(eyeFor(d), [cx, 0, cz], [0, 1, 0]));
      let ok = true;
      for (const [x, z] of corners) {
        const c = M.apply(vp, x, 0, z);
        const px = ((c[0] / c[3]) * 0.5 + 0.5) * this.W, py = (1 - (c[1] / c[3]) * 0.5 - 0.5) * this.H;
        if (px < 8 || px > this.W - 8 || py < this.top || py > this.H - this.bottom) ok = false;
      }
      return ok;
    };
    let lo = 4, hi = 300;
    for (let i = 0; i < 24; i++) { const mid = (lo + hi) / 2; if (fits(mid)) hi = mid; else lo = mid; }
    const d = hi / Math.max(0.3, this.zoom);
    const eye = eyeFor(d);
    const view = M.lookAt(eye, [cx, 0, cz], [0, 1, 0]);
    return { vp: M.mul(proj, view), eye, center: [cx, cz] };
  }

  updateCamera() {
    let cam = this.buildCamera();
    this.baseVP = cam.vp;
    if (this.pan[0] || this.pan[1]) {
      const inv = M.inv(cam.vp);
      const hit = (px, py) => this.groundHit(inv, px, py);
      const a = hit(this.W / 2, this.H / 2), b = hit(this.W / 2 - this.pan[0], this.H / 2 - this.pan[1]);
      if (a && b) cam = this.buildCamera([b[0] - a[0], b[1] - a[1]]);
    }
    this.vp = cam.vp; this.eye = cam.eye; this.invVP = M.inv(cam.vp);
  }

  groundHit(inv, px, py) {
    const nx = (px / this.W) * 2 - 1, ny = 1 - (py / this.H) * 2;
    const a = M.apply(inv, nx, ny, -1), b = M.apply(inv, nx, ny, 1);
    const p0 = [a[0] / a[3], a[1] / a[3], a[2] / a[3]], p1 = [b[0] / b[3], b[1] / b[3], b[2] / b[3]];
    const dy = p1[1] - p0[1];
    if (Math.abs(dy) < 1e-6) return null;
    const t = -p0[1] / dy;
    return [p0[0] + (p1[0] - p0[0]) * t, p0[2] + (p1[2] - p0[2]) * t];
  }

  cellAt(px, py) {
    if (!this.invVP) return null;
    const g = this.groundHit(this.invVP, px, py);
    if (!g) return null;
    const x = Math.floor(g[0]), y = Math.floor(g[1]);
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return null;
    return y * this.w + x;
  }

  project(x, y, z) {
    const c = M.apply(this.vp, x, y, z);
    if (c[3] <= 0) return null;
    return [((c[0] / c[3]) * 0.5 + 0.5) * this.W, (1 - ((c[1] / c[3]) * 0.5 + 0.5)) * this.H];
  }

  cellCenterPx(cell) {
    return this.project((cell % this.w) + 0.5, 0, Math.floor(cell / this.w) + 0.5);
  }

  /** Pixels per cell at a ground point (for sizing 2D overlays). */
  pxPerCell(x, z) {
    const a = this.project(x, 0, z), b = this.project(x + 1, 0, z);
    return a && b ? Math.abs(b[0] - a[0]) : 20;
  }

  // ---------- drawing ----------
  drawBatch(batch, blend) {
    const gl = this.gl;
    for (const [name, list] of Object.entries(batch.lists)) {
      if (!list.length) continue;
      const mesh = this.meshes[name];
      gl.bindVertexArray(mesh.vao);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.instBuf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(list), gl.DYNAMIC_DRAW);
      const locs = ['aM0', 'aM1', 'aM2', 'aM3', 'aCol'].map((n) => gl.getAttribLocation(this.prog, n));
      locs.forEach((l, i) => {
        gl.enableVertexAttribArray(l);
        gl.vertexAttribPointer(l, 4, gl.FLOAT, false, 80, i * 16);
        gl.vertexAttribDivisor(l, 1);
      });
      gl.drawArraysInstanced(gl.TRIANGLES, 0, mesh.count, list.length / 20);
    }
    gl.bindVertexArray(null);
  }

  /** @param scene {{statics, snap, born: Map, region: number[]}} */
  draw(scene, now) {
    const gl = this.gl;
    const dark = this.themeName === 'dark';
    this.updateCamera();
    const bg = hex(dark ? '#12161c' : '#ece5d6');
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(bg[0], bg[1], bg[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.disable(gl.CULL_FACE);

    // ground
    gl.useProgram(this.gprog);
    gl.uniformMatrix4fv(this.gu.uVP, false, this.vp);
    gl.uniform2f(this.gu.uSize, this.w, this.h);
    gl.uniform1f(this.gu.uLight, dark ? 0.85 : 1.0);
    const fullMap = this.noFog;
    const rg = fullMap ? [-50, -50, this.w + 50, this.h + 50] : this.region;
    gl.uniform4f(this.gu.uRegion, rg[0], rg[1], rg[2], rg[3]);
    gl.uniform3f(this.gu.uFog, bg[0], bg[1], bg[2]);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.tex);
    gl.uniform1i(this.gu.uTex, 0);
    gl.bindVertexArray(this.gvao);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindVertexArray(null);

    gl.useProgram(this.prog);
    gl.uniformMatrix4fv(this.u.uVP, false, this.vp);
    gl.uniform3f(this.u.uSun, -0.45, 0.8, 0.5);
    gl.uniform3f(this.u.uSky, ...(dark ? [0.5, 0.58, 0.75] : [0.92, 0.95, 1.0]));
    gl.uniform3f(this.u.uGround, ...(dark ? [0.28, 0.3, 0.34] : [0.72, 0.68, 0.6]));
    gl.uniform1f(this.u.uAmb, dark ? 0.62 : 0.74);
    gl.uniform1f(this.u.uSunI, dark ? 0.38 : 0.52);
    gl.uniform4f(this.u.uRegion, rg[0], rg[1], rg[2], rg[3]);
    gl.uniform3f(this.u.uFog, bg[0], bg[1], bg[2]);

    this.buildDynamic(scene, now);
    // flat shadows first (blended, no depth write)
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.depthMask(false);
    this.drawBatch(this.shadow);
    this.drawBatch(this.shadowNature);
    this.drawBatch(this.dynShadow);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    this.drawBatch(this.natureBatch);
    this.drawBatch(this.stat);
    this.drawBatch(this.dyn);
  }

  buildDynamic(scene, now) {
    const { statics, snap, born } = scene;
    const { w } = this;
    const p = this.palette();
    const cols = GROUP_COLORS[this.themeName].map(hex);
    const d = this.dyn, sh = (this.dynShadow = this.dynShadow || new Batch());
    d.clear(); sh.clear();
    const xz = (c) => [(c % w) + 0.5, Math.floor(c / w) + 0.5];
    const night = this.themeName === 'dark';
    const glass = night ? [1, 0.86, 0.45] : [0.36, 0.52, 0.66];
    const lamp = night ? [1, 0.95, 0.7] : [1, 0.97, 0.8];
    // a point on a building, given in the building's own (turned) frame
    const at = (x, z, lx, lz, rot) => [x + Math.cos(rot) * lx + Math.sin(rot) * lz, z - Math.sin(rot) * lx + Math.cos(rot) * lz];
    const pop = (key) => {
      if (!born || this.reduceMotion) return 1;
      if (!born.has(key)) born.set(key, now);
      const t = (now - born.get(key)) / 0.5;
      if (t >= 1) return 1;
      const u = Math.max(0, t) - 1;
      return Math.max(0.02, 1 + 2.70158 * u * u * u + 1.70158 * u * u);
    };

    // silhouettes: every colour gets its own roof shape
    const roof = (g, x, y, z, scale, col, rot) => {
      const dk = shade(col, 0.82);
      switch (ROOF_KINDS[g % 6]) {
        case 'gable': d.add('gable', x, y, z, scale, scale * 0.55, scale * 0.95, rot, dk); break;
        case 'flat': d.add('box', x, y, z, scale * 1.02, scale * 0.12, scale * 1.02, rot, dk); d.add('box', x - scale * 0.2, y + scale * 0.12, z, scale * 0.3, scale * 0.2, scale * 0.3, rot, dk); break;
        case 'pyramid': d.add('pyramid', x, y, z, scale * 1.35, scale * 0.7, scale * 1.35, rot, dk); break;
        case 'dome': d.add('dome', x, y, z, scale * 1.05, scale * 0.9, scale * 1.05, rot, dk); break;
        case 'spire': d.add('cyl', x, y, z, scale * 0.9, scale * 0.15, scale * 0.9, rot, dk); d.add('cone', x, y + scale * 0.15, z, scale * 0.9, scale * 0.6, scale * 0.9, rot, dk); break;
        default: d.add('box', x, y, z, scale * 0.9, scale * 0.12, scale * 0.9, rot, dk); d.add('box', x, y + scale * 0.12, z, scale * 0.6, scale * 0.14, scale * 0.6, rot, shade(col, 0.7)); d.add('cyl', x, y + scale * 0.26, z, scale * 0.08, scale * 0.35, scale * 0.08, rot, p.white);
      }
    };

    (statics.houses || []).forEach(([c, g], i) => {
      const [x, z] = xz(c), k = pop('h' + i), col = cols[g];
      const rot = (cellHash(c % w, Math.floor(c / w), 3) < 0.5 ? 0 : Math.PI / 2);
      d.add('box', x, 0, z, 0.66 * k, 0.4 * k, 0.6 * k, rot, col);
      roof(g, x, 0.4 * k, z, 0.72 * k, col, rot);
      for (const lx of [-0.17, 0.17]) { const [wx, wz] = at(x, z, lx * k, 0.31 * k, rot); d.add('box', wx, 0.17 * k, wz, 0.12 * k, 0.1 * k, 0.03, rot, glass); }
      { const [dx2, dz2] = at(x, z, 0, 0.31 * k, rot); d.add('box', dx2, 0, dz2, 0.1 * k, 0.16 * k, 0.03, rot, shade(col, 0.5)); }
      sh.add('box', x + 0.14, 0.004, z + 0.18, 0.7 * k, 0.001, 0.62 * k, 0, p.shadow, 0.2);
    });

    (statics.dests || []).forEach(([c, g, level, kind], i) => {
      const x0 = c % w, z0 = Math.floor(c / w), cx = x0 + 1, cz = z0 + 1, k = pop('d' + i), col = cols[g];
      d.add('box', cx, 0, cz, 1.92, 0.04, 1.92, 0, p.slab);
      for (let j = 0; j < 5; j++) d.add('box', x0 + 0.3 + j * 0.35, 0.04, z0 + 1.62, 0.03, 0.005, 0.3, 0, [1, 1, 1], 0.9);
      const bw = 1.3 * k;
      if (level > 0) {
        d.add('cyl', cx, 0.04, cz - 0.15, bw * 0.95, 0.85 * k, bw * 0.95, 0, col);
        d.add('dome', cx, 0.04 + 0.85 * k, cz - 0.15, bw * 0.98, 0.5 * k, bw * 0.98, 0, shade(col, 0.8));
      } else {
        d.add('box', cx, 0.04, cz - 0.15, bw, 0.8 * k, bw, 0, col);
        roof(g, cx, 0.04 + 0.8 * k, cz - 0.15, bw * 1.05, col, 0);
      }
      d.add('box', cx, 0.04, cz + 0.52 * k, bw * 0.8, 0.14 * k, 0.1, 0, [1, 1, 1], 1); // awning
      for (let wi = -1; wi <= 1; wi++) d.add('box', cx + wi * 0.36 * k, 0.34 * k, cz - 0.15 + bw / 2 + 0.01, 0.22 * k, 0.2 * k, 0.03, 0, glass);
      if (kind === 1) { d.add('box', cx, 0.04, cz + 0.72, 1.4, 0.2, 0.18, 0, p.ink); d.add('cyl', cx - 0.45, 0.04, cz + 0.72, 0.1, 0.2, 0.1, 0, p.white); d.add('cyl', cx + 0.45, 0.04, cz + 0.72, 0.1, 0.2, 0.1, 0, p.white); }
      if (kind === 2) { d.add('box', cx, 0.04, cz + 0.72, 1.5, 0.12, 0.2, 0, p.deck); d.add('cyl', cx + 0.8, 0.04, cz + 0.7, 0.1, 0.3, 0.1, 0, p.ink); }
      sh.add('box', cx + 0.22, 0.004, cz + 0.1, 2.0, 0.001, 1.95, 0, p.shadow, 0.2);
    });

    // rails, trains, boats
    const kindAt = (c) => this.terrain[c] & 3;
    for (let x = 0; x < this.w; x++) {
      if (statics.railY >= 0) {
        const z = statics.railY + 0.5;
        d.add('box', x + 0.5, 0, z, 0.14, 0.03, 0.8, 0, p.sleeper);
        if (x % 1 === 0) { d.add('box', x + 0.5, 0.03, z - 0.24, 1.02, 0.04, 0.06, 0, p.rail); d.add('box', x + 0.5, 0.03, z + 0.24, 1.02, 0.04, 0.06, 0, p.rail); }
      }
    }
    for (const [kind, tx, ty, dir] of snap.transit || []) {
      if (kind === 0) {
        for (let k = 0; k < 3; k++) {
          const x = tx - dir * k * 1.05;
          d.add('box', x, 0.06, ty, 0.98, 0.42, 0.5, 0, k === 0 ? p.ink : p.deck);
          d.add('box', x, 0.3, ty, 0.7, 0.1, 0.52, 0, [0.35, 0.55, 0.75]);
          if (k === 0) d.add('cyl', x + dir * 0.25, 0.48, ty, 0.2, 0.18, 0.2, 0, p.ink);
          sh.add('box', x + 0.12, 0.004, ty + 0.14, 1.0, 0.001, 0.5, 0, p.shadow, 0.2);
        }
      } else {
        d.add('box', tx, 0.02, ty, 1.2 * Math.sign(dir || 1), 0.3, 0.55, 0, p.ink);
        d.add('box', tx, 0.32, ty, 0.5, 0.25, 0.4, 0, p.deck);
        d.add('cyl', tx - 0.1, 0.57, ty, 0.12, 0.15, 0.12, 0, [0.9, 0.35, 0.3]);
        sh.add('box', tx + 0.1, 0.003, ty + 0.14, 1.2, 0.001, 0.6, 0, [0, 0.1, 0.2], 0.25);
      }
    }
    if (statics.boat && statics.boat.length === 3) {
      const [yb, a, b] = statics.boat;
      for (let x = a; x <= b; x += 2) d.add('box', x + 0.5, 0.01, yb + 0.5, 0.5, 0.01, 0.05, 0, [1, 1, 1], 0.7);
    }

    // cars
    const mw = (statics.mw || []).map(([a, b]) => [xz(a), xz(b)]);
    for (const [x, z, dx, dz, g, parked] of snap.cars || []) {
      if (parked) continue;
      let y = 0.0;
      if (this.terrain[Math.floor(z) * w + Math.floor(x)] & 1) y = 0.34;
      for (const [[ax, az], [bx, bz]] of mw) {
        const vx = bx - ax, vz = bz - az, l2 = vx * vx + vz * vz || 1;
        const t = Math.max(0, Math.min(1, ((x - ax) * vx + (z - az) * vz) / l2));
        if (Math.hypot(ax + vx * t - x, az + vz * t - z) < 0.12) y = 1.0;
      }
      const rot = Math.atan2(-dz, dx), col = cols[g];
      d.add('box', x, y + 0.04, z, 0.42, 0.13, 0.24, rot, col);
      d.add('box', x - Math.cos(rot) * 0.02, y + 0.17, z + Math.sin(rot) * 0.02, 0.22, 0.1, 0.2, rot, shade(col, 1.15));
      d.add('box', x, y + 0.2, z, 0.12, 0.05, 0.18, rot, [0.15, 0.2, 0.28]);
      for (const lz of [-0.08, 0.08]) { const [hx, hz] = at(x, z, 0.215, lz, rot); d.add('box', hx, y + 0.07, hz, 0.03, 0.04, 0.05, rot, lamp); }
      sh.add('box', x + 0.05, 0.005, z + 0.07, 0.44, 0.001, 0.26, rot, p.shadow, 0.25);
    }
  }
}
