import hljs from "highlight.js/lib/common";

export interface HighlightResult {
  /** 高亮后的安全 HTML（源码已转义 + token span）；null 表示未高亮，调用方应回退纯文本 */
  html: string | null;
  /** 检测到的语言；null 表示未识别或回退 */
  language: string | null;
}

const MAX_LENGTH = 20000;

export function highlightCode(content: string): HighlightResult {
  if (!content || content.length > MAX_LENGTH) {
    return { html: null, language: null };
  }
  try {
    const result = hljs.highlightAuto(content);
    return { html: result.value, language: result.language ?? null };
  } catch {
    return { html: null, language: null };
  }
}
