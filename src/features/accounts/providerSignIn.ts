import { terminalText } from "../../core/types";
export const loginOrigins: Record<string, string[]> = {
  github: ["https://github.com"],
  codex: ["https://auth.openai.com", "https://chatgpt.com"],
  claude: [
    "https://claude.ai",
    "https://console.anthropic.com",
    "https://platform.claude.com",
  ],
  gemini: ["https://accounts.google.com"],
};
export function officialProviderUrl(
  id: string,
  output: string,
): string | undefined {
  return terminalText(output)
    .match(/https:\/\/[^\s<>"']+/g)
    ?.find((raw) => {
      try {
        return loginOrigins[id]?.includes(new URL(raw).origin);
      } catch {
        return false;
      }
    });
}
export function providerUserCode(output: string): string | undefined {
  return terminalText(output)
    .match(
      /(?:one-time code|enter (?:this |the )?code)\s*[:(]?\s*([A-Z0-9]{4}(?:-[A-Z0-9]{4}){1,2})\b/i,
    )?.[1]
    ?.toUpperCase();
}
