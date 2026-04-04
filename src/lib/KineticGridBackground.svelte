<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    active?: boolean;
    progress?: number;
  }

  type Palette = {
    background: [number, number, number];
    starCool: [number, number, number];
    starWarm: [number, number, number];
    cloudLight: [number, number, number];
    cloudWarm: [number, number, number];
    cloudMagenta: [number, number, number];
    dust: [number, number, number];
    vignette: [number, number, number];
  };

  const { active = false, progress = 0 }: Props = $props();

  const vertexShaderSource = `#version 300 es
in vec2 a_position;
out vec2 v_uv;

void main() {
  v_uv = a_position * 0.5 + 0.5;
  gl_Position = vec4(a_position, 0.0, 1.0);
}`;

  const fragmentShaderSource = `#version 300 es
precision highp float;
precision highp int;

in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_resolution;
uniform float u_time;
uniform float u_progress;
uniform float u_active;
uniform float u_reduce_motion;
uniform vec3 u_background;
uniform vec3 u_star_cool;
uniform vec3 u_star_warm;
uniform vec3 u_cloud_light;
uniform vec3 u_cloud_warm;
uniform vec3 u_cloud_magenta;
uniform vec3 u_dust;
uniform vec3 u_vignette;

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

float ridgeFbm(vec2 p) {
  float value = 0.0;
  float amplitude = 0.55;
  mat2 m = mat2(1.7, 1.1, -1.1, 1.7);
  for (int i = 0; i < 4; i++) {
    value += abs(noise(p) - 0.5) * 2.0 * amplitude;
    p = m * p;
    amplitude *= 0.55;
  }
  return value;
}

mat2 rotate2d(float a) {
  float s = sin(a);
  float c = cos(a);
  return mat2(c, -s, s, c);
}

vec3 starLayer(vec2 uv, float scale, float threshold, float sizeBoost, float twinkleShift) {
  vec2 gv = fract(uv * scale) - 0.5;
  vec2 id = floor(uv * scale);
  float seed = hash12(id);
  float mask = step(threshold, seed);
  vec2 jitter = vec2(hash12(id + 13.7), hash12(id + 31.1)) - 0.5;
  vec2 delta = gv - jitter * 0.6;
  float dist = length(delta);
  float flare = max(0.0, 1.0 - dist * (28.0 - sizeBoost * 10.0));
  flare = pow(flare, 5.0 - sizeBoost * 1.5);
  float crossA = max(0.0, 1.0 - abs(delta.x * delta.y) * (120.0 - sizeBoost * 30.0));
  crossA = pow(crossA, 8.0);
  float twinkle = 0.78 + 0.22 * sin(seed * 80.0 + u_time * (0.4 + seed * 1.8) + twinkleShift);
  vec3 tint = mix(u_star_warm, u_star_cool, hash12(id + 8.2));
  return tint * (flare + crossA * 0.35) * mask * twinkle;
}

vec3 starField(vec2 uv) {
  vec3 stars = vec3(0.0);
  stars += starLayer(uv, 180.0, 0.985, 0.2, 0.0);
  stars += starLayer(uv, 340.0, 0.992, 0.12, 1.1);
  stars += starLayer(uv, 38.0, 0.94, 1.0, 2.2);

  float grain = pow(hash12(floor(uv * vec2(1200.0, 680.0))), 18.0) * 0.85;
  stars += mix(u_star_warm, u_star_cool, hash12(floor(uv * 800.0) + 4.0)) * grain;
  return stars;
}

void main() {
  float aspect = u_resolution.x / max(u_resolution.y, 1.0);
  float t = u_reduce_motion > 0.5 ? 0.0 : u_time;
  float prog = clamp(u_progress, 0.0, 1.0);
  float activeMix = clamp(u_active, 0.0, 1.0);

  vec2 p = v_uv * 2.0 - 1.0;
  p.x *= aspect;
  vec2 rotated = rotate2d(-0.33) * p;

  float motionSpeed = mix(0.0035, 0.0065, activeMix * (0.7 + prog * 0.3));
  vec2 flow = vec2(t * motionSpeed, -t * motionSpeed * 0.55);
  vec2 cloudUv = rotated * vec2(1.45, 3.25);

  vec2 warpVec = vec2(
    fbm(cloudUv * 0.82 + flow * 1.6 + vec2(3.2, 1.7)),
    fbm(cloudUv * 0.82 - flow * 1.2 + vec2(9.4, 6.1))
  ) - 0.5;

  vec2 warpedCloudUv = cloudUv + warpVec * 0.95;

  float bandWidth = mix(0.18, 0.23, fbm(vec2(rotated.x * 1.35 + 4.0, 0.0)));
  float band = exp(-pow(abs(rotated.y) / bandWidth, 1.35));

  float cloudBase = fbm(warpedCloudUv * 1.0 + flow);
  float cloudDetail = fbm(warpedCloudUv * 2.4 - flow * 1.4 + vec2(7.0, 2.8));
  float cloudWisps = fbm(warpedCloudUv * 4.2 + flow * 2.0 + vec2(1.3, 9.7));
  float dustField = ridgeFbm(warpedCloudUv * 2.0 - flow * 1.15 + vec2(2.0, 11.0));

  float clouds = band * smoothstep(0.24, 0.92, cloudBase * 0.72 + cloudDetail * 0.28);
  float wisps = band * smoothstep(0.34, 0.92, cloudWisps);
  float dust = band * smoothstep(0.28, 0.9, dustField);

  float core = exp(-pow(rotated.x * 0.95 + 0.06, 2.0) * 4.0 - pow(rotated.y, 2.0) * 30.0);
  float bulge = exp(-pow(rotated.x * 1.8 - 0.24, 2.0) * 10.0 - pow(rotated.y, 2.0) * 45.0);

  float magentaMask = band * smoothstep(0.52, 0.88, fbm(warpedCloudUv * 3.1 + vec2(12.0, 4.0) - flow * 1.8));
  float warmMask = band * smoothstep(0.34, 0.86, cloudDetail + core * 0.55);

  vec3 color = u_background;
  color += starField(v_uv);
  color += u_cloud_light * clouds * (0.38 + core * 0.7);
  color += u_cloud_warm * warmMask * (0.24 + bulge * 0.68);
  color += u_cloud_magenta * magentaMask * 0.22;
  color += mix(u_cloud_light, u_cloud_warm, 0.45) * wisps * 0.16;
  color = mix(color, u_dust, dust * (0.58 + core * 0.12));
  color += u_cloud_light * core * 0.9;
  color += u_cloud_warm * bulge * 0.56;

  float skyHaze = fbm(p * vec2(1.5, 1.2) + vec2(t * 0.004, -t * 0.003));
  color += u_star_cool * skyHaze * 0.025;

  float vignetteMask = smoothstep(0.34, 1.20, length((v_uv - 0.5) * vec2(1.22, 1.04)) * 1.32);
  color = mix(color, u_vignette, vignetteMask * 0.54);

  color = color / (1.0 + color);
  color = pow(color, vec3(0.92));

  fragColor = vec4(color, 1.0);
}`;

  let canvas: HTMLCanvasElement;

  onMount(() => {
    const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    let reduceMotion = motionQuery.matches;

    const gl = canvas.getContext("webgl2", {
      alpha: true,
      antialias: false,
      depth: false,
      stencil: false,
      premultipliedAlpha: true,
      preserveDrawingBuffer: false,
      powerPreference: "high-performance",
    });

    if (!gl) {
      return;
    }

    let program: WebGLProgram | null = null;
    let buffer: WebGLBuffer | null = null;
    let rafId = 0;
    let visible = !document.hidden;
    let lastFrameTime = 0;
    let lastRenderNow = 0;
    let startTime = performance.now();
    let palette = readPalette();

    function clamp(value: number, min: number, max: number) {
      return Math.max(min, Math.min(max, value));
    }

    function parseAccentRgb(): [number, number, number] {
      const accent = getComputedStyle(document.documentElement).getPropertyValue("--accent-rgb").trim();
      const [r = "224", g = "149", b = "69"] = accent.split(",").map((item) => item.trim());
      return [
        clamp(Number.parseInt(r, 10) || 224, 0, 255),
        clamp(Number.parseInt(g, 10) || 149, 0, 255),
        clamp(Number.parseInt(b, 10) || 69, 0, 255),
      ];
    }

    function mixRgb(
      from: [number, number, number],
      to: [number, number, number],
      weight: number,
    ): [number, number, number] {
      return [
        Math.round(from[0] + ((to[0] - from[0]) * weight)),
        Math.round(from[1] + ((to[1] - from[1]) * weight)),
        Math.round(from[2] + ((to[2] - from[2]) * weight)),
      ];
    }

    function toUnit(rgb: [number, number, number]) {
      return [rgb[0] / 255, rgb[1] / 255, rgb[2] / 255] as const;
    }

    function readPalette(): Palette {
      const accent = parseAccentRgb();
      const theme = document.documentElement.getAttribute("data-theme") === "light" ? "light" : "dark";

      if (theme === "light") {
        return {
          background: toUnit([242, 239, 235]),
          starCool: toUnit([200, 212, 255]),
          starWarm: toUnit([255, 232, 205]),
          cloudLight: toUnit([246, 240, 230]),
          cloudWarm: toUnit(mixRgb(accent, [228, 198, 166], 0.52)),
          cloudMagenta: toUnit([214, 168, 214]),
          dust: toUnit([198, 186, 176]),
          vignette: toUnit([230, 224, 216]),
        };
      }

      return {
        background: toUnit([6, 8, 14]),
        starCool: toUnit([168, 194, 255]),
        starWarm: toUnit([255, 214, 166]),
        cloudLight: toUnit([234, 233, 244]),
        cloudWarm: toUnit(mixRgb(accent, [243, 210, 164], 0.48)),
        cloudMagenta: toUnit([210, 118, 212]),
        dust: toUnit([18, 16, 20]),
        vignette: toUnit([4, 6, 10]),
      };
    }

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
      const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
      const width = Math.max(1, Math.round(canvas.clientWidth * dpr));
      const height = Math.max(1, Math.round(canvas.clientHeight * dpr));

      if (canvas.width === width && canvas.height === height) return;
      canvas.width = width;
      canvas.height = height;
      gl.viewport(0, 0, width, height);
    }

    function drawFrame(time: number) {
      if (!program || !buffer) return;

      resizeCanvas();

      gl.useProgram(program);

      const positionLocation = gl.getAttribLocation(program, "a_position");
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
      gl.enableVertexAttribArray(positionLocation);
      gl.vertexAttribPointer(positionLocation, 2, gl.FLOAT, false, 0, 0);

      gl.uniform2f(gl.getUniformLocation(program, "u_resolution"), canvas.width, canvas.height);
      gl.uniform1f(gl.getUniformLocation(program, "u_time"), time);
      gl.uniform1f(gl.getUniformLocation(program, "u_progress"), clamp(progress, 0, 1));
      gl.uniform1f(gl.getUniformLocation(program, "u_active"), active ? 1 : 0);
      gl.uniform1f(gl.getUniformLocation(program, "u_reduce_motion"), reduceMotion ? 1 : 0);
      gl.uniform3f(gl.getUniformLocation(program, "u_background"), ...palette.background);
      gl.uniform3f(gl.getUniformLocation(program, "u_star_cool"), ...palette.starCool);
      gl.uniform3f(gl.getUniformLocation(program, "u_star_warm"), ...palette.starWarm);
      gl.uniform3f(gl.getUniformLocation(program, "u_cloud_light"), ...palette.cloudLight);
      gl.uniform3f(gl.getUniformLocation(program, "u_cloud_warm"), ...palette.cloudWarm);
      gl.uniform3f(gl.getUniformLocation(program, "u_cloud_magenta"), ...palette.cloudMagenta);
      gl.uniform3f(gl.getUniformLocation(program, "u_dust"), ...palette.dust);
      gl.uniform3f(gl.getUniformLocation(program, "u_vignette"), ...palette.vignette);
      gl.drawArrays(gl.TRIANGLES, 0, 6);
    }

    function render(now: number) {
      rafId = 0;
      if (!visible) return;

      const time = (now - startTime) * 0.001;
      const frameBudget = active ? (1000 / 36) : (1000 / 24);

      if (lastRenderNow !== 0 && (now - lastRenderNow) < frameBudget) {
        startLoop();
        return;
      }

      lastRenderNow = now;
      lastFrameTime = time;
      drawFrame(reduceMotion ? 0 : time);
      startLoop();
    }

    function startLoop() {
      if (rafId || !visible) return;
      rafId = requestAnimationFrame(render);
    }

    function stopLoop() {
      if (!rafId) return;
      cancelAnimationFrame(rafId);
      rafId = 0;
    }

    function handleVisibilityChange() {
      visible = !document.hidden;
      if (!visible) {
        stopLoop();
        return;
      }

      startTime = performance.now() - (lastFrameTime * 1000);
      lastRenderNow = 0;
      drawFrame(reduceMotion ? 0 : lastFrameTime);
      startLoop();
    }

    function handleMotionChange(event: MediaQueryListEvent) {
      reduceMotion = event.matches;
      lastRenderNow = 0;
      drawFrame(reduceMotion ? 0 : lastFrameTime);
      startLoop();
    }

    const resizeObserver = new ResizeObserver(() => {
      resizeCanvas();
      drawFrame(reduceMotion ? 0 : lastFrameTime);
    });

    const themeObserver = new MutationObserver(() => {
      palette = readPalette();
      drawFrame(reduceMotion ? 0 : lastFrameTime);
    });

    try {
      program = createProgram(gl);
    } catch (error) {
      console.error("KineticGridBackground shader init failed", error);
      return;
    }

    buffer = gl.createBuffer();
    if (!buffer) {
      gl.deleteProgram(program);
      return;
    }

    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([
      -1, -1,
       1, -1,
      -1,  1,
      -1,  1,
       1, -1,
       1,  1,
    ]), gl.STATIC_DRAW);

    resizeObserver.observe(canvas);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme", "style"],
    });
    document.addEventListener("visibilitychange", handleVisibilityChange);
    motionQuery.addEventListener("change", handleMotionChange);

    drawFrame(0);
    startLoop();

    return () => {
      stopLoop();
      resizeObserver.disconnect();
      themeObserver.disconnect();
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      motionQuery.removeEventListener("change", handleMotionChange);
      if (buffer) gl.deleteBuffer(buffer);
      if (program) gl.deleteProgram(program);
    };
  });
</script>

<canvas class="kinetic-grid" bind:this={canvas} aria-hidden="true"></canvas>

<style>
  .kinetic-grid {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
    pointer-events: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .kinetic-grid {
      pointer-events: none;
    }
  }
</style>
