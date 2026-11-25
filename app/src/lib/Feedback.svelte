<script lang="ts">
  // Fehler melden oder Idee einreichen: landet als Issue im Repository.
  import { tick } from 'svelte';
  import { isTauri } from './api';
  import {
    REPO,
    TOKEN_HELP_URL,
    clearToken,
    hasToken,
    issueUrl,
    loadDraft,
    openInBrowser,
    saveDraft,
    setToken,
    submit,
    technicalContext,
    type Draft,
  } from './feedback';
  import { notify } from './toast.svelte';

  type Props = { open: boolean; version: string | null; views: string[] };
  let { open = $bindable(), version, views }: Props = $props();

  let draft = $state<Draft>(loadDraft());
  let direct = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let tokenOpen = $state(false);
  let tokenValue = $state('');
  let titleEl = $state<HTMLInputElement>();

  let context = $derived(technicalContext(version, views));
  let canSend = $derived(draft.title.trim().length > 0 && !busy);

  $effect(() => {
    if (!open) return;
    error = null;
    tokenOpen = false;
    tokenValue = '';
    hasToken()
      .then((v) => (direct = v))
      .catch(() => (direct = false));
    tick().then(() => titleEl?.focus());
  });

  // Entwurf behalten, falls das Fenster geschlossen wird oder das Senden scheitert.
  $effect(() => {
    saveDraft($state.snapshot(draft));
  });

  function done(message: string, url: string) {
    draft = { kind: draft.kind, title: '', body: '', withContext: draft.withContext };
    open = false;
    notify('info', message, { label: 'Öffnen', run: () => openInBrowser(url) });
  }

  async function send() {
    if (!canSend) return;
    error = null;
    if (!direct) {
      // Ohne Zugriffstoken: ausgefülltes Formular im Browser, dort absenden.
      await openInBrowser(issueUrl(draft, context)).catch((e) => (error = String(e)));
      if (!error) open = false;
      return;
    }
    busy = true;
    try {
      const created = await submit($state.snapshot(draft), context);
      done(`Issue #${created.number} angelegt.`, created.url);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function saveToken() {
    error = null;
    busy = true;
    try {
      await setToken(tokenValue);
      tokenValue = '';
      tokenOpen = false;
      direct = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function removeToken() {
    await clearToken().catch(() => undefined);
    direct = false;
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      open = false;
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      send();
    }
    // Tastenkürzel der Anwendung gelten hier nicht.
    e.stopPropagation();
  }
</script>

{#if open}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (open = false)}>
    <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="fb-title" tabindex="-1" onkeydown={key}>
      <header>
        <h2 id="fb-title">Feedback</h2>
        <div class="kinds" role="radiogroup" aria-label="Art">
          <button role="radio" aria-checked={draft.kind === 'bug'} class:on={draft.kind === 'bug'} onclick={() => (draft.kind = 'bug')}
            >Fehler</button
          >
          <button role="radio" aria-checked={draft.kind === 'idea'} class:on={draft.kind === 'idea'} onclick={() => (draft.kind = 'idea')}
            >Idee</button
          >
        </div>
      </header>

      <input
        bind:this={titleEl}
        bind:value={draft.title}
        maxlength="200"
        aria-label="Titel"
        placeholder={draft.kind === 'bug' ? 'Was funktioniert nicht?' : 'Was wünschst du dir?'}
      />
      <textarea
        bind:value={draft.body}
        rows="8"
        aria-label="Beschreibung"
        placeholder={draft.kind === 'bug'
          ? 'Was hast du getan, was ist passiert, was hättest du erwartet?'
          : 'Wobei würde es helfen? Wie stellst du es dir vor?'}
      ></textarea>

      <label class="context">
        <input type="checkbox" bind:checked={draft.withContext} />
        <span>Technische Angaben anhängen: <code>{context}</code></span>
      </label>

      {#if error}<p class="error" role="alert">{error}</p>{/if}

      {#if tokenOpen}
        <div class="token">
          <p>
            Mit einem Zugriffstoken legt Formavium das Issue direkt an. Erstelle auf GitHub ein „Fine-grained
            token“ nur für das Repository <strong>{REPO}</strong> mit der Berechtigung
            <strong>Issues: Read and write</strong>. Es wird in der Windows-Anmeldeinformationsverwaltung
            gespeichert.
          </p>
          <div class="token-row">
            <input
              type="password"
              bind:value={tokenValue}
              aria-label="Zugriffstoken"
              placeholder="github_pat_…"
              autocomplete="off"
            />
            <button class="ghost small" onclick={() => openInBrowser(TOKEN_HELP_URL)}>Token erstellen …</button>
            <button class="primary small" disabled={!tokenValue.trim() || busy} onclick={saveToken}>Speichern</button>
          </div>
        </div>
      {/if}

      <footer>
        <p class="where">
          {#if direct}
            Wird als öffentliches Issue in {REPO} angelegt.
            <button class="link" onclick={removeToken}>Token entfernen</button>
          {:else}
            Öffnet das ausgefüllte, öffentliche Issue-Formular im Browser.
            {#if isTauri && !tokenOpen}
              <button class="link" onclick={() => (tokenOpen = true)}>Direkt senden einrichten</button>
            {/if}
          {/if}
        </p>
        <button class="ghost" onclick={() => (open = false)}>Abbrechen</button>
        <button class="primary" disabled={!canSend} onclick={send} title="Strg+Eingabe">
          {busy ? 'Sendet …' : direct ? 'Senden' : 'Im Browser öffnen'}
        </button>
      </footer>
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
    padding: 16px;
  }
  .sheet {
    display: grid;
    gap: 10px;
    width: min(620px, 100%);
    max-height: 100%;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 18px 20px 16px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    outline: none;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  .kinds {
    display: flex;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 2px;
  }
  .kinds button {
    font: inherit;
    font-size: 13px;
    padding: 4px 14px;
    border-radius: 6px;
    border: 1px solid transparent;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .kinds button.on {
    background: var(--panel);
    color: var(--text);
    border-color: var(--line);
  }
  input:not([type='checkbox']),
  textarea {
    width: 100%;
    font: inherit;
    color: var(--text);
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    padding: 8px 10px;
    resize: vertical;
  }
  input:focus,
  textarea:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .context {
    display: flex;
    gap: 8px;
    align-items: baseline;
    font-size: 12px;
    color: var(--text-2);
  }
  code {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted);
    word-break: break-word;
  }
  .error {
    margin: 0;
    font-size: 13px;
    color: var(--danger);
    background: var(--danger-soft);
    border-radius: 6px;
    padding: 6px 10px;
  }
  .token {
    background: var(--panel-2);
    border-radius: 8px;
    padding: 10px 12px;
  }
  .token p {
    margin: 0 0 8px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .token-row {
    display: flex;
    gap: 8px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
  }
  .where {
    flex: 1;
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
  .link {
    font: inherit;
    color: var(--accent-strong);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-decoration: underline;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
