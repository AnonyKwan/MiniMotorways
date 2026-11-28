// Thin wrapper around the WebAssembly simulation core (see src/ffi.rs).
// Works in the browser and in Node (pass the bytes yourself).

export const ERR = {
  OK: 0, BOUNDS: 1, NOT_ADJACENT: 2, BLOCKED: 3, NO_CONCRETE: 4, NO_BRIDGE: 5, NO_TUNNEL: 6,
  CROSSING: 7, EXISTS: 8, NO_UPGRADE: 9, BAD_PLACEMENT: 10, GAME_OVER: 11, NOTHING: 12,
  TOO_SHORT: 13, NO_OFFER: 14, PERMANENT: 15,
};

// kind numbers used in upgrade counts and offers
export const KIND = { CONCRETE: 0, BRIDGE: 1, MOTORWAY: 2, LIGHT: 3, ROUNDABOUT: 4, TUNNEL: 5 };
export const KIND_NAMES = ['Concrete', 'Bridge', 'Motorway', 'Traffic light', 'Roundabout', 'Tunnel'];

export class Core {
  constructor(instance) {
    this.x = instance.exports;
    this.dec = new TextDecoder();
    this.cities = this.readJson(this.x.ll_catalog());
  }

  /** @param {BufferSource | Response | Promise<Response>} source */
  static async load(source) {
    let instance;
    if (source && typeof source.then === 'function') source = await source;
    if (typeof Response !== 'undefined' && source instanceof Response) {
      if (WebAssembly.instantiateStreaming) {
        try {
          ({ instance } = await WebAssembly.instantiateStreaming(source.clone(), {}));
        } catch (_) {
          instance = undefined; // wrong MIME type etc.: fall back to bytes
        }
      }
      if (!instance) ({ instance } = await WebAssembly.instantiate(await source.arrayBuffer(), {}));
    } else {
      ({ instance } = await WebAssembly.instantiate(source, {}));
    }
    return new Core(instance);
  }

  bytes(len) {
    return new Uint8Array(this.x.memory.buffer, this.x.ll_out_ptr(), len).slice();
  }
  readJson(len) {
    return JSON.parse(this.dec.decode(new Uint8Array(this.x.memory.buffer, this.x.ll_out_ptr(), len)));
  }

  /** @param mode 0 classic, 1 expert, 2 creative */
  newGame(cityIndex, seed, mode = 0) {
    this.x.ll_new_mode(cityIndex >>> 0, seed >>> 0, mode >>> 0);
    this.city = this.cities[cityIndex];
  }
  step(dt) { this.x.ll_step(dt); }
  snapshot() { return this.readJson(this.x.ll_snapshot()); }
  statics() { return this.readJson(this.x.ll_statics()); }
  terrain() { return this.bytes(this.x.ll_terrain()); }

  addRoad(a, b) { return this.x.ll_add_road(a, b); }
  removeRoad(a, b) { return this.x.ll_remove_road(a, b); }
  eraseCell(c) { return this.x.ll_erase_cell(c); }
  addMotorway(a, b) { return this.x.ll_add_motorway(a, b); }
  removeMotorway(i) { return this.x.ll_remove_motorway(i); }
  toggleLight(c) { return this.x.ll_toggle_light(c); }
  toggleRoundabout(c) { return this.x.ll_toggle_roundabout(c); }
  chooseUpgrade(i) { return this.x.ll_choose_upgrade(i); }
}
