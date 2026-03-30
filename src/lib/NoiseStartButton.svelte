<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    label: string;
    disabled?: boolean;
    onClick?: () => void;
  }

  const { label, disabled = false, onClick }: Props = $props();

  const vertexShaderSource = `#version 300 es
in vec2 a_position;
out vec2 v_uv;

void main() {
  v_uv = a_position * 0.5 + 0.5;
  gl_Position = vec4(a_position, 0.0, 1.0);
}`;

  const fragmentShaderSource = `#version 300 es
precision highp float;

in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_resolution;
uniform float u_time;
uniform float u_hover;
uniform float u_press;

float hash12(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);

  float a = hash12(i);
  float b = hash12(i + vec2(1.0, 0.0));
  float c = hash12(i + vec2(0.0, 1.0));
  float d = hash12(i + vec2(1.0, 1.0));

  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm(vec2 p) {
  float value = 0.0;
  float amplitude = 0.5;
  mat2 m = mat2(1.6, 1.2, -1.2, 1.6);
  for (int i = 0; i < 4; i++) {
    value += amplitude * noise(p);
    p = m * p;
    amplitude *= 0.5;
  }
  return value;
}

void main() {
  vec2 uv = v_uv;
  vec2 centered = uv * 2.0 - 1.0;
  centered.x *= u_resolution.x / max(u_resolution.y, 1.0);

  float t = u_time * 0.58;
  float flow = fbm(centered * 2.4 + vec2(0.0, t));
  float detail = fbm(centered * 7.8 + vec2(t * 1.3, -t * 0.85));
  float streak = sin((uv.x * 18.0 - t * 4.2) + detail * 3.2) * 0.5 + 0.5;
  float pulse = sin(t * 2.1) * 0.5 + 0.5;

  vec3 base = vec3(0.055, 0.075, 0.12);
  vec3 fill = vec3(0.88, 0.58, 0.24);
  vec3 tint = vec3(0.34, 0.5, 0.92);

  float glow = smoothstep(1.18, 0.22, length(centered * vec2(0.86, 1.12)));
  float band = smoothstep(0.32, 0.95, streak + flow * 0.35);
  float shimmer = smoothstep(0.46, 1.0, detail + pulse * 0.22);

  vec3 color = base;
  color += fill * band * (0.12 + u_hover * 0.30 + u_press * 0.24);
  color += tint * shimmer * (0.08 + u_hover * 0.22);
  color += fill * glow * (0.18 + u_hover * 0.18 + u_press * 0.16);
  color += vec3(1.0) * pow(max(0.0, band - 0.72), 2.0) * (0.08 + u_hover * 0.18);

  fragColor = vec4(color, 0.96);
}`;

  let buttonEl: HTMLButtonElement;
  let canvasEl: HTMLCanvasElement;
  let reduceMotion = false;

  let hoverTarget = 0;
  let pressTarget = 0;

  let frameId = 0;
  let observer: ResizeObserver | undefined;
  let gl: WebGL2RenderingContext | null = null;
  let program: WebGLProgram | null = null;
  let disposeGl: (() => void) | undefined;

  function createShader(ctx: WebGL2RenderingContext, type: number, source: string) {
    const shader = ctx.createShader(type);
    if (!shader) throw new Error("createShader failed");
    ctx.shaderSource(shader, source.trim());
    ctx.compileShader(shader);
    if (!ctx.getShaderParameter(shader, ctx.COMPILE_STATUS)) {
      const info = ctx.getShaderInfoLog(shader) ?? "shader compile failed";
      ctx.deleteShader(shader);
      throw new Error(info);
    }
    return shader;
  }

  function createProgram(ctx: WebGL2RenderingContext) {
    const vertexShader = createShader(ctx, ctx.VERTEX_SHADER, vertexShaderSource);
    const fragmentShader = createShader(ctx, ctx.FRAGMENT_SHADER, fragmentShaderSource);
    const shaderProgram = ctx.createProgram();
    if (!shaderProgram) throw new Error("createProgram failed");
    ctx.attachShader(shaderProgram, vertexShader);
    ctx.attachShader(shaderProgram, fragmentShader);
    ctx.linkProgram(shaderProgram);
    ctx.deleteShader(vertexShader);
    ctx.deleteShader(fragmentShader);
    if (!ctx.getProgramParameter(shaderProgram, ctx.LINK_STATUS)) {
      const info = ctx.getProgramInfoLog(shaderProgram) ?? "program link failed";
      ctx.deleteProgram(shaderProgram);
      throw new Error(info);
    }
    return shaderProgram;
  }

  function resizeCanvas() {
    if (!gl || !canvasEl) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const width = Math.max(1, Math.round(canvasEl.clientWidth * dpr));
    const height = Math.max(1, Math.round(canvasEl.clientHeight * dpr));
    if (canvasEl.width !== width || canvasEl.height !== height) {
      canvasEl.width = width;
      canvasEl.height = height;
      gl.viewport(0, 0, width, height);
    }
  }

  function startRenderLoop() {
    if (!gl || !program) return;

    const positionLocation = gl.getAttribLocation(program, "a_position");
    const resolutionLocation = gl.getUniformLocation(program, "u_resolution");
    const timeLocation = gl.getUniformLocation(program, "u_time");
    const hoverLocation = gl.getUniformLocation(program, "u_hover");
    const pressLocation = gl.getUniformLocation(program, "u_press");

    const buffer = gl.createBuffer();
    if (!buffer) return;
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([
      -1, -1,
       1, -1,
      -1,  1,
      -1,  1,
       1, -1,
       1,  1,
    ]), gl.STATIC_DRAW);

    gl.useProgram(program);
    gl.enableVertexAttribArray(positionLocation);
    gl.vertexAttribPointer(positionLocation, 2, gl.FLOAT, false, 0, 0);

    let hover = 0;
    let press = 0;
    const startedAt = performance.now();

    const render = (now: number) => {
      frameId = requestAnimationFrame(render);
      resizeCanvas();
      hover += (hoverTarget - hover) * 0.12;
      press += (pressTarget - press) * 0.18;

      gl!.useProgram(program);
      gl!.uniform2f(resolutionLocation, canvasEl.width, canvasEl.height);
      gl!.uniform1f(timeLocation, reduceMotion ? 0 : (now - startedAt) * 0.001);
      gl!.uniform1f(hoverLocation, hover);
      gl!.uniform1f(pressLocation, press);
      gl!.drawArrays(gl!.TRIANGLES, 0, 6);
    };

    frameId = requestAnimationFrame(render);

    disposeGl = () => {
      cancelAnimationFrame(frameId);
      gl?.deleteBuffer(buffer);
    };
  }

  function handlePointerEnter() {
    hoverTarget = 1;
  }

  function handlePointerLeave() {
    hoverTarget = 0;
    pressTarget = 0;
  }

  function handlePointerDown() {
    pressTarget = 1;
  }

  function handlePointerUp() {
    pressTarget = 0;
  }

  function handleClick() {
    if (disabled) return;
    onClick?.();
  }

  onMount(() => {
    reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    gl = canvasEl.getContext("webgl2", {
      alpha: true,
      antialias: true,
      depth: false,
      stencil: false,
      premultipliedAlpha: true,
    });

    if (!gl) return;

    try {
      program = createProgram(gl);
    } catch (error) {
      console.error("NoiseStartButton shader init failed", error);
      return;
    }

    observer = new ResizeObserver(() => resizeCanvas());
    observer.observe(buttonEl);
    resizeCanvas();
    startRenderLoop();

    return () => {
      observer?.disconnect();
      disposeGl?.();
      if (program) gl?.deleteProgram(program);
    };
  });
</script>

<button
  bind:this={buttonEl}
  class="noise-button"
  type="button"
  {disabled}
  onmouseenter={handlePointerEnter}
  onmouseleave={handlePointerLeave}
  onpointerdown={handlePointerDown}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerUp}
  onblur={handlePointerUp}
  onclick={handleClick}
>
  <canvas bind:this={canvasEl} class="noise-canvas" aria-hidden="true"></canvas>
  <span class="noise-border" aria-hidden="true"></span>
  <span class="noise-glare" aria-hidden="true"></span>
  <span class="noise-label">{label}</span>
</button>

<style>
  .noise-button {
    position: relative;
    isolation: isolate;
    display: inline-grid;
    place-items: center;
    min-width: 168px;
    padding: 0 28px;
    height: 56px;
    border: 0;
    border-radius: 18px;
    background: linear-gradient(180deg, rgba(17, 20, 28, 0.96), rgba(11, 13, 20, 0.96));
    color: #f7efe6;
    font: inherit;
    font-size: var(--font-md);
    font-weight: 700;
    letter-spacing: 0.01em;
    cursor: pointer;
    overflow: hidden;
    box-shadow:
      0 18px 40px rgba(0, 0, 0, 0.28),
      0 0 0 1px rgba(255, 255, 255, 0.08),
      inset 0 1px 0 rgba(255, 255, 255, 0.1);
    transition:
      transform 180ms ease,
      box-shadow 240ms ease,
      filter 240ms ease;
  }

  .noise-button:hover {
    transform: translateY(-1px);
    box-shadow:
      0 24px 46px rgba(0, 0, 0, 0.34),
      0 0 0 1px rgba(255, 255, 255, 0.1),
      0 0 30px rgba(var(--accent-rgb), 0.16),
      inset 0 1px 0 rgba(255, 255, 255, 0.14);
  }

  .noise-button:active {
    transform: translateY(1px) scale(0.985);
  }

  .noise-button:focus-visible {
    outline: none;
    box-shadow:
      0 24px 46px rgba(0, 0, 0, 0.34),
      0 0 0 1px rgba(255, 255, 255, 0.1),
      0 0 0 3px rgba(var(--accent-rgb), 0.22),
      0 0 32px rgba(var(--accent-rgb), 0.2);
  }

  .noise-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
    filter: grayscale(0.15);
  }

  .noise-canvas,
  .noise-border,
  .noise-glare {
    position: absolute;
    inset: 0;
    border-radius: inherit;
  }

  .noise-canvas {
    width: 100%;
    height: 100%;
    display: block;
    z-index: 0;
  }

  .noise-border {
    z-index: 1;
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.1),
      inset 0 -20px 36px rgba(0, 0, 0, 0.16);
  }

  .noise-glare {
    z-index: 1;
    background:
      linear-gradient(125deg, rgba(255, 255, 255, 0.24), transparent 24%, transparent 58%, rgba(255, 255, 255, 0.12) 80%, transparent),
      radial-gradient(circle at 50% -24%, rgba(255, 255, 255, 0.24), transparent 58%);
    mix-blend-mode: screen;
    opacity: 0.9;
    transition: opacity 220ms ease;
    pointer-events: none;
  }

  .noise-button:hover .noise-glare {
    opacity: 1;
  }

  .noise-label {
    position: relative;
    z-index: 2;
    text-shadow: 0 1px 0 rgba(0, 0, 0, 0.22);
    pointer-events: none;
    font-size: 18px;
  }
</style>
