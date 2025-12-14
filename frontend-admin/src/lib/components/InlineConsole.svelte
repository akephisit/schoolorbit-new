<script lang="ts">
  import type { LogMessage, Progress } from '$lib/utils/sse';
  
  interface Props {
    logs?: LogMessage[];
    isLoading?: boolean;
    progress?: Progress;
  }
  
  let { logs = [], isLoading = false, progress }: Props = $props();
  
  let consoleEl: HTMLDivElement;
  
  // Auto-scroll เมื่อมี log ใหม่
  $effect(() => {
    if (logs.length && consoleEl) {
      consoleEl.scrollTop = consoleEl.scrollHeight;
    }
  });
  
  function formatTime(date: Date): string {
    return date.toLocaleTimeString('th-TH', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
  }
  
  function getLevelIcon(level: string): string {
    switch (level) {
      case 'success': return '✅';
      case 'error': return '❌';
      case 'warning': return '⚠️';
      default: return 'ℹ️';
    }
  }
</script>

<div class="inline-console">
	{#if progress}
		<div class="progress-bar">
			<div class="progress-fill" style="width: {(progress.step / progress.total) * 100}%"></div>
			<span class="progress-text">
				Step {progress.step}/{progress.total}: {progress.message}
			</span>
		</div>
	{/if}

	<div class="console-output" bind:this={consoleEl}>
		{#each logs as log, i (i)}
			<div class="log-line {log.level}">
				<span class="time">{formatTime(log.timestamp)}</span>
				<span class="icon">{getLevelIcon(log.level)}</span>
				<span class="message">{log.message}</span>
			</div>
		{/each}

		{#if isLoading && logs.length === 0}
			<div class="log-line loading">
				<span class="spinner">⏳</span>
				<span class="message">Initializing...</span>
			</div>
		{/if}
	</div>
</div>

<style>
  .inline-console {
    background: #1e1e1e;
    border-radius: 8px;
    overflow: hidden;
    margin-top: 1rem;
    border: 1px solid #333;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  }
  
  .progress-bar {
    position: relative;
    height: 36px;
    background: #2d2d2d;
    border-bottom: 1px solid #333;
  }
  
  .progress-fill {
    position: absolute;
    height: 100%;
    background: linear-gradient(90deg, #3b82f6, #60a5fa);
    transition: width 0.3s ease;
  }
  
  .progress-text {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #fff;
    font-size: 0.875rem;
    font-weight: 500;
    z-index: 1;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  }
  
  .console-output {
    max-height: 300px;
    overflow-y: auto;
    padding: 0.75rem;
    font-family: 'Monaco', 'Menlo', 'Courier New', monospace;
    font-size: 0.813rem;
    line-height: 1.6;
  }
  
  .log-line {
    display: flex;
    gap: 0.5rem;
    padding: 0.25rem 0;
    color: #d4d4d4;
    animation: fadeIn 0.2s ease;
  }
  
  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(-2px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  
  .log-line.success { color: #4ade80; }
  .log-line.error { color: #ef4444; }
  .log-line.warning { color: #fbbf24; }
  .log-line.info { color: #60a5fa; }
  
  .time {
    color: #737373;
    flex-shrink: 0;
    font-size: 0.75rem;
  }
  
  .icon {
    flex-shrink: 0;
  }
  
  .message {
    flex: 1;
  }
  
  .spinner {
    display: inline-block;
    animation: spin 1s linear infinite;
  }
  
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  
  /* Scrollbar styling */
  .console-output::-webkit-scrollbar {
    width: 8px;
  }
  
  .console-output::-webkit-scrollbar-track {
    background: #2d2d2d;
  }
  
  .console-output::-webkit-scrollbar-thumb {
    background: #525252;
    border-radius: 4px;
  }
  
  .console-output::-webkit-scrollbar-thumb:hover {
    background: #737373;
  }
</style>
