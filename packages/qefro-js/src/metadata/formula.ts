/** Preview-only formula evaluation. The server remains authoritative. */

type Node =
  | { kind: "num"; value: number }
  | { kind: "field"; name: string }
  | { kind: "child"; table: string; field: string }
  | { kind: "bin"; op: string; left: Node; right: Node }
  | { kind: "call"; name: string; args: Node[] };

export function previewFormula(
  formula: string,
  record: Record<string, unknown>,
  children: Record<string, Array<Record<string, unknown>>> = {},
): number | null {
  try {
    const expr = parse(formula);
    return evalNode(expr, record, children);
  } catch {
    return null;
  }
}

function parse(src: string): Node {
  const p = { s: src.trim(), i: 0 };
  const expr = parseExpr(p);
  skip(p);
  if (p.i !== p.s.length) throw new Error("trailing");
  return expr;
}

function skip(p: { s: string; i: number }) {
  while (p.s[p.i] && /\s/.test(p.s[p.i])) p.i += 1;
}

function parseExpr(p: { s: string; i: number }): Node {
  let left = parseTerm(p);
  for (;;) {
    skip(p);
    const op = p.s[p.i];
    if (op !== "+" && op !== "-") break;
    p.i += 1;
    left = { kind: "bin", op, left, right: parseTerm(p) };
  }
  return left;
}

function parseTerm(p: { s: string; i: number }): Node {
  let left = parseFactor(p);
  for (;;) {
    skip(p);
    const op = p.s[p.i];
    if (op !== "*" && op !== "/" && op !== "%") break;
    p.i += 1;
    left = { kind: "bin", op, left, right: parseFactor(p) };
  }
  return left;
}

function parseFactor(p: { s: string; i: number }): Node {
  skip(p);
  const c = p.s[p.i];
  if (c === "(") {
    p.i += 1;
    const inner = parseExpr(p);
    skip(p);
    p.i += 1;
    return inner;
  }
  if (c === "-") {
    p.i += 1;
    return { kind: "bin", op: "-", left: { kind: "num", value: 0 }, right: parseFactor(p) };
  }
  if (c && /[0-9.]/.test(c)) {
    const start = p.i;
    while (p.s[p.i] && /[0-9.]/.test(p.s[p.i])) p.i += 1;
    return { kind: "num", value: Number(p.s.slice(start, p.i)) };
  }
  const start = p.i;
  while (p.s[p.i] && /[A-Za-z0-9_]/.test(p.s[p.i])) p.i += 1;
  const ident = p.s.slice(start, p.i);
  skip(p);
  if (p.s[p.i] === "(") {
    p.i += 1;
    const args: Node[] = [];
    skip(p);
    if (p.s[p.i] !== ")") {
      for (;;) {
        args.push(parseExpr(p));
        skip(p);
        if (p.s[p.i] === ",") {
          p.i += 1;
          continue;
        }
        break;
      }
    }
    p.i += 1;
    return { kind: "call", name: ident.toUpperCase(), args };
  }
  if (p.s[p.i] === ".") {
    p.i += 1;
    const f0 = p.i;
    while (p.s[p.i] && /[A-Za-z0-9_]/.test(p.s[p.i])) p.i += 1;
    return { kind: "child", table: ident, field: p.s.slice(f0, p.i) };
  }
  return { kind: "field", name: ident };
}

function evalNode(
  node: Node,
  record: Record<string, unknown>,
  children: Record<string, Array<Record<string, unknown>>>,
): number {
  switch (node.kind) {
    case "num":
      return node.value;
    case "field":
      if (children[node.name]) return children[node.name].length;
      return num(record[node.name]);
    case "child":
      return (children[node.table] ?? []).reduce((s, row) => s + num(row[node.field]), 0);
    case "bin": {
      const l = evalNode(node.left, record, children);
      const r = evalNode(node.right, record, children);
      if (node.op === "+") return l + r;
      if (node.op === "-") return l - r;
      if (node.op === "*") return l * r;
      if (node.op === "/") return r === 0 ? 0 : l / r;
      return r === 0 ? 0 : l % r;
    }
    case "call": {
      if (node.name === "ROUND") {
        const v = evalNode(node.args[0], record, children);
        const d = node.args[1] ? evalNode(node.args[1], record, children) : 0;
        const f = 10 ** d;
        return Math.round(v * f) / f;
      }
      const arg = node.args[0];
      const rows =
        arg?.kind === "child"
          ? (children[arg.table] ?? []).map((row) => num(row[arg.field]))
          : arg?.kind === "field"
            ? (children[arg.name] ?? []).map(() => 1)
            : [evalNode(arg, record, children)];
      if (node.name === "SUM") return rows.reduce((a, b) => a + b, 0);
      if (node.name === "COUNT") return rows.length;
      if (node.name === "MIN") return rows.length ? Math.min(...rows) : 0;
      if (node.name === "MAX") return rows.length ? Math.max(...rows) : 0;
      return 0;
    }
  }
}

function num(v: unknown): number {
  if (typeof v === "number") return v;
  if (typeof v === "string" && v !== "") return Number(v) || 0;
  return 0;
}
