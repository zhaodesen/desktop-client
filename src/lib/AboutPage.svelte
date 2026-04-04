<script lang="ts">
  import { onDestroy } from "svelte";

  interface Props {
    avatarSrc: string;
  }

  const EMAIL_ADDRESS = "1026108026@qq.com";

  const { avatarSrc }: Props = $props();
  let avatarLoadFailed = $state(false);
  const avatarAvailable = $derived(Boolean(avatarSrc) && !avatarLoadFailed);

  let showCopyToast = $state(false);
  let copyToastTimer: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => {
    clearTimeout(copyToastTimer);
  });

  async function handleCopyEmail(event: MouseEvent) {
    event.preventDefault();

    try {
      await navigator.clipboard.writeText(EMAIL_ADDRESS);
      showCopyToast = true;
      clearTimeout(copyToastTimer);
      copyToastTimer = setTimeout(() => {
        showCopyToast = false;
      }, 2200);
    } catch (error) {
      console.error("复制邮箱地址失败", error);
    }
  }

  function handleAvatarLoadError() {
    avatarLoadFailed = true;
  }
</script>

<section class="page about-page" data-active="true">
  {#if showCopyToast}
    <div class="about-copy-toast" role="status" aria-live="polite">
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <polyline points="20 6 9 17 4 12" />
      </svg>
      <span>已复制邮箱地址</span>
    </div>
  {/if}

  <div class="about-scene" aria-hidden="true">
    <span class="scene-glow glow-a"></span>
    <span class="scene-glow glow-b"></span>
    <span class="scene-orbit orbit-a"></span>
    <span class="scene-orbit orbit-b"></span>
    <span class="scene-orbit orbit-c"></span>
    <span class="scene-line line-a"></span>
    <span class="scene-line line-b"></span>
    <span class="scene-line line-c"></span>
  </div>

  <div class="about-content">
    <div class="about-copy">
      <h1>
        <span>大家好! 我是</span>
        <span class="about-signature">Sen</span>
      </h1>
      <p class="about-intro">
       我喜欢一边工作，一边听听音乐，
       有时候也想顺便学点英语，但发现光听其实不太够，还得看到内容才更容易理解 
       所以我做了这个工具，用 AI 把音视频转成文字
       这样就可以一边听，一边看，更容易学习 😊
       <br/>
       <br/>

       希望这个工具也可以帮助到你，如果在使用过程中遇到问题，我会尽快修复，也欢迎通过
        <a
          href={`mailto:${EMAIL_ADDRESS}`}
          onclick={handleCopyEmail}
          aria-label={`点击复制邮箱地址 ${EMAIL_ADDRESS}`}
          title="点击复制邮箱地址"
        >
          {EMAIL_ADDRESS}
        </a>
        联系我。
      </p>
    </div>

    <div class="about-model" aria-hidden="true">
      <div class="model-stage">
        <div class="model-backdrop"></div>
        <div class="model-halo"></div>
        <div class="model-ring"></div>
        <div class="model-ring model-ring-delayed"></div>
        {#if avatarAvailable}
          <img class="about-model-shadow" src={avatarSrc} alt="" />
        {/if}
        <div class="model-plane"></div>
        {#if avatarAvailable}
          <img class="about-model-image" src={avatarSrc} alt="" onerror={handleAvatarLoadError} />
        {:else}
          <div class="about-model-fallback">
            <span class="about-model-fallback-mark">S</span>
            <span class="about-model-fallback-ring"></span>
          </div>
        {/if}
        <span class="model-foreground-shard shard-a"></span>
        <span class="model-foreground-shard shard-b"></span>
        <span class="model-scanline"></span>
      </div>
    </div>
  </div>
</section>

<style>
  .about-page {
    position: relative;
    min-height: calc(100vh - 96px);
    display: grid;
    place-items: center;
    padding: 24px 0;
    overflow: hidden;
  }

  .about-scene {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
  }

  .about-copy-toast {
    position: fixed;
    top: 24px;
    left: calc(var(--sidebar-w, 220px) + (100vw - var(--sidebar-w, 220px)) / 2);
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 18px;
    border-radius: var(--radius-pill);
    background: var(--bg-glass);
    border: 1px solid var(--border);
    color: var(--success);
    font-size: var(--font-sm);
    font-weight: 600;
    box-shadow: var(--shadow-md);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    z-index: var(--z-toast);
    animation: about-toast-enter 220ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .scene-orbit,
  .scene-line,
  .scene-glow {
    position: absolute;
    display: block;
  }

  .scene-glow {
    border-radius: 999px;
    filter: blur(28px);
    opacity: 0.34;
    animation: glowPulse 8s ease-in-out infinite;
  }

  .glow-a {
    width: 260px;
    height: 260px;
    top: 14%;
    right: 14%;
    background: rgba(var(--accent-rgb), 0.26);
  }

  .glow-b {
    width: 190px;
    height: 190px;
    left: 10%;
    bottom: 14%;
    background: rgba(92, 142, 255, 0.18);
    animation-delay: -2.8s;
  }

  .scene-orbit {
    border-radius: 999px;
    border: 1px solid rgba(var(--accent-rgb), 0.18);
    animation: orbitFloat 9s ease-in-out infinite;
  }

  .orbit-a {
    width: 340px;
    height: 340px;
    top: 8%;
    right: 10%;
  }

  .orbit-b {
    width: 220px;
    height: 220px;
    bottom: 12%;
    left: 8%;
    animation-delay: -3s;
  }

  .orbit-c {
    width: 520px;
    height: 520px;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    opacity: 0.18;
    animation-duration: 14s;
  }

  .scene-line {
    height: 1px;
    background: linear-gradient(90deg, transparent, rgba(var(--accent-rgb), 0.42), transparent);
    transform-origin: left center;
    animation: lineDrift 6s ease-in-out infinite;
  }

  .line-a {
    width: 220px;
    top: 28%;
    left: 6%;
  }

  .line-b {
    width: 260px;
    right: 4%;
    bottom: 26%;
    animation-delay: -2.4s;
  }

  .line-c {
    width: 320px;
    top: 52%;
    right: 18%;
    animation-delay: -1.2s;
  }

  .about-content {
    position: relative;
    z-index: 1;
    width: min(1040px, 100%);
    display: grid;
    grid-template-columns: minmax(320px, 440px) minmax(360px, 1fr);
    align-items: center;
    gap: 32px;
    padding: 0 12px;
  }

  .about-copy {
    max-width: 540px;
  }

  .about-model {
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 620px;
  }

  .model-stage {
    position: relative;
    width: min(470px, 100%);
    aspect-ratio: 4 / 5;
    display: grid;
    place-items: center;
    animation: modelFloat 5.2s ease-in-out infinite;
  }

  .model-backdrop {
    position: absolute;
    inset: 12% 10% 10%;
    border-radius: 44% 56% 52% 48% / 26% 26% 74% 74%;
    background:
      radial-gradient(circle at 50% 35%, rgba(114, 162, 255, 0.24), transparent 38%),
      radial-gradient(circle at 50% 70%, rgba(var(--accent-rgb), 0.24), transparent 48%),
      linear-gradient(180deg, rgba(255, 255, 255, 0.06), rgba(255, 255, 255, 0.01));
    filter: blur(10px);
    opacity: 0.9;
    animation: backdropShift 8.2s ease-in-out infinite;
  }

  .model-halo {
    position: absolute;
    inset: 8% 10% 10%;
    border-radius: 50%;
    background:
      radial-gradient(circle, rgba(var(--accent-rgb), 0.24), rgba(var(--accent-rgb), 0.04) 55%, transparent 72%);
    filter: blur(12px);
    animation: haloBreath 6.6s ease-in-out infinite;
  }

  .model-ring {
    position: absolute;
    inset: 7%;
    border-radius: 50%;
    border: 1px solid rgba(var(--accent-rgb), 0.24);
    animation: ringSpin 16s linear infinite;
  }

  .model-ring::before,
  .model-ring::after {
    content: "";
    position: absolute;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 16px rgba(var(--accent-rgb), 0.4);
  }

  .model-ring::before {
    top: 12%;
    left: 16%;
  }

  .model-ring::after {
    right: 12%;
    bottom: 18%;
  }

  .model-ring-delayed {
    inset: 16%;
    border-style: dashed;
    border-color: rgba(255, 255, 255, 0.14);
    animation-direction: reverse;
    animation-duration: 20s;
  }

  .about-model-shadow {
    position: absolute;
    z-index: 0;
    width: 86%;
    height: 90%;
    object-fit: cover;
    object-position: 46% 18%;
    border-radius: 36px;
    clip-path: polygon(18% 3%, 84% 7%, 100% 23%, 94% 88%, 74% 100%, 18% 96%, 0 70%, 5% 18%);
    filter: blur(18px) saturate(1.25);
    opacity: 0.28;
    transform: translate(14px, 20px) scale(1.02);
    animation: shadowParallax 6.2s ease-in-out infinite;
  }

  .model-plane {
    position: absolute;
    z-index: 1;
    inset: 14% 14% 6%;
    border-radius: 42px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.12), rgba(255, 255, 255, 0.02) 20%, rgba(255, 255, 255, 0.02)),
      linear-gradient(180deg, rgba(13, 16, 22, 0.18), rgba(13, 16, 22, 0));
    border: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(8px);
    transform: translateY(4px);
  }

  .about-model-image {
    position: relative;
    z-index: 2;
    width: 86%;
    height: 92%;
    object-fit: cover;
    object-position: 46% 16%;
    border-radius: 36px;
    clip-path: polygon(18% 3%, 84% 7%, 100% 23%, 94% 88%, 74% 100%, 18% 96%, 0 70%, 5% 18%);
    box-shadow:
      0 28px 64px rgba(0, 0, 0, 0.28),
      0 0 0 1px rgba(255, 255, 255, 0.08);
    filter: saturate(1.08) contrast(1.05);
    animation: portraitParallax 5.6s ease-in-out infinite;
  }

  .about-model-fallback {
    position: relative;
    z-index: 2;
    width: 86%;
    height: 92%;
    border-radius: 36px;
    clip-path: polygon(18% 3%, 84% 7%, 100% 23%, 94% 88%, 74% 100%, 18% 96%, 0 70%, 5% 18%);
    display: grid;
    place-items: center;
    overflow: hidden;
    background:
      radial-gradient(circle at 30% 22%, rgba(255, 255, 255, 0.35), transparent 28%),
      radial-gradient(circle at 72% 18%, rgba(105, 168, 255, 0.38), transparent 24%),
      linear-gradient(160deg, rgba(12, 18, 36, 0.92), rgba(30, 64, 124, 0.88));
    box-shadow:
      0 28px 64px rgba(0, 0, 0, 0.28),
      0 0 0 1px rgba(255, 255, 255, 0.08);
    animation: portraitParallax 5.6s ease-in-out infinite;
  }

  .about-model-fallback::before {
    content: "";
    position: absolute;
    inset: 14px;
    border-radius: 28px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.12), transparent 48%),
      radial-gradient(circle at center, rgba(var(--accent-rgb), 0.18), transparent 62%);
  }

  .about-model-fallback-mark {
    position: relative;
    z-index: 1;
    font-size: clamp(4.6rem, 11vw, 7rem);
    font-weight: 700;
    letter-spacing: 0.12em;
    color: rgba(255, 255, 255, 0.92);
    text-shadow: 0 0 36px rgba(var(--accent-rgb), 0.42);
  }

  .about-model-fallback-ring {
    position: absolute;
    width: 70%;
    aspect-ratio: 1;
    border-radius: 50%;
    border: 1px solid rgba(var(--accent-rgb), 0.34);
    box-shadow: 0 0 32px rgba(var(--accent-rgb), 0.22);
    animation: orbitPulse 4.8s ease-in-out infinite;
  }

  :global([data-theme="light"]) .about-model-image {
    box-shadow:
      0 24px 48px rgba(18, 24, 32, 0.12),
      0 0 0 1px rgba(0, 0, 0, 0.06);
  }

  :global([data-theme="light"]) .about-model-fallback {
    background:
      radial-gradient(circle at 30% 22%, rgba(255, 255, 255, 0.78), transparent 28%),
      radial-gradient(circle at 72% 18%, rgba(105, 168, 255, 0.3), transparent 24%),
      linear-gradient(160deg, rgba(233, 240, 252, 0.94), rgba(184, 210, 255, 0.9));
    box-shadow:
      0 24px 48px rgba(18, 24, 32, 0.12),
      0 0 0 1px rgba(0, 0, 0, 0.06);
  }

  :global([data-theme="light"]) .about-model-fallback-mark {
    color: rgba(28, 53, 98, 0.88);
    text-shadow: 0 0 28px rgba(var(--accent-rgb), 0.18);
  }

  .model-foreground-shard {
    position: absolute;
    z-index: 3;
    border-radius: 999px;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.24), rgba(255, 255, 255, 0.02));
    border: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(10px);
    box-shadow: 0 12px 28px rgba(0, 0, 0, 0.12);
  }

  .shard-a {
    --tilt: 12deg;
    --tilt-hover: 16deg;
    width: 88px;
    height: 220px;
    right: 4%;
    top: 18%;
    transform: rotate(var(--tilt));
    animation: foregroundFloat 4.8s ease-in-out infinite;
  }

  .shard-b {
    --tilt: -14deg;
    --tilt-hover: -18deg;
    width: 66px;
    height: 160px;
    left: 8%;
    bottom: 12%;
    transform: rotate(var(--tilt));
    animation: foregroundFloat 5.8s ease-in-out infinite reverse;
  }

  .model-scanline {
    position: absolute;
    z-index: 4;
    left: 16%;
    right: 16%;
    height: 2px;
    top: 24%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.9), transparent);
    box-shadow: 0 0 22px rgba(255, 255, 255, 0.35);
    animation: scanlineSweep 6.4s ease-in-out infinite;
  }

  .about-copy h1 {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.18em;
    font-size: clamp(2rem, 4vw, 3.2rem);
    line-height: 1.05;
    letter-spacing: -0.05em;
    margin-bottom: 18px;
  }

  .about-signature {
    position: relative;
    display: inline-block;
    padding: 0.02em 0.18em 0.12em;
    letter-spacing: -0.08em;
    color: transparent;
    background:
      linear-gradient(135deg, rgba(255, 244, 222, 0.98) 0%, rgba(255, 211, 151, 0.96) 26%, rgba(var(--accent-rgb), 1) 68%, rgba(255, 236, 203, 0.92) 100%);
    -webkit-background-clip: text;
    background-clip: text;
    filter: drop-shadow(0 10px 22px rgba(var(--accent-rgb), 0.26));
    animation: signatureFloat 4.8s ease-in-out infinite;
  }

  .about-signature::before,
  .about-signature::after {
    content: "";
    position: absolute;
    pointer-events: none;
  }

  .about-signature::before {
    inset: 58% -3% -2%;
    border-radius: 999px;
    background: linear-gradient(90deg, rgba(var(--accent-rgb), 0), rgba(var(--accent-rgb), 0.28) 20%, rgba(255, 223, 181, 0.78) 48%, rgba(var(--accent-rgb), 0.22) 78%, rgba(var(--accent-rgb), 0));
    filter: blur(9px);
    opacity: 0.9;
    z-index: -2;
  }

  .about-signature::after {
    left: 0.18em;
    right: 0.12em;
    bottom: 0.08em;
    height: 0.16em;
    border-radius: 999px;
    background: linear-gradient(90deg, rgba(var(--accent-rgb), 0.24), rgba(255, 227, 184, 0.96), rgba(var(--accent-rgb), 0.24));
    box-shadow: 0 0 20px rgba(var(--accent-rgb), 0.18);
    z-index: -1;
  }

  .about-intro {
    font-size: 1rem;
    line-height: 1.9;
    color: var(--text-secondary);
  }

  .about-intro a {
    cursor: pointer;
    color: var(--accent);
    text-decoration: none;
    border-bottom: 1px solid rgba(var(--accent-rgb), 0.28);
    transition:
      color 160ms ease,
      border-color 160ms ease,
      opacity 160ms ease;
  }

  .about-intro a:hover {
    color: var(--accent-hover);
    border-bottom-color: rgba(var(--accent-rgb), 0.48);
  }

  @media (max-width: 1120px) {
    .about-page {
      padding: 18px 0;
    }

    .about-content {
      width: min(920px, 100%);
      grid-template-columns: minmax(300px, 1.05fr) minmax(280px, 0.82fr);
      gap: 20px;
      padding: 0 14px;
    }

    .about-copy {
      max-width: 100%;
    }

    .about-copy h1 {
      font-size: clamp(1.88rem, 3.5vw, 2.8rem);
      margin-bottom: 14px;
    }

    .about-intro {
      font-size: 0.94rem;
      line-height: 1.78;
    }

    .about-model {
      min-height: 500px;
    }

    .model-stage {
      width: min(360px, 100%);
    }

    .scene-line {
      opacity: 0.28;
    }

    .orbit-c {
      width: 440px;
      height: 440px;
    }
  }

  @keyframes orbitFloat {
    0%, 100% {
      transform: translateY(0) scale(1);
      opacity: 0.45;
    }
    50% {
      transform: translateY(-12px) scale(1.04);
      opacity: 0.8;
    }
  }

  @keyframes glowPulse {
    0%, 100% {
      transform: scale(0.94);
      opacity: 0.18;
    }
    50% {
      transform: scale(1.08);
      opacity: 0.36;
    }
  }

  @keyframes lineDrift {
    0%, 100% {
      transform: translateX(0) scaleX(0.88);
      opacity: 0.3;
    }
    50% {
      transform: translateX(18px) scaleX(1);
      opacity: 0.8;
    }
  }

  @keyframes modelFloat {
    0%, 100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-14px);
    }
  }

  @keyframes backdropShift {
    0%, 100% {
      transform: translate3d(-8px, 8px, 0) scale(0.98);
      opacity: 0.82;
    }
    50% {
      transform: translate3d(10px, -10px, 0) scale(1.03);
      opacity: 1;
    }
  }

  @keyframes haloBreath {
    0%, 100% {
      transform: scale(0.94);
      opacity: 0.42;
    }
    50% {
      transform: scale(1.06);
      opacity: 0.82;
    }
  }

  @keyframes ringSpin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes shadowParallax {
    0%, 100% {
      transform: translate(14px, 20px) scale(1.02);
      opacity: 0.24;
    }
    50% {
      transform: translate(22px, 8px) scale(1.05);
      opacity: 0.34;
    }
  }

  @keyframes portraitParallax {
    0%, 100% {
      transform: translate3d(0, 0, 0);
    }
    50% {
      transform: translate3d(0, -10px, 0);
    }
  }

  @keyframes orbitPulse {
    0%, 100% {
      transform: scale(0.92);
      opacity: 0.42;
    }
    50% {
      transform: scale(1.04);
      opacity: 0.82;
    }
  }

  @keyframes foregroundFloat {
    0%, 100% {
      transform: translateY(0) rotate(var(--tilt));
      opacity: 0.46;
    }
    50% {
      transform: translateY(-14px) rotate(var(--tilt-hover));
      opacity: 0.82;
    }
  }

  @keyframes scanlineSweep {
    0%, 100% {
      top: 24%;
      opacity: 0;
    }
    12% {
      opacity: 0.8;
    }
    50% {
      top: 82%;
      opacity: 0.6;
    }
    68% {
      opacity: 0;
    }
  }

  @keyframes signatureFloat {
    0%, 100% {
      transform: translateY(0);
      filter: drop-shadow(0 10px 22px rgba(var(--accent-rgb), 0.22));
    }
    50% {
      transform: translateY(-3px);
      filter: drop-shadow(0 14px 30px rgba(var(--accent-rgb), 0.34));
    }
  }

  @keyframes about-toast-enter {
    from {
      opacity: 0;
      transform: translate(-50%, -8px);
    }
    to {
      opacity: 1;
      transform: translate(-50%, 0);
    }
  }

  @media (max-width: 860px) {
    .about-page {
      min-height: auto;
      padding: 12px 0 28px;
    }

    .about-content {
      grid-template-columns: 1fr;
      gap: 20px;
      justify-items: center;
      text-align: center;
    }

    .about-copy {
      max-width: 620px;
    }

    .about-copy h1 {
      justify-content: center;
    }

    .about-model {
      min-height: 420px;
      width: min(420px, 100%);
    }

    .model-stage {
      width: min(340px, 100%);
    }

    .about-copy-toast {
      top: 18px;
      left: 50%;
      max-width: calc(100vw - 32px);
    }
  }
</style>
