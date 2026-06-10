import { describe, it, expect, beforeEach } from "vitest";
import { getChartColors, getChartTextColor, getChartBgColor } from "@/utils/chart-theme";

describe("chart-theme", () => {
  beforeEach(() => {
    // Set up CSS custom properties on the root element
    document.documentElement.style.setProperty("--chart-color-1", "#5470C6");
    document.documentElement.style.setProperty("--chart-color-2", "#91CC75");
    document.documentElement.style.setProperty("--chart-color-3", "#FAC858");
    document.documentElement.style.setProperty("--chart-color-4", "#EE6666");
    document.documentElement.style.setProperty("--chart-color-5", "#73C0DE");
    document.documentElement.style.setProperty("--text-secondary", "#666");
    document.documentElement.style.setProperty("--card-bg", "#fff");
  });

  it("getChartColors returns 5 color values", () => {
    const colors = getChartColors();
    expect(colors).toHaveLength(5);
    expect(colors[0]).toBe("#5470C6");
    expect(colors[4]).toBe("#73C0DE");
  });

  it("getChartTextColor returns text-secondary value", () => {
    expect(getChartTextColor()).toBe("#666");
  });

  it("getChartBgColor returns card-bg value", () => {
    expect(getChartBgColor()).toBe("#fff");
  });
});
