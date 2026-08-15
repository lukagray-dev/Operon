// Thinking Orb Canvas Renderer
//
// 1:1 mathematical port of the 'composing' preset (ribbon sash undulation with spin=0)
// from Jakubantalik/thinking-orbs and Slint UI:
// Frozen 3D sash tumble with traveling wave undulation along 10 parallel lanes x 40 segments,
// camera projection, and depth-based particle shading.

export class ThinkingOrbRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private animFrameId: number | null = null;
  private t = 0.0;
  private isRunning = false;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      throw new Error('Canvas 2D context not supported');
    }
    this.ctx = ctx;

    // Retina / High-DPI screen scaling for ultra-sharp particle edges
    const dpr = window.devicePixelRatio || 1;
    const logicalSize = 20;
    this.canvas.width = logicalSize * dpr;
    this.canvas.height = logicalSize * dpr;
    this.ctx.scale(dpr, dpr);
  }

  /**
   * Starts the 60 FPS animation loop.
   */
  public start(): void {
    if (this.isRunning) return;
    this.isRunning = true;
    this.loop();
  }

  /**
   * Stops and freezes the animation loop.
   */
  public stop(): void {
    this.isRunning = false;
    if (this.animFrameId !== null) {
      cancelAnimationFrame(this.animFrameId);
      this.animFrameId = null;
    }
  }

  /**
   * Cleans up canvas resources.
   */
  public destroy(): void {
    this.stop();
    this.ctx.clearRect(0, 0, 20, 20);
  }

  private loop = (): void => {
    if (!this.isRunning) return;
    this.render();
    this.t += 0.006;
    if (this.t > 1000.0) {
      this.t = 0.0;
    }
    this.animFrameId = requestAnimationFrame(this.loop);
  };

  /**
   * Renders one frame of the 3D sash ribbon particles.
   */
  private render(): void {
    const size = 20;
    this.ctx.clearRect(0, 0, size, size);

    // 10 Parallel Lanes x 40 Segments per lane
    for (let w = 0; w < 10; w++) {
      const laneOff = (w - 4.5) * 0.06;
      const edge = Math.abs(w - 4.5) / 4.5;

      for (let k = 0; k < 40; k++) {
        const aRad = (k / 40.0) * Math.PI * 2;

        // Traveling wave undulation equation from Slint composing.slint
        const wob =
          0.16 * Math.sin(aRad * 3.0 - this.t * Math.PI * 2 * 1.7 + w * 0.22) +
          0.07 * Math.sin(aRad * 5.0 + this.t * Math.PI * 2 * 1.1);
        const off = laneOff + wob;

        const xRaw = Math.cos(aRad);
        const yRaw = Math.sin(aRad) * 0.8525 - off * 0.5227;
        const zRaw = Math.sin(aRad) * 0.5227 + off * 0.8525;

        const len = Math.sqrt(Math.max(0.01, xRaw * xRaw + yRaw * yRaw + zRaw * zRaw));

        // Camera tilt projection (camTilt = 0.3 rad)
        const px3d = xRaw / len;
        const py3d = (yRaw / len) * 0.9553 - (zRaw / len) * 0.2955;
        const z3d = (yRaw / len) * 0.2955 + (zRaw / len) * 0.9553;

        const depth = (z3d + 1.0) / 2.0;

        // Point size and opacity scaled for 20px viewport
        const ptWidth = Math.max(0.7, (1.1 + 1.7 * depth) * (1.0 - 0.2 * edge) * (size / 64));
        const opacity = Math.max(0.12, (0.38 + 0.62 * depth) * (0.65 + 0.35 * (1.0 - edge)));

        const px = size / 2 + px3d * (size * 0.39) - ptWidth / 2;
        const py = size / 2 - py3d * (size * 0.39) - ptWidth / 2;

        this.ctx.fillStyle = `rgba(255, 255, 255, ${opacity.toFixed(3)})`;
        this.ctx.beginPath();
        this.ctx.arc(px + ptWidth / 2, py + ptWidth / 2, ptWidth / 2, 0, Math.PI * 2);
        this.ctx.fill();
      }
    }
  }
}
