<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
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
let glossaryText = ref("");
let promptTemplate = ref(DEFAULT_PROMPT);

let placeholder = computed(() => {
  return "Place your translated markdown here:\n" + originalMarkdownText.value;
});

onMounted(() => {
  title.value = localStorage.getItem("title") || "";
  promptTemplate.value =
    localStorage.getItem("promptTemplate") || DEFAULT_PROMPT;
  const savedData = localStorage.getItem("rewrappedData");
  if (savedData) {
    try {
      const saved = JSON.parse(savedData);
      linkTableRows.value = saved.links || [];
      glossaryText.value = saved.glossary || "";
    } catch {
      linkTableRows.value = [];
    }
  }
  // Older saved data may not have the formatted glossary yet.
  if (!glossaryText.value) {
    glossaryText.value = get_glossary(JSON.stringify(linkTableRows.value));
  }
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
    glossaryText.value = get_glossary(JSON.stringify(linkTableRows.value));

    let rewrappedData = {
      template: parsed.template,
      references: parsed.references,
      links: linkTableRows.value,
      glossary: glossaryText.value,
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
</script>

<template>
  <h1>Wikipedia Translator</h1>
  <input v-model="title" placeholder="Enter English Wikipedia page title" />
  <button @click="translate_page">Translate</button>
  <h2>Markdown Output</h2>
  <p>
    Paste the translated markdown here. This box is not used by the copy button.
  </p>
  <button :disabled="!originalMarkdownText" @click="copyPromptWithPage">
    Copy prompt + page
  </button>
  <p aria-live="polite">{{ copyStatus }}</p>
  <textarea
    v-model="markdownText"
    class="markdown-input"
    rows="20"
    :placeholder="placeholder"
  ></textarea>
  <button @click="rewrap">Restub</button>
  <button @click="copyRestubbedPage">Copy Restubbed Page</button>
  <h2>Restubbed Output</h2>
  <textarea v-model="restubbedText" class="restub-input"></textarea>
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
