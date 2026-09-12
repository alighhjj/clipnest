const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

function pad(value: number): string {
  return value.toString().padStart(2, "0");
}

/** 相对时间：刚刚 / 12 分钟前 / 昨天 14:03 / 3月5日 */
export function relativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;

  if (diff < 0) return "刚刚";
  if (diff < MINUTE) return "刚刚";
  if (diff < HOUR) return `${Math.floor(diff / MINUTE)} 分钟前`;

  const date = new Date(timestamp);
  const today = new Date();
  const sameDay =
    date.getFullYear() === today.getFullYear() &&
    date.getMonth() === today.getMonth() &&
    date.getDate() === today.getDate();

  if (sameDay) return `${pad(date.getHours())}:${pad(date.getMinutes())}`;

  const yesterday = new Date(today.getTime() - DAY);
  const isYesterday =
    date.getFullYear() === yesterday.getFullYear() &&
    date.getMonth() === yesterday.getMonth() &&
    date.getDate() === yesterday.getDate();

  if (isYesterday) return `昨天 ${pad(date.getHours())}:${pad(date.getMinutes())}`;

  if (date.getFullYear() === today.getFullYear()) {
    return `${date.getMonth() + 1}月${date.getDate()}日`;
  }
  return `${date.getFullYear()}/${date.getMonth() + 1}/${date.getDate()}`;
}

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 100 || unit === 0 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

export function isMacPlatform(): boolean {
  return typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
}

/** 把 `ctrl+shift+KeyV` 渲染成更好看的 `Ctrl + Shift + V` */
export function prettyShortcut(spec: string): string {
  const mac = isMacPlatform();
  return spec
    .split("+")
    .map((raw) => raw.trim())
    .filter(Boolean)
    .map((token) => {
      const lower = token.toLowerCase();
      if (lower === "control" || lower === "ctrl") return mac ? "⌃" : "Ctrl";
      if (lower === "shift") return mac ? "⇧" : "Shift";
      if (lower === "alt") return mac ? "⌥" : "Alt";
      if (lower === "super" || lower === "meta" || lower === "cmd") return mac ? "⌘" : "Win";
      if (/^key[a-z]$/i.test(token)) return token.slice(3).toUpperCase();
      if (/^digit\d$/i.test(token)) return token.slice(5);
      return token;
    })
    .join(mac ? " " : " + ");
}

export function charCount(text: string): number {
  return Array.from(text).length;
}
