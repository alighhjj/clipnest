import { useEffect, useState } from "react";

import type { AppInfo, ClipStats, Settings } from "../types";
import { prettyShortcut } from "../utils";
import { CloseIcon } from "./Icons";

interface Props {
  settings: Settings;
  info: AppInfo | null;
  stats: ClipStats | null;
  onChange: (next: Settings) => void;
  onClear: (keepPinned: boolean) => void;
  onClose: () => void;
}

interface SwitchRowProps {
  title: string;
  description: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}

function SwitchRow({ title, description, checked, onChange }: SwitchRowProps) {
  return (
    <label className="row row--switch">
      <span className="row__text">
        <span className="row__title">{title}</span>
        <span className="row__desc">{description}</span>
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        className={`switch ${checked ? "is-on" : ""}`}
        onClick={() => onChange(!checked)}
      >
        <span className="switch__knob" />
      </button>
    </label>
  );
}

export default function SettingsPanel({
  settings,
  info,
  stats,
  onChange,
  onClear,
  onClose,
}: Props) {
  const [draft, setDraft] = useState<Settings>(settings);
  const [shortcutText, setShortcutText] = useState(settings.shortcut);

  useEffect(() => {
    setDraft(settings);
    setShortcutText(settings.shortcut);
  }, [settings]);

  const patch = (partial: Partial<Settings>) => {
    const next = { ...draft, ...partial };
    setDraft(next);
    onChange(next);
  };

  const commitShortcut = () => {
    const value = shortcutText.trim();
    if (!value || value === draft.shortcut) {
      setShortcutText(draft.shortcut);
      return;
    }
    patch({ shortcut: value });
  };

  return (
    <aside className="settings" onClick={(event) => event.stopPropagation()}>
      <header className="settings__head">
        <h2>设置</h2>
        <button type="button" className="icon-btn" title="关闭设置" onClick={onClose}>
          <CloseIcon size={15} />
        </button>
      </header>

      <div className="settings__body">
        <section className="settings__group">
          <h3>记录</h3>
          <SwitchRow
            title="记录剪贴板"
            description="关闭后只保留已有历史，不再写入新内容"
            checked={draft.enabled}
            onChange={(value) => patch({ enabled: value })}
          />
          <SwitchRow
            title="失焦自动隐藏"
            description="点到其它窗口时把面板收起来"
            checked={draft.hideOnBlur}
            onChange={(value) => patch({ hideOnBlur: value })}
          />

          <label className="row">
            <span className="row__text">
              <span className="row__title">轮询间隔</span>
              <span className="row__desc">越短越灵敏，但更耗电（200 – 10000 毫秒）</span>
            </span>
            <input
              className="input input--num"
              type="number"
              min={200}
              max={10000}
              step={100}
              value={draft.pollIntervalMs}
              onChange={(event) =>
                setDraft({ ...draft, pollIntervalMs: Number(event.target.value) || 0 })
              }
              onBlur={(event) => patch({ pollIntervalMs: Number(event.target.value) || 700 })}
            />
          </label>

          <label className="row">
            <span className="row__text">
              <span className="row__title">保留条数</span>
              <span className="row__desc">超出后自动淘汰最旧的非置顶条目</span>
            </span>
            <input
              className="input input--num"
              type="number"
              min={20}
              max={20000}
              step={50}
              value={draft.maxItems}
              onChange={(event) =>
                setDraft({ ...draft, maxItems: Number(event.target.value) || 0 })
              }
              onBlur={(event) => patch({ maxItems: Number(event.target.value) || 500 })}
            />
          </label>
        </section>

        <section className="settings__group">
          <h3>快捷键</h3>
          <label className="row row--stack">
            <span className="row__text">
              <span className="row__title">全局唤出</span>
              <span className="row__desc">
                当前显示为 <code>{prettyShortcut(draft.shortcut)}</code>，支持
                ctrl / shift / alt / super + KeyA、Digit1、Space 等写法
              </span>
            </span>
            <input
              className="input"
              value={shortcutText}
              spellCheck={false}
              onChange={(event) => setShortcutText(event.target.value)}
              onBlur={commitShortcut}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.currentTarget.blur();
                }
              }}
            />
          </label>
        </section>

        <section className="settings__group settings__group--danger">
          <h3>数据</h3>
          <p className="settings__hint">
            当前共 <strong>{stats?.total ?? 0}</strong> 条，其中置顶 {stats?.pinned ?? 0} 条。
          </p>
          <div className="settings__actions">
            <button type="button" className="ghost-btn" onClick={() => onClear(true)}>
              清空历史（保留置顶）
            </button>
            <button
              type="button"
              className="ghost-btn ghost-btn--danger"
              onClick={() => onClear(false)}
            >
              全部清空
            </button>
          </div>
        </section>

        <section className="settings__group">
          <h3>关于</h3>
          <dl className="kv">
            <dt>版本</dt>
            <dd>v{info?.version ?? "—"}</dd>
            <dt>数据目录</dt>
            <dd className="kv__path">{info?.dataDir ?? "—"}</dd>
            <dt>图片目录</dt>
            <dd className="kv__path">{info?.mediaDir ?? "—"}</dd>
          </dl>
        </section>
      </div>
    </aside>
  );
}
