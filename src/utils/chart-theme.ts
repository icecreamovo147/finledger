export function getChartColors(): string[] {
  const style = getComputedStyle(document.documentElement);
  return [
    style.getPropertyValue("--chart-color-1").trim(),
    style.getPropertyValue("--chart-color-2").trim(),
    style.getPropertyValue("--chart-color-3").trim(),
    style.getPropertyValue("--chart-color-4").trim(),
    style.getPropertyValue("--chart-color-5").trim(),
  ];
}

export function getChartTextColor(): string {
  return getComputedStyle(document.documentElement)
    .getPropertyValue("--text-secondary")
    .trim();
}

export function getChartBgColor(): string {
  return getComputedStyle(document.documentElement)
    .getPropertyValue("--card-bg")
    .trim();
}
