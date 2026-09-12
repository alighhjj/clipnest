import { convertFileSrc, invoke } from "@tauri-apps/api/core";

import type { AppInfo, Clip, ClipQuery, ClipStats, Settings } from "./types";

export const listClips = (query: ClipQuery) => invoke<Clip[]>("list_clips", { query });

export const fetchStats = () => invoke<ClipStats>("clip_stats");

/** 写回剪贴板，窗口保持打开 */
export const copyClip = (id: number) => invoke<void>("copy_clip", { id });

/** 写回剪贴板并收起窗口 */
export const activateClip = (id: number) => invoke<void>("activate_clip", { id });

export const togglePin = (id: number, pinned: boolean) =>
  invoke<void>("toggle_pin", { id, pinned });

export const deleteClip = (id: number) => invoke<void>("delete_clip", { id });

export const clearClips = (keepPinned: boolean) =>
  invoke<number>("clear_clips", { keepPinned });

export const getSettings = () => invoke<Settings>("get_settings");

export const updateSettings = (settings: Settings) =>
  invoke<Settings>("update_settings", { settings });

export const hideWindow = () => invoke<void>("hide_window");

export const fetchAppInfo = () => invoke<AppInfo>("app_info");

/** 把 Rust 给的绝对路径转成 asset 协议 URL，供 <img> 使用 */
export const assetUrl = (path: string) => convertFileSrc(path);

export function describeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "发生了未知错误";
}
