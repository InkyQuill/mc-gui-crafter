<script lang="ts">
  let { onclose }: { onclose: () => void } = $props();

  let dialogEl = $state<HTMLDivElement | undefined>();
  const dialogId = $props.id();
  const focusableSelector = [
    "button:not(:disabled)",
    "input:not(:disabled)",
    "select:not(:disabled)",
    "textarea:not(:disabled)",
    "a[href]",
    '[tabindex]:not([tabindex="-1"])',
  ].join(",");

  const sections = [
    {
      title: "Tools",
      shortcuts: [
        { keys: ["V"], label: "Select tool" },
        { keys: ["H"], label: "Pan tool" },
        { keys: ["S"], label: "Slot tool" },
        { keys: ["T"], label: "Texture tool" },
        { keys: ["X"], label: "Text tool" },
      ],
    },
    {
      title: "Editing",
      shortcuts: [
        { keys: ["Delete"], label: "Delete selected element" },
        { keys: ["Backspace"], label: "Delete selected element" },
        { keys: ["Esc"], label: "Clear selection and return to Select" },
      ],
    },
    {
      title: "Timeline",
      shortcuts: [
        { keys: ["Space"], label: "Play or pause animation preview" },
      ],
    },
    {
      title: "Help",
      shortcuts: [
        { keys: ["?"], label: "Open shortcuts" },
      ],
    },
  ];

  $effect(() => {
    const previouslyFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const firstFocusable = getFocusableElements()[0];

    (firstFocusable ?? dialogEl)?.focus();

    return () => {
      if (previouslyFocused && document.contains(previouslyFocused)) {
        previouslyFocused.focus();
      }
    };
  });

  function getFocusableElements(): HTMLElement[] {
    if (!dialogEl) return [];

    return Array.from(dialogEl.querySelectorAll<HTMLElement>(focusableSelector))
      .filter((element) => element.offsetParent !== null || element === document.activeElement);
  }

  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      onclose();
    }
  }

  function trapFocus(event: KeyboardEvent) {
    const focusable = getFocusableElements();
    if (focusable.length === 0) {
      event.preventDefault();
      dialogEl?.focus();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];

    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    event.stopPropagation();

    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    } else if (event.key === "Tab") {
      trapFocus(event);
    }
  }
</script>

<div class="dialog-overlay" role="presentation" onclick={handleOverlayClick}>
  <div
    bind:this={dialogEl}
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="{dialogId}-title"
    tabindex="-1"
    onkeydown={handleKeydown}
  >
    <div class="dialog-header">
      <h2 id="{dialogId}-title">Keyboard Shortcuts</h2>
      <button class="close-btn" type="button" aria-label="Close shortcuts" title="Close" onclick={onclose}>×</button>
    </div>

    <div class="shortcut-sections">
      {#each sections as section}
        <section class="section" aria-labelledby="{dialogId}-{section.title.toLowerCase()}">
          <h3 id="{dialogId}-{section.title.toLowerCase()}">{section.title}</h3>

          <dl class="shortcut-list">
            {#each section.shortcuts as shortcut}
              <div class="shortcut-row">
                <dt>
                  {#each shortcut.keys as key}
                    <kbd>{key}</kbd>
                  {/each}
                </dt>
                <dd>{shortcut.label}</dd>
              </div>
            {/each}
          </dl>
        </section>
      {/each}
    </div>

    <div class="dialog-actions">
      <button class="done-btn" type="button" onclick={onclose}>Done</button>
    </div>
  </div>
</div>

<style>
  .dialog-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.62);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 16px;
  }

  .dialog {
    width: min(420px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow: auto;
    background: #1a1a2e;
    border: 1px solid #0f3460;
    border-radius: 8px;
    padding: 16px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    outline: none;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  h2 {
    color: #e0e0e0;
    font-size: 15px;
    margin: 0;
  }

  h3 {
    color: #8080a0;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0;
    margin: 0 0 8px;
    text-transform: uppercase;
  }

  .shortcut-sections {
    border-top: 1px solid #0f3460;
  }

  .section {
    border-bottom: 1px solid #0f3460;
    padding: 12px 0;
  }

  .shortcut-list {
    display: grid;
    gap: 6px;
    margin: 0;
  }

  .shortcut-row {
    display: grid;
    grid-template-columns: 112px minmax(0, 1fr);
    align-items: center;
    gap: 12px;
    min-height: 26px;
  }

  dt,
  dd {
    margin: 0;
  }

  dt {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  dd {
    color: #d8d8e0;
    font-size: 12px;
  }

  kbd {
    min-width: 24px;
    padding: 3px 6px;
    border: 1px solid #304a7a;
    border-bottom-color: #1f3152;
    border-radius: 4px;
    background: #12121f;
    color: #f0f0f5;
    font-family: monospace;
    font-size: 11px;
    line-height: 1.2;
    text-align: center;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 12px;
  }

  button {
    border-radius: 4px;
    cursor: pointer;
    font-family: inherit;
    font-size: 12px;
  }

  .close-btn {
    width: 28px;
    height: 28px;
    background: transparent;
    border: 1px solid transparent;
    color: #a0a0b0;
    font-size: 18px;
    line-height: 1;
  }

  .close-btn:hover {
    background: #0f3460;
    color: #e0e0e0;
  }

  .done-btn {
    background: #e94560;
    border: 1px solid #e94560;
    color: #12121f;
    font-weight: 700;
    padding: 6px 12px;
  }

  .done-btn:hover {
    background: #ff5a7a;
  }

  button:focus-visible {
    outline: 2px solid #e94560;
    outline-offset: 2px;
  }

  :global(:root[data-theme="high_contrast"] .dialog) {
    border-color: #ffffff;
  }
</style>
