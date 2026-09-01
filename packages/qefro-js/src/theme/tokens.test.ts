import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  BRAND_INPUT_TOKENS,
  BUTTON_VARIANTS,
  MD_COLOR_ROLES,
  MD_ELEVATION_TOKENS,
  MD_MOTION_TOKENS,
  MD_SHAPE_TOKENS,
  MD_TYPE_TOKENS,
  QEFRO_COLOR_ROLES,
  QEFRO_ELEVATION_TOKENS,
  QEFRO_MOTION_TOKENS,
  QEFRO_SHAPE_TOKENS,
  QEFRO_TYPE_TOKENS,
  buttonClass,
} from "./tokens";

const css = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), "../styles.css"), "utf8");

describe("M3 token layer", () => {
  it("defines color roles, shape, elevation, type, and motion in CSS", () => {
    for (const token of [
      ...QEFRO_COLOR_ROLES,
      ...QEFRO_SHAPE_TOKENS,
      ...QEFRO_ELEVATION_TOKENS,
      ...QEFRO_TYPE_TOKENS,
      ...QEFRO_MOTION_TOKENS,
      ...MD_COLOR_ROLES,
      ...MD_SHAPE_TOKENS,
      ...MD_ELEVATION_TOKENS,
      ...MD_TYPE_TOKENS,
      ...MD_MOTION_TOKENS,
      ...BRAND_INPUT_TOKENS,
    ]) {
      expect(css).toContain(`${token}:`);
    }
  });

  it("maps tenant branding onto M3 primary", () => {
    expect(css).toContain("--md-primary: var(--accent)");
    expect(css).toMatch(/html\[data-theme="dark"\]/);
  });

  it("keeps shape tokens modest for a dense business UI", () => {
    expect(css).toMatch(/--md-shape-sm:\s*6px/);
    expect(css).toMatch(/--md-shape-md:\s*8px/);
    expect(css).toMatch(/--md-shape-lg:\s*12px/);
  });
});

describe("button variants", () => {
  it("maps filled/tonal/outlined/text/icon/destructive to CSS classes", () => {
    expect(BUTTON_VARIANTS).toEqual(["filled", "tonal", "outlined", "text", "icon", "destructive"]);
    expect(buttonClass("filled")).toBe("btn");
    expect(buttonClass("tonal")).toBe("btn tonal");
    expect(buttonClass("outlined")).toBe("btn ghost");
    expect(buttonClass("text")).toBe("btn text");
    expect(buttonClass("icon")).toBe("btn ghost icon-btn");
    expect(buttonClass("destructive")).toBe("btn danger");
    expect(css).toContain("button.tonal");
    expect(css).toContain("button.text");
    expect(css).toContain("button.danger");
    expect(css).toContain("button.ghost");
    expect(css).toContain(".icon-btn");
  });
});
