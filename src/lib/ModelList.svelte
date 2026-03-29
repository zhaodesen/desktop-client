<script lang="ts">
  import type { ModelInfo, ModelStatus } from "../shared/types";

  interface Props {
    availableModels: ModelInfo[];
    modelsStatusMap: Map<string, ModelStatus>;
    selectedModel: string;
    isDownloading: boolean;
    downloadingModelId: string | undefined;
    modelDownloadPercent: number;
    isDownloadPaused: boolean;
    statusLabel: string;
    pathLabel: string;
    onDownload: (id: string) => void;
    onCancel: () => void;
    onPause: () => void;
    onResume: () => void;
    onSelect: (id: string) => void;
    onDelete: (id: string) => void;
  }

  const {
    availableModels, modelsStatusMap, selectedModel, isDownloading,
    downloadingModelId, modelDownloadPercent, isDownloadPaused,
    statusLabel, pathLabel,
    onDownload, onCancel, onPause, onResume, onSelect, onDelete,
  }: Props = $props();
</script>

<div class="model-info">
  <p class="text-dim">{statusLabel}</p>
  <p class="text-dim text-xs">{pathLabel}</p>
</div>

<div class="model-list">
  {#each availableModels as model (model.id)}
    {@const status = modelsStatusMap.get(model.id)}
    {@const installed = status?.installed ?? false}
    {@const isSelected = model.id === selectedModel}
    {@const isThisDownloading = downloadingModelId === model.id}
    <div
      class="model-item"
      class:model-item-downloading={isThisDownloading}
      data-selected={isSelected}
    >
      {#if isThisDownloading}
        <div class="model-item-progress" style="width: {Math.max(0, Math.min(100, modelDownloadPercent))}%"></div>
      {/if}
      <div class="model-item-info">
        <div class="model-item-title">
          {model.label}
          {#if isSelected}<span class="badge">当前</span>{/if}
          {#if installed}<span class="badge badge-installed">已安装</span>{/if}
        </div>
        <div class="model-item-desc">{model.description}</div>
      </div>
      <div class="model-item-actions">
        {#if isThisDownloading}
          {#if isDownloadPaused}
            <button class="btn btn-sm" onclick={onResume}>继续</button>
          {:else}
            <button class="btn btn-sm btn-outline" onclick={onPause}>暂停</button>
          {/if}
          <button class="btn btn-sm btn-danger" onclick={onCancel}>取消</button>
        {:else if !installed}
          <button class="btn btn-sm btn-outline" disabled={isDownloading} onclick={() => onDownload(model.id)}>下载</button>
        {:else if !isSelected}
          <button class="btn btn-sm" onclick={() => onSelect(model.id)}>选用</button>
        {/if}
        {#if installed}
          <button class="btn btn-sm btn-danger" onclick={() => onDelete(model.id)}>删除</button>
        {/if}
      </div>
    </div>
  {/each}
</div>
