export interface RawParam {
  index: number;
  column: string | null;
  override?: string;
}

export function resolveParamNames(params: RawParam[]): string[] {
  // Pass 1: count column frequency (needed to know which columns collide)
  const freq = new Map<string, number>();
  for (const p of params) {
    if (!p.override && p.column) {
      freq.set(p.column, (freq.get(p.column) ?? 0) + 1);
    }
  }

  // Pass 2: assign names + dedup in one go
  const counters = new Map<string, number>();
  const seen = new Set<string>();
  const result: string[] = new Array(params.length);

  for (let i = 0; i < params.length; i++) {
    const p = params[i];
    let name: string;

    if (p.override) {
      name = p.override;
    } else if (!p.column) {
      name = `param_${p.index}`;
    } else if ((freq.get(p.column) ?? 0) > 1) {
      const n = (counters.get(p.column) ?? 0) + 1;
      counters.set(p.column, n);
      name = `${p.column}_${n}`;
    } else {
      name = p.column;
    }

    // Dedup: resolve any remaining collisions (override-vs-inferred, suffix-vs-literal)
    const base = name;
    let suffix = 1;
    while (seen.has(name)) {
      name = `${base}_${suffix++}`;
    }

    seen.add(name);
    result[i] = name;
  }

  return result;
}
