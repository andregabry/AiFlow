import { describe, expect, it } from "vitest";
import { githubUserCode } from "../src/features/accounts/githubSignIn";

describe("official GitHub CLI device user codes", () => {
  it.each([
    "First copy your one-time code: TEST-1234",
    "! One-time code (TEST-1234) copied to clipboard",
    "\u001b[31m! One-time code (TEST-1234) copied to clipboard\u001b[0m",
    "one-time code: test-1234\nOpen https://github.com/login/device",
  ])("recognizes %s", (output) => {
    expect(githubUserCode(output)).toBe("TEST-1234");
  });
  it("waits for a complete code and ignores unrelated identifiers", () => {
    expect(
      githubUserCode("Open https://github.com/login/device"),
    ).toBeUndefined();
    expect(githubUserCode("one-time code: TEST-")).toBeUndefined();
    expect(githubUserCode("Request ID: TEST-1234")).toBeUndefined();
  });
});
