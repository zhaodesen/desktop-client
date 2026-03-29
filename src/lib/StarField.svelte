<script lang="ts">
  import { onMount } from "svelte";
  import { tsParticles } from "@tsparticles/engine";
  import { loadSlim } from "@tsparticles/slim";

  let container: HTMLDivElement;

  onMount(() => {
    let instance: Awaited<ReturnType<typeof tsParticles.load>> | undefined;

    (async () => {
      await loadSlim(tsParticles);
      instance = await tsParticles.load({
        element: container,
        options: {
          fpsLimit: 60,
          particles: {
            number: {
              value: 140,
              density: { enable: true, width: 1920, height: 1080 },
            },
            color: {
              value: ["#e09545", "#d4a574", "#c9936a", "#b8845e", "#eab380"],
            },
            shape: { type: "circle" },
            opacity: {
              value: { min: 0.15, max: 0.7 },
              animation: {
                enable: true,
                speed: 0.4,
                sync: false,
              },
            },
            size: {
              value: { min: 0.5, max: 2.8 },
            },
            move: {
              enable: true,
              speed: { min: 0.08, max: 0.35 },
              direction: "none",
              random: true,
              straight: false,
              outModes: { default: "out" },
            },
            links: { enable: false },
          },
          interactivity: {
            events: {
              onHover: { enable: false },
              onClick: { enable: false },
            },
          },
          detectRetina: true,
          smooth: true,
        },
      });
    })();

    return () => {
      instance?.destroy();
    };
  });
</script>

<div class="star-field" bind:this={container}></div>

<style>
  .star-field {
    position: absolute;
    inset: 0;
    z-index: 0;
  }
</style>
