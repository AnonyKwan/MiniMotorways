// Game controller support: a virtual cursor, drawing and erasing, zoom, pan, speed and tool buttons.
// The reading of the sticks and buttons is kept pure so it can be tested without a controller.

export const DEAD_ZONE = 0.18;
export const CURSOR_SPEEDS = [0.4, 0.7, 1.0, 1.4, 1.8]; // Slowest .. Fastest (doc 19.2)
export const BASE_CURSOR_PX_PER_SECOND = 620;

/** Stick position with a dead zone, rescaled so it starts from zero just outside it. */
export function stick(x, y) {
  const m = Math.hypot(x, y);
  if (m < DEAD_ZONE) return [0, 0];
  const k = Math.min(1, (m - DEAD_ZONE) / (1 - DEAD_ZONE)) / m;
  return [x * k, y * k];
}

export const BUTTONS = { draw: 0, erase: 1, undo: 2, offerSecond: 3, slower: 4, faster: 5, zoomOut: 6, zoomIn: 7, pause: 9, toolPrev: 14, toolNext: 15 };

export class PadReader {
  constructor() { this.prev = []; }

  /**
   * @param pad {{axes:number[], buttons:{pressed:boolean, value:number}[]}}
   * @returns {{cursor:number[], pan:number[], zoom:number, down:Set<string>, pressed:Set<string>, released:Set<string>}}
   */
  read(pad, dt, sensitivity = 1) {
    const axes = pad.axes || [];
    const [lx, ly] = stick(axes[0] || 0, axes[1] || 0);
    const [rx, ry] = stick(axes[2] || 0, axes[3] || 0);
    const speed = BASE_CURSOR_PX_PER_SECOND * sensitivity * dt;
    const held = pad.buttons.map((b) => Boolean(b && (b.pressed || b.value > 0.6)));
    const down = new Set(), pressed = new Set(), released = new Set();
    for (const [name, i] of Object.entries(BUTTONS)) {
      if (held[i]) down.add(name);
      if (held[i] && !this.prev[i]) pressed.add(name);
      if (!held[i] && this.prev[i]) released.add(name);
    }
    this.prev = held;
    const zoom = (down.has('zoomIn') ? 1 : 0) - (down.has('zoomOut') ? 1 : 0);
    return { cursor: [lx * speed, ly * speed], pan: [rx * 700 * dt, ry * 700 * dt], zoom, down, pressed, released };
  }
}

/** Which of the first connected pads, if any, is in use. */
export function firstPad() {
  if (typeof navigator === 'undefined' || !navigator.getGamepads) return null;
  for (const p of navigator.getGamepads()) if (p && p.connected) return p;
  return null;
}

/** Moves a point inside the screen. */
export function clampPoint(x, y, w, h) { return [Math.max(4, Math.min(w - 4, x)), Math.max(4, Math.min(h - 4, y))]; }
