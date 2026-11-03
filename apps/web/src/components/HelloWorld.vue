<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { fetch_stubbed_page, get_full_prompt, restub_page } from '../../pkg/wiki_translate_client.js';

type LinkTableRow = {
  title: string;
  link_to: string;
  translated: string | null;
};

let title = ref('');
let originalMarkdownText = ref('');
let markdownText = ref('');
let restubbedText = ref('');
let copyStatus = ref('');
let linkTableRows = ref<LinkTableRow[]>([]);

let placeholder = computed(()=>{
  return "Place your translated markdown here:\n" + originalMarkdownText.value
})

onMounted(() => {
  title.value = localStorage.getItem('title') || '';
  const savedData = localStorage.getItem('rewrappedData');
  if (savedData) {
    try {
      linkTableRows.value = JSON.parse(savedData).links || [];
    } catch {
      linkTableRows.value = [];
    }
  }
})

watch(title, () => {
  localStorage.setItem('title', title.value);
})

function translate_page() {
  fetch_stubbed_page('en', title.value).then((result) => {
    let parsed = JSON.parse(result);
    originalMarkdownText.value = parsed.stubbed;
    markdownText.value = '';
    copyStatus.value = '';
    linkTableRows.value = parsed.links || [];

    let rewrappedData = {
      template: parsed.template,
      references: parsed.references,
      links: linkTableRows.value,
    };
    localStorage.setItem('rewrappedData', JSON.stringify(rewrappedData));
    
    // markdownText.value = result;
    // console.log(result);
  });
}

async function copyOriginalMarkdownWithPrompt() {
  if (!originalMarkdownText.value) return;

  try {
    const clipboard = (navigator as Navigator & {
      clipboard: { writeText(text: string): Promise<void> };
    }).clipboard;
    await clipboard.writeText(
      get_full_prompt(originalMarkdownText.value, JSON.stringify(linkTableRows.value)),
    );
    copyStatus.value = 'Original markdown and prompt copied.';
  } catch {
    copyStatus.value = 'Could not copy to clipboard. Check browser clipboard permissions.';
  }
}

function rewrap() {
  let rewrappedData = localStorage.getItem('rewrappedData');
  if (rewrappedData) {
    let parsed = JSON.parse(rewrappedData);
    let stubbedPage = {
      stubbed: markdownText.value,
      ...parsed
    }

    // console.log(stubbedPage);
    let restubbed = restub_page(JSON.stringify(stubbedPage));
    restubbedText.value = restubbed;
  }
}
</script>

<template>
  <h1>Wikipedia Translator</h1>
  <input v-model="title" placeholder="Enter English Wikipedia page title">
  <button @click="translate_page">Translate</button>
  <h2>Markdown Output</h2>
  <p>Paste the translated markdown here. This box is not used by the copy button.</p>
  <button :disabled="!originalMarkdownText" @click="copyOriginalMarkdownWithPrompt">
    Copy original markdown + prompt
  </button>
  <p aria-live="polite">{{ copyStatus }}</p>
  <textarea
    v-model="markdownText"
    class="markdown-input"
    rows="20"
    :placeholder="placeholder"
  ></textarea>
  <button @click="rewrap">rewrap</button>
  <h2>Restubbed Output</h2>
  <textarea v-model="restubbedText"></textarea>
</template>

<style scoped>
.markdown-input {
  box-sizing: border-box;
  display: block;
  width: min(100%, 900px);
  min-height: 320px;
  margin: 0 auto 1rem;
}
</style>
