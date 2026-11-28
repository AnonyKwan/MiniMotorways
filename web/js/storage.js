// Saved progress and preferences. Everything is wrapped: storage can be blocked or full.

const PREFIX = 'littlelanes.';

export function loadPref(key, fallback) {
  try {
    const raw = localStorage.getItem(PREFIX + key);
    return raw == null ? fallback : JSON.parse(raw);
  } catch (_) {
    return fallback;
  }
}

export function savePref(key, value) {
  try {
    localStorage.setItem(PREFIX + key, JSON.stringify(value));
  } catch (_) {
    /* private mode or quota: progress simply isn't kept */
  }
}

/** Best trips per city id. */
export function loadBest() {
  const v = loadPref('best', {});
  return v && typeof v === 'object' ? v : {};
}

export function saveBest(best) {
  savePref('best', best);
}
