// Thinking Orbs Type Definitions
// 1:1 math and state representations from thinking-orbs

export interface Dot {
  x: number;
  y: number;
  z: number;
  r: number;
  /** Ink value: 0 = darkest ink on paper. Mirrored on dark themes. */
  white: number;
  a?: number;
}

export interface Line {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  /** Ink value, same convention as `Dot.white`. */
  white: number;
  a?: number;
  w: number;
}

export interface OrbFrame {
  dots: Dot[];
  lines: Line[];
}

export interface ModeOpts {
  [key: string]: number | undefined;
}

export type ModeFrame = (size: number, t: number, opts: ModeOpts) => OrbFrame;

export type ModeDraw = (
  ctx: CanvasRenderingContext2D,
  size: number,
  t: number,
  dark: boolean,
  opts: ModeOpts
) => void;

export type OrbState = 'composing' | 'shaping' | 'working' | 'connecting';
export type OrbSize = 64 | 20;

export interface ThinkingOrbOptions {
  state?: OrbState;
  size?: number;
  speed?: number;
  dark?: boolean;
}
