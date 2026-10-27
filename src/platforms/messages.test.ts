import { describe, expect, it } from "vitest";
import { platformList } from "./index";

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{(\w+)\}/g)]
    .map((match) => match[1])
    .sort();
}

describe("platform messages", () => {
  for (const platform of platformList) {
    it(`${platform.id} mirrors its keys and placeholders`, () => {
      const en = platform.messages.en;
      const zh = platform.messages["zh-CN"];

      expect(Object.keys(zh).sort()).toEqual(Object.keys(en).sort());
      for (const key of Object.keys(en)) {
        expect(placeholders(zh[key])).toEqual(placeholders(en[key]));
      }
      expect(en.label.trim()).not.toBe("");
      expect(zh.label.trim()).not.toBe("");
    });
  }
});
