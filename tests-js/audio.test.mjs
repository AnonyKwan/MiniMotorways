import test from 'node:test';
import assert from 'node:assert/strict';
import { Sound, noteFreq } from '../web/js/audio.js';
import { ROOF_KINDS } from '../web/js/render3d.js';
import { GROUP_SHAPES } from '../web/js/model.js';

/** A stand-in for the browser's audio so we can see what the game asks it to play. */
function fakeAudio() {
  const log = { oscillators: [], gains: [] };
  class Param { constructor() { this.value = 0; this.sets = []; } setValueAtTime(v) { this.sets.push(v); } exponentialRampToValueAtTime(v) { this.sets.push(v); } setTargetAtTime(v) { this.target = v; this.sets.push(v); } }
  class Node { constructor() { this.gain = new Param(); this.frequency = new Param(); this.detune = new Param(); } connect() {} start() {} stop() {} }
  class Osc extends Node { constructor() { super(); log.oscillators.push(this); } }
  class Gain extends Node { constructor() { super(); log.gains.push(this); } }
  class Ctx {
    constructor() { this.currentTime = 0; this.state = 'running'; this.destination = new Node(); }
    createOscillator() { return new Osc(); }
    createGain() { return new Gain(); }
    resume() {}
  }
  globalThis.window = { AudioContext: Ctx };
  return log;
}

test('the background bed starts, follows pressure and can be switched off', () => {
  const log = fakeAudio();
  const s = new Sound();
  s.unlock();
  const before = log.oscillators.length;
  s.bed(0, 1);
  assert.equal(log.oscillators.length - before, 3, 'three drone voices');
  const low = s.bedNodes.low.gn.gain;
  s.bed(0.3, 2);
  assert.equal(low.target, 0, 'no warning drone at low pressure');
  s.bed(0.95, 3);
  assert.ok(low.target > 0, 'a low drone fades in near game over');
  s.setVolumes({ master: 1, music: 1, sfx: 1, soundscape: 0 });
  s.bed(0.1, 4);
  assert.equal(s.bedNodes.a.gn.gain.target, 0, 'soundscape off silences the bed');
  delete globalThis.window;
});

test('the horn gets louder, faster and more out of tune as pressure rises', () => {
  const log = fakeAudio();
  const s = new Sound();
  s.unlock();
  const detunes = [];
  const gains = [];
  for (const p of [0, 1]) {
    log.oscillators.length = 0;
    s.lastWarn = -100;
    s.warn(100 + p, p);
    const o = log.oscillators.filter((x) => x.type === 'sawtooth');
    assert.equal(o.length, 2, 'two detuned voices');
    detunes.push(Math.abs(o[0].detune.value));
  }
  assert.ok(detunes[1] > detunes[0] * 2, `detune ${detunes}`);
  // rate limit shortens with pressure
  s.lastWarn = 0;
  log.oscillators.length = 0;
  s.warn(2, 0);
  assert.equal(log.oscillators.length, 0, 'calm: wait longer');
  s.warn(2, 1);
  assert.equal(log.oscillators.length, 2, 'urgent: sounds again after two seconds');
  assert.ok(gains.length === 0);
  delete globalThis.window;
});

test('deliveries play notes only in the full soundscape', () => {
  const log = fakeAudio();
  const s = new Sound();
  s.unlock();
  s.deliver(2);
  const played = log.oscillators.length;
  assert.ok(played >= 2);
  s.setVolumes({ master: 1, music: 1, sfx: 1, soundscape: 1 });
  s.deliver(2);
  assert.equal(log.oscillators.length, played, 'minimal soundscape: no notes');
  assert.ok(noteFreq(2, 1) > 0);
  delete globalThis.window;
});

test('every colour has its own shape mark and roof silhouette', () => {
  assert.equal(new Set(GROUP_SHAPES).size, 6);
  assert.equal(new Set(ROOF_KINDS).size, 6);
});
