<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    completed?: boolean;
  }

  const { completed = false }: Props = $props();

  let canvas: HTMLCanvasElement;

  onMount(() => {
    const ctx = canvas.getContext("2d")!;
    let raf: number;
    let rotation = 0;
    let globeOffsetX = 0;
    let globeScale = 1;

    const GLOBE_POINTS = 360;
    const LINK_DIST = 0.32;
    const BASE_SPEED = 0.0014;
    const LERP = 0.035;
    const FLOAT_AMP = 8;
    const FLOAT_FREQ = 0.012;
    const TILT = 23 * Math.PI / 180;
    const cosTilt = Math.cos(TILT);
    const sinTilt = Math.sin(TILT);

    type Pt3 = { x: number; y: number; z: number };

    function hash(i: number): number {
      let h = (i * 2654435761) >>> 0;
      return (h & 0xffff) / 0xffff;
    }

    const globePts: Pt3[] = [];
    const goldenAngle = Math.PI * (3 - Math.sqrt(5));
    for (let i = 0; i < GLOBE_POINTS; i++) {
      const y = 1 - (i / (GLOBE_POINTS - 1)) * 2;
      const r = Math.sqrt(1 - y * y);
      const theta = goldenAngle * i;
      const jitter = 0.97 + hash(i) * 0.06;
      globePts.push({
        x: Math.cos(theta) * r * jitter,
        y: y * (0.97 + hash(i + 999) * 0.06),
        z: Math.sin(theta) * r * jitter,
      });
    }

    let frameCount = 0;

    function accentRGB(): [number, number, number] {
      const s = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim();
      if (s.startsWith("#") && s.length >= 7) {
        return [parseInt(s.slice(1, 3), 16), parseInt(s.slice(3, 5), 16), parseInt(s.slice(5, 7), 16)];
      }
      return [224, 149, 69];
    }

    function draw() {
      const dpr = window.devicePixelRatio || 1;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (w === 0 || h === 0) { raf = requestAnimationFrame(draw); return; }
      canvas.width = w * dpr;
      canvas.height = h * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      frameCount++;

      const targetOffsetX = completed ? -w * 0.15 : 0;
      const targetScale = completed ? 0.9 : 1;
      globeOffsetX += (targetOffsetX - globeOffsetX) * LERP;
      globeScale += (targetScale - globeScale) * LERP;

      const floatY = Math.sin(frameCount * FLOAT_FREQ) * FLOAT_AMP;
      const cx = w / 2 + globeOffsetX;
      const cy = h * 0.44 + floatY;
      const baseRadius = Math.min(w, h) * 0.30;
      const radius = baseRadius * globeScale;
      const [ar, ag, ab] = accentRGB();

      ctx.clearRect(0, 0, w, h);

      /* ── ambient glow ────────────────────────── */
      const glow = ctx.createRadialGradient(cx, cy, radius * 0.1, cx, cy, radius * 1.5);
      glow.addColorStop(0, `rgba(${ar},${ag},${ab}, 0.07)`);
      glow.addColorStop(0.5, `rgba(${ar},${ag},${ab}, 0.025)`);
      glow.addColorStop(1, `rgba(${ar},${ag},${ab}, 0)`);
      ctx.fillStyle = glow;
      ctx.fillRect(0, 0, w, h);

      /* ── rotate (Y) & tilt (X) & project ─────── */
      const cosR = Math.cos(rotation);
      const sinR = Math.sin(rotation);

      type Projected = { sx: number; sy: number; depth: number };
      const projected: Projected[] = [];

      for (const p of globePts) {
        const rx = p.x * cosR - p.z * sinR;
        const ry = p.y;
        const rz = p.x * sinR + p.z * cosR;
        const ty = ry * cosTilt - rz * sinTilt;
        const tz = ry * sinTilt + rz * cosTilt;
        const depth = (tz + 1) / 2;
        projected.push({ sx: cx + rx * radius, sy: cy + ty * radius, depth });
      }

      /* ── connecting lines ────────────────────── */
      const linkPx = LINK_DIST * radius * 2;
      for (let i = 0; i < projected.length; i++) {
        const a = projected[i];
        if (a.depth < 0.22) continue;
        for (let j = i + 1; j < projected.length; j++) {
          const b = projected[j];
          if (b.depth < 0.22) continue;
          const dx = a.sx - b.sx;
          const dy = a.sy - b.sy;
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < linkPx) {
            const alpha = (1 - dist / linkPx) * Math.min(a.depth, b.depth) * 0.2;
            ctx.beginPath();
            ctx.moveTo(a.sx, a.sy);
            ctx.lineTo(b.sx, b.sy);
            ctx.strokeStyle = `rgba(${ar},${ag},${ab},${alpha.toFixed(3)})`;
            ctx.lineWidth = 0.5;
            ctx.stroke();
          }
        }
      }

      /* ── globe points ────────────────────────── */
      for (const p of projected) {
        const alpha = 0.1 + p.depth * 0.78;
        const size = 0.7 + p.depth * 2.0;
        ctx.beginPath();
        ctx.arc(p.sx, p.sy, size, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(${ar},${ag},${ab},${alpha.toFixed(3)})`;
        ctx.fill();
      }

      rotation += BASE_SPEED;
      raf = requestAnimationFrame(draw);
    }

    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  });
</script>

<canvas class="particle-globe" bind:this={canvas}></canvas>

<style>
  .particle-globe {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
