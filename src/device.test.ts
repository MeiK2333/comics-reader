import { describe, expect, it } from "vitest";
import { isMobileWebView } from "./device";

const DESKTOP =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const MAC =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
const ANDROID =
  "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
const IPHONE =
  "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
const IPAD =
  "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

describe("isMobileWebView", () => {
  it("detects the mobile WebViews the app ships with", () => {
    for (const userAgent of [ANDROID, IPHONE, IPAD]) {
      expect(isMobileWebView(userAgent)).toBe(true);
    }
  });

  it("keeps desktop builds desktop even in a narrow window", () => {
    expect(isMobileWebView(DESKTOP)).toBe(false);
    expect(isMobileWebView(MAC)).toBe(false);
  });
});
