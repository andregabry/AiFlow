import { terminalText } from "../../core/types";

export function githubUserCode(output: string): string | undefined {
  return terminalText(output)
    .match(/\bone-time code\s*[:(]?\s*([A-Z0-9]{4}-[A-Z0-9]{4})\b/i)?.[1]
    ?.toUpperCase();
}
