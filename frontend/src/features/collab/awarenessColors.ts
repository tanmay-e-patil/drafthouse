export const PALETTE = [
  "#B45309",
  "#2F855A",
  "#2B6CB0",
  "#7C3AED",
  "#BE123C",
  "#0F766E",
  "#A16207",
  "#9D174D",
] as const;

/** Return first palette color not in `usedColors`. Wraps to first on exhaustion. */
export function assignColor(usedColors: string[]): string {
  const used = new Set(usedColors);
  return PALETTE.find((c) => !used.has(c)) ?? PALETTE[0];
}
