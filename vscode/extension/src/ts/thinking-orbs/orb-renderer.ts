// Thinking Orb Canvas Controller
// Renders 60 FPS particle animations with HiDPI support and performance optimization

import { paintFrame } from './engine/core.js';
import { resolvePreset } from './presets.js';
import type { OrbState, ThinkingOrbOptions } from './types.js';

export class ThinkingOrbRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private state: OrbState = 'composing';
  private size = 64;
  private speed = 3.0;
  private dark = true;
  private animId: number | null = null;
  private running = false;
  private dpr = 1;
  private boundOnVisibility: () => void;

  constructor(canvas: HTMLCanvasElement, options?: ThinkingOrbOptions) {
    this.canvas = canvas;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      throw new Error('Canvas 2D context not supported');
    }
    this.ctx = ctx;

    if (options?.state) this.state = options.state;
    if (options?.size) this.size = options.size;
    if (options?.speed !== undefined) this.speed = options.speed;
    if (options?.dark !== undefined) this.dark = options.dark;

    this.setupDpr();

    this.boundOnVisibility = () => {
      if (document.visibilityState === 'hidden') {
        this.stop();
      } else if (this.running) {
        this.start();
      }
    };
    document.addEventListener('visibilitychange', this.boundOnVisibility);
  }

  private setupDpr(): void {
    this.dpr = Math.min(2, window.devicePixelRatio || 1);
    this.canvas.width = Math.round(this.size * this.dpr);
    this.canvas.height = Math.round(this.size * this.dpr);
  }

  public attachCanvas(canvas: HTMLCanvasElement): void {
    if (this.canvas === canvas) return;
    this.canvas = canvas;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      throw new Error('Canvas 2D context not supported');
    }
    this.ctx = ctx;
    this.setupDpr();
    this.drawSingleFrame();
  }

  public setState(state: OrbState): void {
    if (this.state === state) return;
    this.state = state;
    this.drawSingleFrame();
  }

  public setSpeed(speed: number): void {
    this.speed = speed;
  }

  public setSize(size: number): void {
    this.size = size;
    this.setupDpr();
    this.drawSingleFrame();
  }

  public start(): void {
    this.running = true;
    if (this.animId === null) {
      this.loop();
    }
  }

  public stop(): void {
    this.running = false;
    if (this.animId !== null) {
      cancelAnimationFrame(this.animId);
      this.animId = null;
    }
  }

  public destroy(): void {
    this.stop();
    document.removeEventListener('visibilitychange', this.boundOnVisibility);
    this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
  }

  public drawSingleFrame(tSec?: number): void {
    const t = tSec ?? (performance.now() / 1000);
    const { frame, speed: baseSpeed, opts } = resolvePreset(this.state, this.size);
    const effSpeed = baseSpeed * this.speed;

    this.ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    this.ctx.clearRect(0, 0, this.size, this.size);
    const orbFrame = frame(this.size, t * effSpeed, opts);
    paintFrame(this.ctx, orbFrame, this.dark);
  }

  private loop = (): void => {
    if (!this.running) return;
    this.drawSingleFrame();
    this.animId = requestAnimationFrame(this.loop);
  };
}
