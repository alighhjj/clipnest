export type ClipKind = "text" | "image";

export type Filter = "all" | "text" | "image" | "pinned";

export interface Clip {
  id: number;
  kind: ClipKind;
  content: string;
  preview: string;
  byteSize: number;
  width: number | null;
  height: number | null;
  pinned: boolean;
  createdAt: number;
  imagePath: string | null;
  thumbPath: string | null;
}

export interface ClipQuery {
  search?: string;
  filter?: Filter;
  limit?: number;
  offset?: number;
}

export interface ClipStats {
  total: number;
  text: number;
  image: number;
  pinned: number;
}

export interface Settings {
  enabled: boolean;
  pollIntervalMs: number;
  maxItems: number;
  shortcut: string;
  hideOnBlur: boolean;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  mediaDir: string;
  clipboardReady: boolean;
}
