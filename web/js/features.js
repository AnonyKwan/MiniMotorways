// Everything around the game board: achievements, challenges, tutorial coach, photo mode, undo, Endless.
import {
  ACHIEVEMENTS, emptyProgress, normalise, recordGame, sweep, titleOf, describeDef,
} from './achievements.js';
import {
  dailyChallengeSet, weeklyChallengeSet, cityChallengeSet, modsOf, titleOf as challengeTitle, describe as challengeText,
  ANY_MAP_TRIPS_TO_UNLOCK, CITY_TRIPS_TO_UNLOCK, isoDay,
} from './challenges.js';
import { loadPref, savePref } from './storage.js';

const el = (tag, cls, text) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
};

export function initFeatures(app, api) {
  const $ = (id) => document.getElementById(id);
  app.progress = normalise(loadPref('progress', null));
  app.challengeBest = loadPref('challengeBest', {});
  app.challengeDone = loadPref('challengeDone', {});
  let concreteSpent = 0;
  let lastUsed = 0;
  let lastSweep = 0;
  let tutorialStep = -1;

  const save = () => { savePref('progress', app.progress); savePref('challengeBest', app.challengeBest); savePref('challengeDone', app.challengeDone); };
  const cityName = (id) => { const c = app.core.cities.find((x) => x.id === id); return c ? c.name : id; };
  const bestTrips = () => Math.max(0, ...Object.values(app.progress.best).map((b) => Math.max(b.c || 0, b.e || 0)), ...Object.values(app.best || {}));
  const anyMapUnlocked = () => app.unlockAll || bestTrips() >= ANY_MAP_TRIPS_TO_UNLOCK;
  const cityBest = (id) => Math.max((app.best && app.best[id]) || 0, (app.progress.best[id] || {}).c || 0);

  // ---------- overlays ----------
  function overlay(id, title) {
    const root = el('section', 'overlay');
    root.id = id; root.hidden = true;
    const sheet = el('div', 'sheet wide');
    const h = el('h2', null, title);
    const body = el('div', 'scroll');
    const row = el('div', 'row');
    const close = el('button', 'primary', 'Close');
    close.addEventListener('click', () => { root.hidden = true; });
    row.appendChild(close);
    sheet.append(h, body, row);
    root.appendChild(sheet);
    document.body.appendChild(root);
    return { root, body, h };
  }
  const ach = overlay('ach', 'Achievements');
  const chal = overlay('chal', 'Challenges');

  function renderAchievements() {
    const earned = ACHIEVEMENTS.filter((d) => app.progress.unlocked[d[0]]).length;
    ach.h.textContent = `Achievements · ${earned} / ${ACHIEVEMENTS.length}`;
    ach.body.textContent = '';
    const list = el('ul', 'ach-list');
    const defs = [...ACHIEVEMENTS].sort((a, b) => Number(Boolean(app.progress.unlocked[b[0]])) - Number(Boolean(app.progress.unlocked[a[0]])));
    for (const d of defs) {
      const li = el('li', app.progress.unlocked[d[0]] ? 'ach on' : 'ach');
      li.dataset.id = d[0];
      li.append(el('b', null, (app.progress.unlocked[d[0]] ? '✓ ' : '') + titleOf(d, cityName)), el('span', null, describeDef(d, cityName)));
      list.appendChild(li);
    }
    ach.body.appendChild(list);
  }

  function challengeCard(set, locked, reason) {
    const card = el('div', locked ? 'ch locked' : 'ch');
    const best = app.challengeBest[set.key];
    card.append(
      el('b', null, challengeTitle(set) + (set.mode === 1 ? ' · Expert' : '')),
      el('span', null, `${set.city.name}: ${challengeText(set)}`),
      el('em', null, locked ? reason : (best != null ? `Best ${best} trips` : (set.target ? `Goal ${set.target} trips` : 'Not played yet'))),
    );
    const go = el('button', 'primary', 'Play');
    go.disabled = locked;
    go.dataset.challenge = set.key;
    go.addEventListener('click', () => {
      chal.root.hidden = true;
      api.startGame(set.index, set.seed, set.mode, { mods: modsOf(set), challenge: { key: set.key, kind: set.kind, title: challengeTitle(set), target: set.target } });
    });
    card.appendChild(go);
    return card;
  }

  function renderChallenges() {
    chal.body.textContent = '';
    const now = new Date();
    const cities = app.core.cities;
    const open = anyMapUnlocked();
    const need = `Deliver ${ANY_MAP_TRIPS_TO_UNLOCK} trips on any map first`;
    const sect = (t) => chal.body.appendChild(el('h3', null, t));
    sect(`Today · ${isoDay(now)}`);
    chal.body.appendChild(challengeCard(dailyChallengeSet(now, cities), !open, need));
    sect('This week');
    chal.body.appendChild(challengeCard(weeklyChallengeSet(now, cities), !open, need));
    sect('City challenges');
    for (const c of cities) {
      if (c.id === 'osaka') continue;
      const unlocked = app.unlockAll || cityBest(c.id) >= CITY_TRIPS_TO_UNLOCK;
      if (!unlocked && !params().has('showall')) continue;
      cityChallengeSet(c.id, cities).forEach((s) => chal.body.appendChild(challengeCard(s, !unlocked, `Deliver ${CITY_TRIPS_TO_UNLOCK} trips in ${c.name}`)));
    }
    if (chal.body.lastChild.tagName === 'H3') {
      chal.body.appendChild(el('p', 'muted', `Deliver ${CITY_TRIPS_TO_UNLOCK} trips on a map to unlock its own challenges.`));
    }
  }
  const params = () => new URLSearchParams(location.search);

  // ---------- menu buttons ----------
  const bar = document.querySelector('.menu-modes');
  const mk = (label, id, fn) => { const b = el('button', 'chip', label); b.id = id; b.addEventListener('click', fn); bar.appendChild(b); return b; };
  mk('🏆 Achievements', 'btn-ach', () => { renderAchievements(); ach.root.hidden = false; });
  mk('⚑ Challenges', 'btn-chal', () => { renderChallenges(); chal.root.hidden = false; });
  mk('🎓 Tutorial', 'btn-tutorial', () => {
    const i = app.core.cities.findIndex((c) => c.id === 'losangeles');
    api.startGame(i, 7, 0, { tutorial: true });
  });

  // ---------- toasts for achievements ----------
  function celebrate(defs) {
    for (const d of defs) api.toast(`Achievement: ${titleOf(d, cityName)}`, 2600);
    if (defs.length) save();
  }

  // ---------- coach (tutorial) ----------
  const coach = el('div', 'coach');
  coach.hidden = true;
  const coachText = el('p');
  const coachSkip = el('button', 'chip', 'Skip');
  coach.append(coachText, coachSkip);
  document.body.appendChild(coach);
  coachSkip.addEventListener('click', () => endTutorial(false));
  const STEPS = [
    { say: 'Welcome! Press and drag from a home (the small coloured box) to the big building with the same colour and shape.', done: (s) => s.up[0][1] >= 2 },
    { say: 'Nice. Cars now carry people there. Wait for the first trip to be delivered.', done: (s) => s.score >= 1 },
    { say: 'Pins pile up at destinations. Keep cars coming, or a red ring will fill. Deliver three trips.', done: (s) => s.score >= 3 },
    { say: 'Mistakes happen: right-drag (or Tab) erases a road, Z brings it back. Try erasing something.', done: (s) => s.stats[2] >= 1 || app.erased },
    { say: 'Every week you choose an upgrade. Press 3 to fast-forward until the offer appears, then pick one.', done: (s) => s.week >= 1 && !s.offerOpen },
  ];
  function endTutorial(finished) {
    tutorialStep = -1;
    coach.hidden = true;
    if (finished) {
      app.progress.tutorial = true;
      celebrate(sweep(app.progress, null));
      api.toast('Tutorial complete!', 2200);
      save();
    }
  }
  function coachTick(s) {
    if (tutorialStep < 0) return;
    if (tutorialStep >= STEPS.length) { endTutorial(true); return; }
    coachText.textContent = `${tutorialStep + 1}/${STEPS.length}  ${STEPS[tutorialStep].say}`;
    coach.hidden = false;
    if (STEPS[tutorialStep].done(s)) tutorialStep++;
  }

  // ---------- photo mode ----------
  const photoBtn = el('button', 'chip', '📷');
  photoBtn.id = 'btn-photo'; photoBtn.title = 'Photo mode'; photoBtn.setAttribute('aria-label', 'Photo mode');
  $('btn-sound').before(photoBtn);
  const photoBar = el('div', 'photobar');
  photoBar.hidden = true;
  const save1 = el('button', 'primary', 'Save picture');
  const close1 = el('button', null, 'Done');
  photoBar.append(save1, close1);
  document.body.appendChild(photoBar);
  function setPhoto(on) {
    app.photo = on;
    document.body.classList.toggle('photo', on);
    photoBar.hidden = !on;
    if (on) { app.view = { zoom: 1, panX: 0, panY: 0 }; api.setSpeed(0); }
  }
  photoBtn.addEventListener('click', () => setPhoto(!app.photo));
  close1.addEventListener('click', () => setPhoto(false));
  save1.addEventListener('click', () => {
    app.canvas.toBlob((blob) => {
      if (!blob) return;
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = `little-lanes-${app.core.cities[app.cityIndex].id}.png`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 2000);
    });
  });

  // ---------- undo ----------
  const undoBtn = el('button', 'tool');
  undoBtn.id = 'btn-undo';
  undoBtn.innerHTML = '<svg viewBox="0 0 32 32" aria-hidden="true"><path d="M9 12 H20 A6 6 0 0 1 20 24 H12"/><path d="M13 7 L8 12 L13 17"/></svg><span>Undo</span>';
  undoBtn.addEventListener('click', undo);
  $('tools').appendChild(undoBtn);
  function undo() {
    if (app.screen !== 'game' || app.over) return;
    const code = app.core.undo();
    api.afterEdit(code, false);
    if (code === 0) api.toast('Roads put back', 900);
  }

  // ---------- Endless after a loss ----------
  const endless = el('button', null, 'Keep building (Endless)');
  endless.id = 'btn-endless';
  $('btn-over-menu').before(endless);
  endless.addEventListener('click', () => {
    app.core.continueEndless();
    app.over = false;
    app.playMode = 3;
    $('over').hidden = true;
    $('hud-city').textContent = `${app.core.cities[app.cityIndex].name} · Endless`;
    api.setSpeed(1);
  });

  // ---------- keyboard ----------
  window.addEventListener('keydown', (e) => {
    if (app.screen !== 'game') return;
    if (e.key === 'z' || e.key === 'Z') { e.preventDefault(); undo(); }
    if (e.key === 'Escape' && app.photo) setPhoto(false);
  });

  // ---------- life cycle ----------
  function finishGame(leaving) {
    if (app.finished || !app.snap) return;
    app.finished = true;
    const s = app.snap;
    const stats = { placed: [0, ...s.stats.slice(4, 9)], deleted: s.stats[2], trees: s.stats[1], bigPins: s.stats[3], milestones: s.stats[0] };
    app.progress = recordGame(app.progress, { cityId: app.core.cities[app.cityIndex].id, mode: app.playMode, score: s.score, stats, concrete: concreteSpent });
    const ch = app.challenge;
    if (ch) {
      app.challengeBest[ch.key] = Math.max(app.challengeBest[ch.key] || 0, s.score);
      if (!app.challengeDone[ch.key]) {
        app.challengeDone[ch.key] = true;
        if (ch.kind === 'daily') app.progress.daily += 1;
        if (ch.kind === 'weekly') app.progress.weekly += 1;
      }
    }
    celebrate(sweep(app.progress, null));
    save();
    if (leaving) coach.hidden = true;
  }

  return {
    onStart(opts) {
      app.finished = false;
      app.challenge = opts.challenge || null;
      app.erased = false;
      concreteSpent = 0; lastUsed = 0; lastSweep = 0;
      setPhoto(false);
      tutorialStep = opts.tutorial ? 0 : -1;
      coach.hidden = !opts.tutorial;
      endless.hidden = false;
    },
    onGameOver() {
      endless.hidden = app.playMode === 3;
      finishGame(false);
    },
    onLeave() { finishGame(true); setPhoto(false); coach.hidden = true; tutorialStep = -1; },
    tick(s) {
      const used = s.up[0][1];
      if (used > lastUsed) concreteSpent += used - lastUsed;
      lastUsed = used;
      coachTick(s);
      if (s.t - lastSweep > 1 && !app.over) {
        lastSweep = s.t;
        const live = {
          cityId: app.core.cities[app.cityIndex].id, mode: app.playMode, score: s.score, concrete: concreteSpent,
          stats: { placed: [0, ...s.stats.slice(4, 9)], deleted: s.stats[2], trees: s.stats[1], bigPins: s.stats[3], milestones: s.stats[0] },
        };
        celebrate(sweep(app.progress, live));
      }
    },
    markErased() { app.erased = true; },
    undo,
    setPhoto,
  };
}
