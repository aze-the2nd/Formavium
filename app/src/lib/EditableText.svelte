<script lang="ts">
  // Direkt bearbeitbarer Text ohne Rahmen: sieht aus wie der Inhalt, wird
  // beim Tippen (kurz verzögert) und beim Verlassen übernommen.
  import { onMount, untrack } from 'svelte';

  type Props = {
    value: string;
    label: string;
    multiline?: boolean;
    placeholder?: string;
    /** Beim Einblenden fokussieren und alles markieren. */
    autofocus?: boolean;
    onCommit: (value: string) => unknown;
    onKey?: (e: KeyboardEvent) => void;
    onBlur?: () => void;
    [attribute: string]: unknown;
  };
  let {
    value,
    label,
    multiline = false,
    placeholder = '',
    autofocus = false,
    onCommit,
    onKey,
    onBlur,
    ...rest
  }: Props = $props();

  let draft = $state(untrack(() => value));
  let el = $state<HTMLInputElement | HTMLTextAreaElement>();
  let focused = false;
  let outstanding = 0;
  let lastSent = '';
  let timer: ReturnType<typeof setTimeout> | undefined;

  // Stand des Dokuments übernehmen, außer während der eigenen Eingabe:
  // Antworten auf gesendete Zwischenstände dürfen Getipptes nicht ersetzen.
  $effect(() => {
    const v = value;
    untrack(() => {
      if (!focused || (outstanding === 0 && timer === undefined)) draft = v;
    });
  });

  function send() {
    clearTimeout(timer);
    timer = undefined;
    if (draft === (outstanding > 0 ? lastSent : value)) return;
    lastSent = draft;
    outstanding++;
    Promise.resolve(onCommit(draft))
      .catch(() => undefined)
      .finally(() => {
        outstanding--;
        if (!focused && outstanding === 0) draft = value;
      });
  }

  function input() {
    clearTimeout(timer);
    timer = setTimeout(send, 450);
  }

  function keydown(e: KeyboardEvent) {
    // Vor Befehlen (Eingabe, Tastenkürzel) erst den Text übernehmen.
    if ((e.key === 'Enter' && !multiline) || e.altKey || e.ctrlKey || e.metaKey) send();
    onKey?.(e);
  }

  onMount(() => {
    if (autofocus) {
      el?.focus();
      el?.select();
    }
    // Beim Ausblenden während des Tippens geht nichts verloren.
    return () => {
      if (timer !== undefined) send();
    };
  });
</script>

{#if multiline}
  <textarea
    bind:this={el}
    bind:value={draft}
    rows={Math.max(1, draft.split('\n').length)}
    aria-label={label}
    {placeholder}
    spellcheck="false"
    onfocus={() => (focused = true)}
    onblur={() => {
      focused = false;
      send();
      onBlur?.();
    }}
    oninput={input}
    onkeydown={keydown}
    {...rest}
  ></textarea>
{:else}
  <input
    bind:this={el}
    bind:value={draft}
    aria-label={label}
    {placeholder}
    spellcheck="false"
    onfocus={() => (focused = true)}
    onblur={() => {
      focused = false;
      send();
      onBlur?.();
    }}
    oninput={input}
    onkeydown={keydown}
    {...rest}
  />
{/if}

<style>
  input,
  textarea {
    display: block;
    width: 100%;
    min-width: 0;
    margin: 0;
    font-family: var(--et-font, inherit);
    font-size: var(--et-size, inherit);
    font-weight: var(--et-weight, inherit);
    line-height: var(--et-line, 1.45);
    text-align: var(--et-align, left);
    color: inherit;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 5px;
    padding: var(--et-pad, 3px 5px);
    resize: none;
    overflow: hidden;
    field-sizing: content;
  }
  input {
    field-sizing: var(--et-sizing, fixed);
  }
  input:hover,
  textarea:hover {
    background: var(--field-hover);
  }
  input:focus,
  textarea:focus {
    outline: none;
    background: var(--panel);
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  ::placeholder {
    color: var(--muted);
    opacity: 0.7;
    font-weight: 400;
  }
</style>
