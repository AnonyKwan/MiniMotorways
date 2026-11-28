// Pause screen, options, and the saved game card on the map list.
import { setColorblind } from './model.js';
import { loadPref, savePref } from './storage.js';
import { describe, validSave } from './savegame.js';

export const DEFAULT_OPTIONS = { master: 0.8, music: 0.8, sfx: 0.8, soundscape: 2, colorblind: false, quality: 1, reduceMotion: false };
export const QUALITY_LABELS = { 0.75: 'Lower (faster)', 1: 'Standard', 1.5: 'High', 2: 'Sharpest' };
export const SOUNDSCAPE_LABELS = ['Off', 'Minimal', 'Full'];

const el = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; };

export function normaliseOptions(o) {
  const d = { ...DEFAULT_OPTIONS, ...(o && typeof o === 'object' ? o : {}) };
  const clamp = (v) => Math.max(0, Math.min(1, Number(v)));
  d.master = clamp(d.master); d.music = clamp(d.music); d.sfx = clamp(d.sfx);
  d.soundscape = [0, 1, 2].includes(d.soundscape) ? d.soundscape : 2;
  d.quality = [0.75, 1, 1.5, 2].includes(d.quality) ? d.quality : 1;
  d.colorblind = Boolean(d.colorblind); d.reduceMotion = Boolean(d.reduceMotion);
  return d;
}

export function initOptions(app, api) {
  app.options = normaliseOptions(loadPref('options', null));
  app.paused = false;

  function apply() {
    const o = app.options;
    setColorblind(o.colorblind);
    app.sound.setVolumes(o);
    document.body.classList.toggle('calm', o.reduceMotion);
    app.reduceMotion = o.reduceMotion;
    api.setQuality(o.quality);
    api.refreshColours();
    savePref('options', o);
  }

  // ---------- options sheet ----------
  const root = el('section', 'overlay'); root.id = 'options'; root.hidden = true;
  const sheet = el('div', 'sheet wide');
  const body = el('div', 'scroll');
  sheet.append(el('h2', null, 'Options'), body);
  const row = el('div', 'row'); const close = el('button', 'primary', 'Done'); close.id = 'btn-options-close';
  row.appendChild(close); sheet.appendChild(row); root.appendChild(sheet); document.body.appendChild(root);

  const slider = (label, key) => {
    const w = el('label', 'opt'); w.append(el('span', null, label));
    const i = el('input'); i.type = 'range'; i.min = '0'; i.max = '1'; i.step = '0.05'; i.value = String(app.options[key]); i.dataset.opt = key;
    i.addEventListener('input', () => { app.options[key] = Number(i.value); apply(); });
    w.appendChild(i); return w;
  };
  const choice = (label, key, values, names) => {
    const w = el('label', 'opt'); w.append(el('span', null, label));
    const s = el('select'); s.dataset.opt = key;
    values.forEach((v, k) => { const o = el('option', null, names[k]); o.value = String(v); s.appendChild(o); });
    s.value = String(app.options[key]);
    s.addEventListener('change', () => { app.options[key] = Number(s.value); apply(); });
    w.appendChild(s); return w;
  };
  const toggle = (label, key, hint) => {
    const w = el('label', 'opt'); const t = el('span', null, label);
    if (hint) t.append(el('small', null, ' ' + hint));
    const c = el('input'); c.type = 'checkbox'; c.checked = app.options[key]; c.dataset.opt = key;
    c.addEventListener('change', () => { app.options[key] = c.checked; apply(); });
    w.append(t, c); return w;
  };
  function buildOptions() {
    body.textContent = '';
    body.append(el('h3', null, 'Sound'), slider('Master volume', 'master'), slider('Music and notes', 'music'), slider('Effects and horn', 'sfx'),
      choice('Soundscape', 'soundscape', [0, 1, 2], SOUNDSCAPE_LABELS));
    body.append(el('h3', null, 'Display'), choice('3D resolution', 'quality', [0.75, 1, 1.5, 2], Object.values(QUALITY_LABELS)),
      toggle('Colour-blind colours', 'colorblind', 'safer colours; shapes are always on'), toggle('Reduce motion', 'reduceMotion', 'no pop-in or camera glide'));
    const night = el('label', 'opt'); night.append(el('span', null, 'Night mode'));
    const nc = el('input'); nc.type = 'checkbox'; nc.checked = app.themeName === 'dark'; nc.id = 'opt-night';
    nc.addEventListener('change', () => api.setTheme(nc.checked ? 'dark' : 'light'));
    night.appendChild(nc); body.appendChild(night);
    const reset = el('button', null, 'Delete saved progress'); reset.id = 'btn-reset';
    reset.addEventListener('click', () => {
      if (!confirm('Delete best scores, achievements, challenge results and the saved game on this device?')) return;
      for (const k of ['best', 'progress', 'challengeBest', 'challengeDone', 'save', 'helpSeen']) { try { localStorage.removeItem('littlelanes.' + k); } catch (_) { /* ignore */ } }
      location.reload();
    });
    body.append(el('h3', null, 'Data'), reset);
  }
  close.addEventListener('click', () => { root.hidden = true; });

  // ---------- pause sheet ----------
  const pause = el('section', 'overlay'); pause.id = 'pause'; pause.hidden = true;
  const ps = el('div', 'sheet');
  ps.append(el('h2', null, 'Paused'), el('p', null, 'Your town is saved as you play.'));
  const pr = el('div', 'stack');
  const btn = (id, label, fn, primary) => { const b = el('button', primary ? 'primary' : null, label); b.id = id; b.addEventListener('click', fn); pr.appendChild(b); return b; };
  btn('btn-resume', 'Resume', () => setPaused(false), true);
  btn('btn-restart', 'Start this map again', () => { setPaused(false); api.restart(); });
  btn('btn-pause-options', 'Options', () => { buildOptions(); root.hidden = false; });
  btn('btn-pause-maps', 'Back to maps', () => { setPaused(false); api.showMenu(); });
  ps.appendChild(pr); pause.appendChild(ps); document.body.appendChild(pause);

  function setPaused(on) {
    app.paused = on;
    pause.hidden = !on;
    if (on) { app.speedBeforePause = app.speed; api.setSpeed(0); }
    else if (app.speedBeforePause != null) { api.setSpeed(app.speedBeforePause || 1); app.speedBeforePause = null; }
  }

  // main menu button
  const bar = document.querySelector('.menu-modes');
  const ob = el('button', 'chip', '⚙ Options'); ob.id = 'btn-options';
  ob.addEventListener('click', () => { buildOptions(); root.hidden = false; });
  bar.appendChild(ob);
  const hb = el('button', 'chip', '⏯'); hb.id = 'btn-pause'; hb.title = 'Pause menu (Esc)'; hb.setAttribute('aria-label', 'Pause menu');
  hb.addEventListener('click', () => setPaused(true));
  document.getElementById('speed-0').before(hb);

  // ---------- saved game card ----------
  function resumeCard(host) {
    const old = host.querySelector('.card.resume'); if (old) old.remove();
    const save = loadPref('save', null);
    if (!validSave(save)) return;
    const b = el('button', 'card resume'); b.dataset.resume = '1';
    b.append(el('span', 'name', 'Continue'), el('span', 'meta', describe(save, app.core.cities)), el('span', 'best', 'Pick up where you left off'));
    b.addEventListener('click', () => api.resume(save));
    host.prepend(b);
  }

  apply();
  return { apply, setPaused, resumeCard, buildOptions };
}
