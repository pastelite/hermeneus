<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  fetch_stubbed_page,
  get_glossary,
  restub_page,
} from "../../pkg/wiki_translate_client.js";
import { buildPrompt, DEFAULT_PROMPT } from "../prompt.ts";

type LinkTableRow = {
  title: string;
  link_to: string;
  translated: string | null;
};

let title = ref("");
let originalMarkdownText = ref("");
let markdownText = ref("");
let restubbedText = ref("");
let copyStatus = ref("");
let linkTableRows = ref<LinkTableRow[]>([]);
let promptTemplate = ref(DEFAULT_PROMPT);

let placeholder = computed(() => {
  return "paste translated markdown here";
});

// Glossary for the prompt, derived from linkTableRows.
// Only rows with a non-empty translation are included.
const glossaryText = computed(() => {
  const filtered = linkTableRows.value.filter(
    (row) => row.translated != null && row.translated.trim() !== ""
  );
  return get_glossary(JSON.stringify(filtered));
});

const markdownPreviewRef = ref<HTMLTextAreaElement | null>(null);
const glossaryTableRef = ref<HTMLDivElement | null>(null);
let resizeObserver: ResizeObserver | null = null;

function syncGlossaryHeight() {
  const text = markdownPreviewRef.value;
  const table = glossaryTableRef.value;
  if (!text || !table) return;
  table.style.height = `${text.offsetHeight}px`;
}

onMounted(() => {
  title.value = localStorage.getItem("title") || "";
  promptTemplate.value =
    localStorage.getItem("promptTemplate") || DEFAULT_PROMPT;
  const savedData = localStorage.getItem("rewrappedData");
  if (savedData) {
    try {
      const saved = JSON.parse(savedData);
      linkTableRows.value = saved.links || [];
    } catch {
      linkTableRows.value = [];
    }
  }

  if (markdownPreviewRef.value && "ResizeObserver" in window) {
    resizeObserver = new ResizeObserver(syncGlossaryHeight);
    resizeObserver.observe(markdownPreviewRef.value);
    requestAnimationFrame(syncGlossaryHeight);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
});

watch(title, () => {
  localStorage.setItem("title", title.value);
});

watch(promptTemplate, () => {
  localStorage.setItem("promptTemplate", promptTemplate.value);
});

function translate_page() {
  fetch_stubbed_page("en", title.value).then((result) => {
    let parsed = JSON.parse(result);
    originalMarkdownText.value = parsed.stubbed;
    markdownText.value = "";
    copyStatus.value = "";
    linkTableRows.value = parsed.links || [];

    let rewrappedData = {
      template: parsed.template,
      references: parsed.references,
      links: linkTableRows.value,
    };
    localStorage.setItem("rewrappedData", JSON.stringify(rewrappedData));

    // markdownText.value = result;
    // console.log(result);
  });
}

async function copyRestubbedPage() {
  try {
    const clipboard = (
      navigator as Navigator & {
        clipboard: { writeText(text: string): Promise<void> };
      }
    ).clipboard;
    await clipboard.writeText(restubbedText.value);
    copyStatus.value = "Restubbed page copied.";
  } catch {
    copyStatus.value =
      "Could not copy to clipboard. Check browser clipboard permissions.";
  }
}

async function copyPromptWithPage() {
  if (!originalMarkdownText.value) return;

  try {
    const clipboard = (
      navigator as Navigator & {
        clipboard: { writeText(text: string): Promise<void> };
      }
    ).clipboard;
    await clipboard.writeText(
      buildPrompt(
        promptTemplate.value,
        glossaryText.value,
        originalMarkdownText.value
      )
    );
    copyStatus.value = "Prompt copied with the glossary and stubbed page.";
  } catch {
    copyStatus.value =
      "Could not copy to clipboard. Check browser clipboard permissions.";
  }
}

function resetPrompt() {
  promptTemplate.value = DEFAULT_PROMPT;
  copyStatus.value = "Prompt reset to the default.";
}

function rewrap() {
  let rewrappedData = localStorage.getItem("rewrappedData");
  if (rewrappedData) {
    let parsed = JSON.parse(rewrappedData);
    let stubbedPage = {
      stubbed: markdownText.value,
      ...parsed,
    };

    // console.log(stubbedPage);
    let restubbed = restub_page(JSON.stringify(stubbedPage));
    restubbedText.value = restubbed;
  }
}

async function restubAndCopy() {
  rewrap();
  await copyRestubbedPage();
}
</script>

<template>
  <h1>Wikipedia Translator</h1>
  <input v-model="title" placeholder="Enter English Wikipedia page title" />
  <button @click="translate_page">Translate</button>
  <h2>Markdown Output</h2>
  <div class="page-preview">
    <!-- <div class="markdown-preview">markdown preview</div> -->
    <textarea
      ref="markdownPreviewRef"
      disabled
      class="markdown-preview"
      :value="originalMarkdownText"
    />
    <div ref="glossaryTableRef" class="glossary-table" role="table">
      <div style="grid-column: span 3; font-size: small; text-align: left">
        Note: Empty here will not provided in the prompt glossary
      </div>
      <div class="glossary-cell glossary-header" role="columnheader">
        Text In Page
      </div>
      <div class="glossary-cell glossary-header" role="columnheader">
        Link To
      </div>
      <div class="glossary-cell glossary-header" role="columnheader">
        Translation
      </div>
      <template v-for="(link, index) in linkTableRows" :key="index">
        <div class="glossary-cell" role="cell">{{ link.title }}</div>
        <div class="glossary-cell" role="cell">{{ link.link_to }}</div>
        <div class="glossary-cell" role="cell">
          <input v-model="link.translated" />
        </div>
      </template>
    </div>
  </div>
  <button :disabled="!originalMarkdownText" @click="copyPromptWithPage">
    Copy glossary and page
  </button>
  <p aria-live="polite">{{ copyStatus }}</p>
  <textarea
    v-model="markdownText"
    class="markdown-input"
    rows="20"
    :placeholder="placeholder"
  ></textarea>
  <button @click="rewrap">Restub</button>
  <button @click="restubAndCopy">Restub and copy page</button>
  <h2>Restubbed Output</h2>
  <textarea v-model="restubbedText" class="restub-input"></textarea>
  <button @click="copyRestubbedPage">Copy above text</button>
  <section class="prompt-editor">
    <h2>Prompt Template</h2>
    <p>
      Edit the prompt used by the copy button.
      <code>&#123;&#123;glossary&#125;&#125;</code> is replaced with the
      glossary and <code>&#123;&#123;source&#125;&#125;</code> with the stubbed
      page.
    </p>
    <textarea
      v-model="promptTemplate"
      class="prompt-input"
      rows="12"
      spellcheck="false"
    ></textarea>
    <button @click="resetPrompt">Reset to default prompt</button>
    <button :disabled="!originalMarkdownText" @click="copyPromptWithPage">
      Copy prompt + page
    </button>
    <p aria-live="polite">{{ copyStatus }}</p>
  </section>
</template>

<style scoped>
.page-preview {
  display: flex;
  flex-direction: row;
  justify-content: space-between;
  align-items: flex-start;
}

.markdown-preview {
  flex: 2;
  margin-left: 1rem;
  overflow: auto;
  min-height: 300px;
  resize: both;
}

.glossary-table {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  align-content: start;
  border: 1px solid #fff2;
  flex: 1;
  min-height: 0;
  align-self: flex-start;
  overflow-y: auto;
  box-sizing: border-box;

  .glossary-cell {
    font-size: medium;
    text-align: left;
    padding: 0 0.5rem;
    min-width: 0;
    overflow-wrap: anywhere;
    border-bottom: 1px solid #fff2;

    input {
      box-sizing: border-box;
      width: 100%;
      min-width: 0;
    }
  }

  .glossary-header {
    font-weight: bold;
    background-color: #fff2;
  }
}

.markdown-input,
.prompt-input,
.restub-input {
  box-sizing: border-box;
  display: block;
  width: min(100%, 900px);
  margin: 0 auto 1rem;
}

.markdown-input,
.restub-input {
  min-height: 320px;
}

.prompt-input {
  min-height: 240px;
  font-family: monospace;
}
</style>
