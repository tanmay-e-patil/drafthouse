export const PALETTE = [
  "#E53E3E",
  "#DD6B20",
  "#D69E2E",
  "#38A169",
  "#3182CE",
  "#805AD5",
  "#D53F8C",
  "#00B5D8",
] as const;

/** Return first palette color not in `usedColors`. Wraps to first on exhaustion. */
export function assignColor(usedColors: string[]): string {
  const used = new Set(usedColors);
  return PALETTE.find((c) => !used.has(c)) ?? PALETTE[0];
}
