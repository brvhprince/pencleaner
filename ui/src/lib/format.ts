// Decimal units, like Finder and the "1 TB" on the box.
const UNITS = ["KB", "MB", "GB", "TB", "PB"];

export function bytes(n: number): string {
  if (n <= 0) return "Zero KB";
  if (n < 1000) return `${n} bytes`;
  let v = n;
  let i = -1;
  do {
    v /= 1000;
    i++;
  } while (v >= 1000 && i < UNITS.length - 1);
  const digits = v >= 100 ? 0 : v >= 10 ? 1 : v >= 1 ? 1 : 2;
  return `${Number(v.toFixed(digits))} ${UNITS[i]}`;
}

export function count(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1e6) return `${Number((n / 1e3).toFixed(n < 1e4 ? 1 : 0))}K`;
  return `${Number((n / 1e6).toFixed(n < 1e7 ? 1 : 0))}M`;
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n.toLocaleString()} ${n === 1 ? one : many}`;
}

/** Keeps the start and the end of a long path visible. */
export function middleEllipsis(s: string, max = 70): string {
  if (s.length <= max) return s;
  const keep = max - 1;
  const head = Math.ceil(keep * 0.45);
  return `${s.slice(0, head)}…${s.slice(s.length - (keep - head))}`;
}

export function duration(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  return `${Math.floor(s / 60)}m ${s % 60}s`;
}

/** "today", "3 days ago", "2 months ago" from unix seconds. */
export function ago(unixSeconds: number): string {
  const d = Math.floor((Date.now() / 1000 - unixSeconds) / 86400);
  if (d < 1) return "today";
  if (d < 2) return "yesterday";
  if (d < 30) return `${d} days ago`;
  const m = Math.floor(d / 30.4);
  if (m < 12) return m === 1 ? "a month ago" : `${m} months ago`;
  const y = Math.floor(d / 365);
  return y === 1 ? "a year ago" : `${y} years ago`;
}
