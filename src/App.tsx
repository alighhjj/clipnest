import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import * as api from "./api";
import ClipItem from "./components/ClipItem";
import Lightbox from "./components/Lightbox";
import SettingsPanel from "./components/SettingsPanel";
import {
  ClipboardMark,
  CloseIcon,
  GearIcon,
  ImageIcon,
  LayersIcon,
  MinimizeIcon,
  SearchIcon,
  StarIcon,
  TextIcon,
} from "./components/Icons";
import type { AppInfo, Clip, ClipStats, Filter, Settings } from "./types";
import { prettyShortcut } from "./utils";

const FILTERS: { id: Filter; label: string; icon: ReactNode }[] = [
  { id: "all", label: "全部", icon: <LayersIcon size={14} /> },
  { id: "text", label: "文本", icon: <TextIcon size={14} /> },
  { id: "image", label: "图片", icon: <ImageIcon size={14} /> },
  { id: "pinned", label: "置顶", icon: <StarIcon size={14} /> },
];

const PAGE_SIZE = 400;

export default function App() {
  const [clips, setClips] = useState<Clip[]>([]);
  const [stats, setStats] = useState<ClipStats | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);

  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [selected, setSelected] = useState(0);
  const [loading, setLoading] = useState(true);

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [previewing, setPreviewing] = useState<Clip | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const listRef = useRef<HTMLElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const toastTimer = useRef<number | null>(null);

  const notify = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current !== null) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2600);
  }, []);

  // ------------------------------------------------------------ 数据加载

  const refresh = useCallback(
    async (options?: { resetSelection?: boolean }) => {
      try {
        const [nextClips, nextStats] = await Promise.all([
          api.listClips({ search, filter, limit: PAGE_SIZE }),
          api.fetchStats(),
        ]);
        setClips(nextClips);
        setStats(nextStats);
        setSelected((current) => {
          if (options?.resetSelection || nextClips.length === 0) return 0;
          return Math.min(current, nextClips.length - 1);
        });
      } catch (error) {
        notify(api.describeError(error));
      } finally {
        setLoading(false);
      }
    },
    [search, filter, notify],
  );

  // 监听器里要用最新的 refresh，但监听本身只注册一次
  const refreshRef = useRef(refresh);
  useEffect(() => {
    refreshRef.current = refresh;
  }, [refresh]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void refreshRef.current();
    }, 120);
    return () => window.clearTimeout(timer);
  }, [refresh]);

  useEffect(() => {
    void (async () => {
      try {
        const [nextSettings, nextInfo] = await Promise.all([
          api.getSettings(),
          api.fetchAppInfo(),
        ]);
        setSettings(nextSettings);
        setInfo(nextInfo);
      } catch (error) {
        notify(api.describeError(error));
      }
    })();
  }, [notify]);

  useEffect(() => {
    let disposed = false;
    const disposers: Array<() => void> = [];

    void (async () => {
      const registered = await Promise.all([
        listen("clipnest://clip-added", () => {
          void refreshRef.current();
        }),
        listen("clipnest://history-cleared", () => {
          void refreshRef.current({ resetSelection: true });
        }),
        listen<Settings>("clipnest://settings-changed", (event) => {
          setSettings(event.payload);
        }),
        listen("clipnest://focus-search", () => {
          setSearch("");
          setSettingsOpen(false);
          setPreviewing(null);
          void refreshRef.current({ resetSelection: true }).then(() => {
            requestAnimationFrame(() => {
              searchRef.current?.focus();
              searchRef.current?.select();
            });
          });
        }),
      ]);

      if (disposed) {
        registered.forEach((unlisten) => unlisten());
        return;
      }
      disposers.push(...registered);
    })();

    return () => {
      disposed = true;
      disposers.forEach((unlisten) => unlisten());
    };
  }, []);

  // 选中项始终保持在可视区域内
  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>(".clip.is-selected")
      ?.scrollIntoView({ block: "nearest" });
  }, [selected, clips]);

  // ------------------------------------------------------------ 行为

  const selectedClip = clips[selected] ?? null;

  const move = (delta: number) => {
    setSelected((current) => {
      if (clips.length === 0) return 0;
      return Math.min(Math.max(current + delta, 0), clips.length - 1);
    });
  };

  const activate = async (clip: Clip) => {
    try {
      await api.activateClip(clip.id);
    } catch (error) {
      notify(api.describeError(error));
    }
  };

  const copy = async (clip: Clip) => {
    try {
      await api.copyClip(clip.id);
      notify("已复制到剪贴板");
    } catch (error) {
      notify(api.describeError(error));
    }
  };

  const pin = async (clip: Clip) => {
    try {
      await api.togglePin(clip.id, !clip.pinned);
      await refreshRef.current();
    } catch (error) {
      notify(api.describeError(error));
    }
  };

  const remove = async (clip: Clip) => {
    try {
      await api.deleteClip(clip.id);
      await refreshRef.current();
    } catch (error) {
      notify(api.describeError(error));
    }
  };

  const changeSettings = async (next: Settings) => {
    try {
      const saved = await api.updateSettings(next);
      setSettings(saved);
    } catch (error) {
      notify(api.describeError(error));
      try {
        setSettings(await api.getSettings());
      } catch {
        /* 回滚失败也不值得再打扰用户 */
      }
    }
  };

  const clearHistory = async (keepPinned: boolean) => {
    try {
      const removed = await api.clearClips(keepPinned);
      notify(removed > 0 ? `已清除 ${removed} 条记录` : "没有可清除的记录");
      await refreshRef.current({ resetSelection: true });
    } catch (error) {
      notify(api.describeError(error));
    }
  };

  // ------------------------------------------------------------ 键盘

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement;
    if (target.closest(".settings") || target.closest(".lightbox")) return;

    const mod = event.metaKey || event.ctrlKey;
    const key = event.key;
    const inInput = target.tagName === "INPUT";

    if (key === "Escape") {
      event.preventDefault();
      if (search) {
        setSearch("");
        searchRef.current?.focus();
        return;
      }
      void api.hideWindow();
      return;
    }

    if (key === "ArrowDown") {
      event.preventDefault();
      move(1);
      return;
    }
    if (key === "ArrowUp") {
      event.preventDefault();
      move(-1);
      return;
    }
    if (key === "Home") {
      event.preventDefault();
      setSelected(0);
      return;
    }
    if (key === "End") {
      event.preventDefault();
      setSelected(Math.max(0, clips.length - 1));
      return;
    }

    if (key === "Enter" && selectedClip && !mod) {
      event.preventDefault();
      void activate(selectedClip);
      return;
    }
    if (mod && key.toLowerCase() === "c" && selectedClip && !inInput) {
      event.preventDefault();
      void copy(selectedClip);
      return;
    }
    if (mod && key.toLowerCase() === "p" && selectedClip) {
      event.preventDefault();
      void pin(selectedClip);
      return;
    }
    if (key === "Delete" && selectedClip) {
      event.preventDefault();
      void remove(selectedClip);
    }
  };

  // ------------------------------------------------------------ 渲染

  const counts: Record<Filter, number> = {
    all: stats?.total ?? 0,
    text: stats?.text ?? 0,
    image: stats?.image ?? 0,
    pinned: stats?.pinned ?? 0,
  };

  return (
    <div
      className="app"
      onKeyDown={handleKeyDown}
      onContextMenu={(event) => event.preventDefault()}
    >
      <header className="titlebar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <span className="brand__mark">
            <ClipboardMark size={20} />
          </span>
          <span className="brand__name">ClipNest</span>
          <span
            className={`brand__state ${settings?.enabled === false ? "is-paused" : "is-recording"}`}
            title={settings?.enabled === false ? "已暂停记录" : "正在记录剪贴板"}
          >
            {settings?.enabled === false ? "已暂停" : "记录中"}
          </span>
        </div>

        <div className="search">
          <SearchIcon size={15} />
          <input
            ref={searchRef}
            className="search__input"
            placeholder="搜索历史记录…"
            value={search}
            spellCheck={false}
            autoFocus
            onChange={(event) => setSearch(event.target.value)}
          />
          {search && (
            <button
              type="button"
              className="search__clear"
              title="清空搜索"
              onClick={() => {
                setSearch("");
                searchRef.current?.focus();
              }}
            >
              <CloseIcon size={13} />
            </button>
          )}
        </div>

        <div className="titlebar__actions">
          <button
            type="button"
            className={`icon-btn ${settingsOpen ? "is-active" : ""}`}
            title="设置"
            onClick={() => setSettingsOpen((open) => !open)}
          >
            <GearIcon size={16} />
          </button>
          <button
            type="button"
            className="icon-btn"
            title="最小化"
            onClick={() => void getCurrentWindow().minimize()}
          >
            <MinimizeIcon size={16} />
          </button>
          <button
            type="button"
            className="icon-btn icon-btn--danger"
            title="收起到托盘（Esc）"
            onClick={() => void api.hideWindow()}
          >
            <CloseIcon size={16} />
          </button>
        </div>
      </header>

      <nav className="filters">
        {FILTERS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`chip ${filter === item.id ? "is-active" : ""}`}
            onClick={() => {
              setFilter(item.id);
              setSelected(0);
            }}
          >
            {item.icon}
            <span>{item.label}</span>
            <span className="chip__count">{counts[item.id]}</span>
          </button>
        ))}
      </nav>

      <main className="list" ref={listRef}>
        {loading ? (
          <div className="state">
            <p className="state__desc">正在读取历史…</p>
          </div>
        ) : clips.length === 0 ? (
          <div className="state">
            <ClipboardMark size={30} />
            <p className="state__title">{search ? "没有匹配的记录" : "还没有记录"}</p>
            <p className="state__desc">
              {search ? "换个关键词再看看" : "复制任意文本或图片，它就会出现在这里"}
            </p>
          </div>
        ) : (
          clips.map((clip, index) => (
            <ClipItem
              key={clip.id}
              clip={clip}
              selected={index === selected}
              onSelect={() => setSelected(index)}
              onActivate={() => void activate(clip)}
              onTogglePin={() => void pin(clip)}
              onDelete={() => void remove(clip)}
              onPreview={() => setPreviewing(clip)}
            />
          ))
        )}
      </main>

      <footer className="statusbar">
        <span className="statusbar__count">
          显示 {clips.length} / {stats?.total ?? 0} 条
        </span>
        <span className="statusbar__spacer" />
        <span className="hint">
          <kbd>↑</kbd>
          <kbd>↓</kbd>选择
        </span>
        <span className="hint">
          <kbd>Enter</kbd>复制
        </span>
        <span className="hint">
          <kbd>Del</kbd>删除
        </span>
        <span className="hint">
          <kbd>Esc</kbd>收起
        </span>
        {settings && (
          <span className="statusbar__shortcut" title="全局唤出快捷键">
            {prettyShortcut(settings.shortcut)}
          </span>
        )}
      </footer>

      {settingsOpen && (
        <div className="settings-backdrop" onClick={() => setSettingsOpen(false)} />
      )}

      {settingsOpen && settings && (
        <SettingsPanel
          settings={settings}
          info={info}
          stats={stats}
          onChange={(next) => void changeSettings(next)}
          onClear={(keepPinned) => void clearHistory(keepPinned)}
          onClose={() => setSettingsOpen(false)}
        />
      )}

      {previewing && (
        <Lightbox
          clip={previewing}
          onCopy={() => void copy(previewing)}
          onClose={() => setPreviewing(null)}
        />
      )}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
