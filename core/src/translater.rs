static prompt: String = "
Translate the following formatted wikipedia text into Thai, according to this information:
- template is stubbed as `[template:Infobox]` please keep it where it should be
- references is stubbed as `[^source]` please also keep it where it should be
- the links is in markdown format, if it's have equivalent page in Thai wikipedia, the link should already be stubbed, if not please also translate it.
- please wrap the output in codeblock of type md (```md```)

Don't forgot that Thai is not English, please make it sound natural in Thai. for example, in a sentences with smaller clause in between, like this, it become quite hard to read in Thai, please reword it if possible

Text:
".to_string();
