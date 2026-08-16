// Presets and mode resolvers for thinking-orbs

import type { ModeFrame, ModeOpts, OrbSize, OrbState } from './types.js';
import { BASE_PROFILES, scaleCounts, scaleRadii } from './engine/profiles.js';
import { frameRibbon } from './engine/ribbon.js';
import { frameMorph } from './engine/morph.js';
import { frameOrbits } from './engine/orbits.js';
import { frameWeb } from './engine/web.js';

export const MODE_FRAMES: Record<OrbState, ModeFrame> = {
  composing: frameRibbon,
  shaping: frameMorph,
  working: frameOrbits,
  connecting: frameWeb
};

export interface Preset {
  speed: number;
  count: number;
  size: number;
  extra?: ModeOpts;
}

export const PRESETS: Record<OrbState, Record<OrbSize, Preset>> = {
  composing: {
    64: { speed: 2.34, count: 0.25, size: 0.85, extra: { spin: 0, bandMul: 3.9, wobMul: 1 } },
    20: { speed: 3.12, count: 0.051, size: 1.073, extra: { spin: 0, bandMul: 4.94, wobMul: 1 } }
  },
  shaping: {
    64: { speed: 2.405, count: 0.702, size: 0.395, extra: { spread: 1.45 } },
    20: { speed: 2.08, count: 0.53, size: 1.011, extra: { spread: 1.45 } }
  },
  working: {
    64: { speed: 1.885, count: 1, size: 1 },
    20: { speed: 3.9, count: 0.238, size: 2.4 }
  },
  connecting: {
    64: { speed: 3.315, count: 1.35, size: 0.95 },
    20: { speed: 6.63, count: 0.25, size: 1.52 }
  }
};

export interface ResolvedPreset {
  frame: ModeFrame;
  speed: number;
  opts: ModeOpts;
}

const cache = new Map<string, ResolvedPreset>();

export function resolvePreset(state: OrbState, size: number): ResolvedPreset {
  const normSize: OrbSize = size <= 32 ? 20 : 64;
  const key = `${state}-${normSize}`;
  const cached = cache.get(key);
  if (cached) return cached;

  const mode = state;
  const preset = PRESETS[mode][normSize];
  const baseMode = mode === 'composing' ? 'ribbon' : mode === 'shaping' ? 'morph' : mode === 'working' ? 'orbits' : 'web';
  let opts: ModeOpts = { ...BASE_PROFILES[baseMode] };
  if (preset.count !== 1) opts = scaleCounts(opts, preset.count);
  if (preset.size !== 1) opts = scaleRadii(opts, preset.size);
  if (preset.extra) opts = { ...opts, ...preset.extra };

  const resolved: ResolvedPreset = {
    frame: MODE_FRAMES[mode],
    speed: preset.speed,
    opts
  };
  cache.set(key, resolved);
  return resolved;
}
