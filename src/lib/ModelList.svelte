<script lang="ts">
  import type { ModelInfo, ModelStatus } from "../shared/types";

  interface Props {
    availableModels: ModelInfo[];
    modelsStatusMap: Map<string, ModelStatus>;
    selectedModel: string;
    isDownloading: boolean;
    statusLabel: string;
    pathLabel: string;
    onDownload: (id: string) => void;
    onSelect: (id: string) => void;
    onDelete: (id: string) => void;
  }

  const {
    availableModels, modelsStatusMap, selectedModel, isDownloading,
    statusLabel, pathLabel, onDownload, onSelect, onDelete,
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
    <div class="model-item" data-selected={isSelected}>
      <div class="model-item-info">
        <div class="model-item-title">
          {model.label}
          {#if isSelected}<span class="badge">当前</span>{/if}
          {#if installed}<span class="badge badge-installed">已安装</span>{/if}
        </div>
        <div class="model-item-desc">{model.description}</div>
      </div>
      <div class="model-item-actions">
        {#if !installed}
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
