pub const MARKER_START: &str = "<<<TRANSLATION>>>";
pub const MARKER_END: &str = "<<<END_TRANSLATION>>>";

pub fn build_prompt(input: &str, target_lang: &str) -> String {
    let base = format!(
        "You are a professional {to} native translator who needs to fluently translate text into {to}.\n\n## Translation Rules\n1. Output only the translated content, wrapped by the required markers and nothing else\n2. The returned translation must maintain exactly the same number of paragraphs and format as the original text\n3. If the text contains HTML tags, consider where the tags should be placed in the translation while maintaining fluency\n4. For content that should not be translated (such as proper nouns, code, etc.), keep the original text.\n5. If input contains %%, use %% in your output, if input has no %%, don't use %% in your output\n\n## OUTPUT FORMAT:\n- **Single paragraph input** -> Output translation directly (no separators, no extra text)\n- **Multi-paragraph input** -> Use %% as paragraph separator between translations\n\n## Marking Requirement\nWrap the final translation between {start} and {end} on a single output. Output nothing outside the markers.\n\n## Examples\n### Multi-paragraph Input:\nParagraph A\n%%\nParagraph B\n%%\nParagraph C\n%%\nParagraph D\n\n### Multi-paragraph Output:\nTranslation A\n%%\nTranslation B\n%%\nTranslation C\n%%\nTranslation D\n\n### Single paragraph Input:\nSingle paragraph content\n\n### Single paragraph Output:\nDirect translation without separators\n",
        to = target_lang,
        start = MARKER_START,
        end = MARKER_END,
    );

    format!("{base}\n\n### Input\n{input}")
}

pub fn extract_translation(content: &str) -> Option<String> {
    let start = content.find(MARKER_START)? + MARKER_START.len();
    let end = content[start..].find(MARKER_END)? + start;
    let extracted = content[start..end].trim();
    if extracted.is_empty() {
        None
    } else {
        Some(extracted.to_string())
    }
}
