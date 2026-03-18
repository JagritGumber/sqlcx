export interface RawParam {
  index: number;
  column: string | null;
  override?: string;
}

export function resolveParamNames(params: RawParam[]): string[] {
  // First pass: apply overrides and collect column frequency
  const freq = new Map<string, number>();
  for (const p of params) {
    if (!p.override && p.column) {
      freq.set(p.column, (freq.get(p.column) ?? 0) + 1);
    }
  }

  // Second pass: assign names with collision suffixes
  const counters = new Map<string, number>();
  const assigned: string[] = params.map((p) => {
    if (p.override) return p.override;
    if (!p.column) return `param_${p.index}`;

    const count = freq.get(p.column) ?? 0;
    if (count > 1) {
      const n = (counters.get(p.column) ?? 0) + 1;
      counters.set(p.column, n);
      return `${p.column}_${n}`;
    }

    return p.column;
  });

  // Third pass: detect and fix duplicates (override-vs-inferred or suffix-vs-literal)
  const seen = new Set<string>();
  for (let i = 0; i < assigned.length; i++) {
    let name = assigned[i];
    while (seen.has(name)) {
      name = `${name}_${i + 1}`;
    }
    assigned[i] = name;
    seen.add(name);
  }

  return assigned;
}
