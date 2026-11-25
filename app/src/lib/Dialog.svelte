<script lang="ts">
  import { dialogState } from './dialogs.svelte';

  let value = $state('');
  let inputEl = $state<HTMLInputElement>();
  let okEl = $state<HTMLButtonElement>();

  $effect(() => {
    const d = dialogState.current;
    if (!d) return;
    value = d.input ?? '';
    queueMicrotask(() => (d.input !== undefined ? inputEl?.select() : okEl?.focus()));
  });

  function close(result: boolean) {
    const d = dialogState.current;
    if (!d) return;
    dialogState.current = null;
    d.resolve(result ? (d.input !== undefined ? value : true) : d.input !== undefined ? null : false);
  }
</script>

{#if dialogState.current}
  {@const d = dialogState.current}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && close(false)}>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="dlg-title"
      tabindex="-1"
      onkeydown={(e) => {
        if (e.key === 'Escape') close(false);
        if (e.key === 'Enter' && d.input !== undefined) close(true);
      }}
    >
      <h2 id="dlg-title">{d.title}</h2>
      {#if d.message}<p>{d.message}</p>{/if}
      {#if d.input !== undefined}
        <input bind:this={inputEl} bind:value aria-label={d.title} />
      {/if}
      <div class="buttons">
        <button class="ghost" onclick={() => close(false)}>{d.cancelLabel}</button>
        <button class="primary" class:danger={d.danger} bind:this={okEl} onclick={() => close(true)}>{d.okLabel}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 20, 30, 0.35);
    display: grid;
    place-items: center;
    z-index: 100;
  }
  .dialog {
    background: var(--panel);
    color: var(--text);
    border-radius: 12px;
    padding: 20px 22px 16px;
    width: min(480px, calc(100vw - 32px));
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    border: 1px solid var(--line);
  }
  h2 {
    margin: 0 0 8px;
    font-size: 16px;
  }
  p {
    margin: 0 0 14px;
    white-space: pre-line;
    line-height: 1.5;
    color: var(--text-2);
  }
  input {
    width: 100%;
    font: inherit;
    font-family: var(--mono);
    font-size: 13px;
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid var(--line-strong);
    background: var(--panel);
    color: var(--text);
    margin-bottom: 14px;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
