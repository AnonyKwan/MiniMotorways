// Small generative soundtrack made with Web Audio: a note for every delivery,
// each colour in its own register of one pentatonic scale, and a low detuned horn as a warning.

const SCALE = [0, 2, 4, 7, 9]; // major pentatonic, in semitones
const ROOTS = [0, 12, 7, 19, 2, 14]; // register / colour offsets from C3

/** Frequency of the n-th note of a colour's scale. */
export function noteFreq(group, step) {
  const octave = Math.floor(step / SCALE.length);
  const semis = ROOTS[group % ROOTS.length] + SCALE[((step % SCALE.length) + SCALE.length) % SCALE.length] + 12 * octave;
  return 130.81 * Math.pow(2, semis / 12);
}

export class Sound {
  constructor() {
    this.ctx = null;
    this.muted = false;
    this.step = 0;
    this.lastWarn = -10;
  }

  /** Must be called from a user gesture before anything is audible. */
  unlock() {
    if (!this.ctx) {
      const AC = typeof window !== 'undefined' && (window.AudioContext || window.webkitAudioContext);
      if (!AC) return;
      try { this.ctx = new AC(); this.master = this.ctx.createGain(); this.master.gain.value = 0.5; this.master.connect(this.ctx.destination); } catch (_) { this.ctx = null; }
    }
    if (this.ctx && this.ctx.state === 'suspended') this.ctx.resume();
  }

  /** Quiet evolving drone; `p` is the worst destination's timer share (0..1). */
  bed(p, nowSeconds) {
    if (!this.ctx || this.muted) return;
    if (!this.bedNodes) {
      const mk = (f, type, g) => {
        const o = this.ctx.createOscillator(); const gn = this.ctx.createGain();
        o.type = type; o.frequency.value = f; gn.gain.value = 0;
        o.connect(gn); gn.connect(this.master); o.start();
        return { o, gn };
      };
      this.bedNodes = { a: mk(110, 'sine'), b: mk(164.8, 'sine'), low: mk(55, 'sawtooth') };
    }
    const t = this.ctx.currentTime;
    const wob = 0.5 + 0.5 * Math.sin(nowSeconds * 0.21);
    this.bedNodes.a.gn.gain.setTargetAtTime(0.012 + 0.01 * wob, t, 0.8);
    this.bedNodes.b.gn.gain.setTargetAtTime(0.008 + 0.008 * (1 - wob), t, 0.8);
    // below the game over line the warning drone fades in
    this.bedNodes.low.gn.gain.setTargetAtTime(p > 0.8 ? 0.03 * Math.min(1, (p - 0.8) / 0.2) : 0, t, 0.4);
  }

  setMuted(m) { this.muted = m; if (this.master) this.master.gain.value = m ? 0 : 0.5; }

  tone(freq, dur, type, gain, when = 0, detune = 0) {
    if (!this.ctx || this.muted) return;
    const t = this.ctx.currentTime + when;
    const o = this.ctx.createOscillator();
    const g = this.ctx.createGain();
    o.type = type; o.frequency.value = freq; o.detune.value = detune;
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(gain, t + 0.015);
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    o.connect(g); g.connect(this.master);
    o.start(t); o.stop(t + dur + 0.05);
  }

  /** A pin was delivered. */
  deliver(group) {
    this.step = (this.step + 1 + (this.step % 3)) % 15;
    this.tone(noteFreq(group, this.step), 0.7, 'sine', 0.09);
    this.tone(noteFreq(group, this.step) * 2, 0.35, 'triangle', 0.02, 0.01);
  }

  /** Something new was built or picked. */
  click() { this.tone(520, 0.08, 'triangle', 0.04); }

  /** A destination is close to overflowing. Rate-limited. */
  warn(nowSeconds, pressure = 0.5) {
    const gap = 3.6 - 2.6 * Math.min(1, pressure);
    if (nowSeconds - this.lastWarn < gap) return;
    this.lastWarn = nowSeconds;
    const detune = 14 + 40 * Math.min(1, pressure); // the horn goes more out of tune as it gets worse
    const gain = 0.03 + 0.05 * Math.min(1, pressure);
    this.tone(146, 0.5, 'sawtooth', gain, 0, -detune);
    this.tone(146, 0.5, 'sawtooth', gain, 0, detune);
  }
}
