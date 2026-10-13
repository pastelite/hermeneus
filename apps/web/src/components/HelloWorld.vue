<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { fetch_stubbed_page, restub_page } from '../../pkg/wiki_translate_client';

let title = ref('');
let markdownText = ref('');
let restubbedText = ref('');

onMounted(() => {
  title.value = localStorage.getItem('title') || '';
})

watch(title, () => {
  localStorage.setItem('title', title.value);
})

function translate_page() {
  fetch_stubbed_page('en', title.value).then((result) => {
    let parsed = JSON.parse(result);
    markdownText.value = parsed.stubbed;

    let rewrappedData = {
      title: parsed.title,
      lang_links: parsed.lang_links,
      template: parsed.template,
      references: parsed.references,
    };
    localStorage.setItem('rewrappedData', JSON.stringify(rewrappedData));
    
    // markdownText.value = result;
    // console.log(result);
  });
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
  <textarea v-model="markdownText"></textarea>
  <button @click="rewrap">rewrap</button>
  <h2>Restubbed Output</h2>
  <textarea v-model="restubbedText"></textarea>
</template>
