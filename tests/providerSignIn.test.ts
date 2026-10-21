import { expect, it } from "vitest";
import {
  officialProviderUrl,
  providerUserCode,
} from "../src/features/accounts/providerSignIn";
it("only exposes an official login origin for the selected CLI", () => {
  expect(
    officialProviderUrl(
      "codex",
      "Open https://auth.openai.com/oauth/authorize?client_id=official",
    ),
  ).toContain("auth.openai.com");
  expect(
    officialProviderUrl(
      "gemini",
      "Open https://accounts.google.com/o/oauth2/auth",
    ),
  ).toContain("accounts.google.com");
  expect(
    officialProviderUrl("codex", "https://auth.openai.com.attacker.example/"),
  ).toBeUndefined();
  expect(
    officialProviderUrl("claude", "https://accounts.google.com/"),
  ).toBeUndefined();
  expect(
    officialProviderUrl("codex", "http://auth.openai.com/"),
  ).toBeUndefined();
});
it("shows a code only when the CLI actually supplies a short user code", () => {
  expect(providerUserCode("one-time code: ABCD-1234")).toBe("ABCD-1234");
  expect(
    providerUserCode("Open the browser to finish signing in"),
  ).toBeUndefined();
});
