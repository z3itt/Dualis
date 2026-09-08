import { describe, expect, it } from "vitest";
import { formatEta, formatTime } from "./utils";

describe("formatTime", () => {
  it("pads seconds", () => {
    expect(formatTime(75)).toBe("1:15");
  });
});

describe("formatEta", () => {
  it("hides zero and formats minutes", () => {
    expect(formatEta(0)).toBe("");
    expect(formatEta(12)).toBe("12s");
    expect(formatEta(75)).toBe("1m 15s");
  });
});
