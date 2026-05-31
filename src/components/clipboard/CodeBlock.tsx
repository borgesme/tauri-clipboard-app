import { useMemo } from "react";

import { highlightCode } from "@/lib/highlight";
import { cn } from "@/lib/utils";

interface CodeBlockProps {
  content: string;
  className?: string;
}

export function CodeBlock({ content, className }: CodeBlockProps) {
  const { html, language } = useMemo(() => highlightCode(content), [content]);
  return (
    <pre className={cn("code-block", className)} data-language={language ?? undefined}>
      {html !== null ? (
        <code className="hljs" dangerouslySetInnerHTML={{ __html: html }} />
      ) : (
        <code className="hljs">{content}</code>
      )}
    </pre>
  );
}
