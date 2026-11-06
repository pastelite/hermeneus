/// Default translation prompt. `{{glossary}}` and `{{source}}` are replaced by
/// [`buildPrompt`].
export const DEFAULT_PROMPT = `You are translating English into natural Thai for Thai Wikipedia. Formal encyclopedic register, neutral tone.

## Inputs

- <source>: the English paragraph. This is the source of truth for facts.
- <glossary>: required Thai terms. This is the source of truth for terminology.

## Rules
CORE PRINCIPLE: Thai packs information into chains, not sentences. Don't translate sentence by sentence. Find the thread (one subject, one time anchor), merge related English sentences into one chain, and break only when the subject, topic or time anchor really changes. If your Thai back-translates into a clean series of short English sentences, it reads as translated. A long run-on is usually the right sign.

Thai is not English. Thai does not use commas and full stops to organize sentences in the same way English does. Spaces often separate clauses and phrases, making conjunctions and connective expressions essential for maintaining the flow of information.

**Never translate English sentences as isolated statements. Every clause and piece of information must be naturally connected to the preceding context.**

- Always identify the relationship between consecutive sentences and clauses, then express it naturally using appropriate Thai connectors.
- Preserve relationships within sentences, especially relative clauses, subordinate clauses, and embedded information.
- Freely combine, split, and restructure English sentences. Do not mechanically preserve English punctuation or sentence boundaries.
- Maintain continuity of subjects and topics. Do not make each sentence sound like an entirely new statement.
- Write as a native Thai writer would, ensuring the entire paragraph flows naturally.

JOINTS: every link in the chain must say what kind of link it is. Never default to และ.
- Addition: และ, อีกทั้ง, นอกจากนี้, รวมถึง
- Explanation: ซึ่ง, โดย, กล่าวคือ, ทั้งนี้
- Causality: เพราะ, เนื่องจาก, ดังนั้น, ทำให้, ส่งผลให้
- Sequence: จากนั้น, ต่อมา, หลังจากนั้น, ก่อนที่
- Contrast: แต่, อย่างไรก็ตาม, ในขณะที่, แม้ว่า
- Elaboration: เช่น, โดยเฉพาะ, ซึ่ง, นั่นคือ
- Concession: แม้ว่า, ถึงแม้ว่า, ทั้งที่

PACKING:
- Name the subject once, then drop it. Avoid เขา/มัน/พวกเขา and possessives like ของเขา.
- Set the time anchor once; later events inherit it. Don't restate "In 2011" in every sentence.
- Fold small facts into modifiers instead of stating them separately.
- Restructure English relative clauses; don't stack ที่ or ซึ่ง.
- "The" is usually omitted. Add นั้น sparingly, only when the reference really needs pointing out.
- Avoid ก็ as a casual filler; use it only where formal writing needs it.

GRAMMAR AND STYLE:
- Use เป็น, คือ, ได้แก่. Avoid slang.
- Passive: avoid ถูก unless adversity is meant. Use ได้รับ, ...ขึ้น, โดย, or rewrite actively.
- No plural marking; classifiers only when counting.
- Don't use ได้, แล้ว, จะ, กำลัง by reflex. Thai has no tense.
- Follow Royal Institute (ราชบัณฑิตยสภา) conventions for transliteration and technical terms. Use established Thai terms if they exist.
- Use ค.ศ. for years (พ.ศ. only if the source context is Thai and calls for it). Stay consistent.
- Use correct ราชาศัพท์ and titles. Give the original name in parentheses on first mention.
- No evaluative or promotional wording.

Before output, check: no repeated pronouns, no sentence-by-sentence mapping, every joint is specific, no stray full stops.

## Preserve exactly

Copy the following character for character, including backticks:
- templates such as \`[template:...]\`
- references such as [^source]
- link targets (translate only the visible display text, never the target)

Keep bold, italics, and list markers as in the source.

## Link explanations
- Understand link syntax: Markdown links follow [display text](target).
  - Display text (source text): The visible text readers click on.
  - Target: The destination the link leads to (usually a page name or URL).
  - Example: [Mercury](Mercury (element)) — "Mercury" is the display text, and "Mercury (element)" is the target.
- Translate and transliterate: Match links using their targets, not display text. Use existing Thai page links when available. Otherwise, translate both display text and targets into Thai, including appropriate transliteration of foreign names and terms. Keep scientific terms in English unless a Thai page exists.
- Preserve and verify links: Never remove links. Keep filenames, category names, and external URLs unchanged. Preserve Markdown syntax, use angle brackets around targets containing spaces or parentheses, and check that no untranslated English targets remain (except permitted scientific terms).

## Output

Output 3 things:
- <first_draft> wrapped in a markdown code block
- <issues_found> see the first draft, notice something wrong? something unnatural? anything untranslated (except in exception)? can we rewrite sentence? which one is unclear? did you use hard literary word? did you forgot the connector? go paragraph by paragraph
- <final_draft> fix the issues that you found and output the final draft, wrapped in a markdown code block

<glossary>
{{glossary}}
</glossary>

<source>
{{source}}
</source>
`;

/// Inserts `glossary` and `source` into a prompt template.
export function buildPrompt(template: string, glossary: string, source: string): string {
  return template.split('{{glossary}}').join(glossary).split('{{source}}').join(source);
}
