import { memo } from "react";

import { assetUrl } from "../api";
import type { Clip } from "../types";
import { charCount, formatBytes, relativeTime } from "../utils";
import { CopyIcon, ImageIcon, PinIcon, TextIcon, TrashIcon } from "./Icons";

interface Props {
  clip: Clip;
  selected: boolean;
  onSelect: () => void;
  onActivate: () => void;
  onTogglePin: () => void;
  onDelete: () => void;
  onPreview: () => void;
}

function ClipItemInner({
  clip,
  selected,
  onSelect,
  onActivate,
  onTogglePin,
  onDelete,
  onPreview,
}: Props) {
  const thumb = clip.thumbPath ? assetUrl(clip.thumbPath) : null;

  const className = [
    "clip",
    `clip--${clip.kind}`,
    selected ? "is-selected" : "",
    clip.pinned ? "is-pinned" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const stop = (handler: () => void) => (event: React.MouseEvent) => {
    event.stopPropagation();
    handler();
  };

  return (
    <article className={className} onClick={onSelect} onDoubleClick={onActivate}>
      <header className="clip__bar">
        <span className="clip__kind">
          {clip.kind === "text" ? <TextIcon size={13} /> : <ImageIcon size={13} />}
          {clip.kind === "text" ? "文本" : "图片"}
        </span>
        <span className="clip__dot">·</span>
        <span className="clip__time">{relativeTime(clip.createdAt)}</span>
        {clip.pinned && <span className="clip__badge">置顶</span>}
        <span className="clip__spacer" />

        <div className="clip__actions">
          <button
            type="button"
            className={`icon-btn ${clip.pinned ? "is-active" : ""}`}
            title={clip.pinned ? "取消置顶" : "置顶（Ctrl+P）"}
            onClick={stop(onTogglePin)}
          >
            <PinIcon size={14} filled={clip.pinned} />
          </button>
          <button
            type="button"
            className="icon-btn"
            title="复制到剪贴板"
            onClick={stop(onActivate)}
          >
            <CopyIcon size={14} />
          </button>
          <button
            type="button"
            className="icon-btn icon-btn--danger"
            title="删除（Delete）"
            onClick={stop(onDelete)}
          >
            <TrashIcon size={14} />
          </button>
        </div>
      </header>

      <div className="clip__body">
        {clip.kind === "text" ? (
          <p className="clip__text">{clip.preview}</p>
        ) : (
          <button
            type="button"
            className="clip__thumb"
            title="点击查看大图"
            onClick={stop(onPreview)}
          >
            {thumb ? (
              <img src={thumb} alt="剪贴板图片" loading="lazy" draggable={false} />
            ) : (
              <span className="clip__thumb-missing">缩略图缺失</span>
            )}
          </button>
        )}
      </div>

      <footer className="clip__foot">
        <span>
          {clip.kind === "text"
            ? `${charCount(clip.content)} 字符`
            : `${clip.width ?? "?"} × ${clip.height ?? "?"}`}
        </span>
        <span className="clip__dot">·</span>
        <span>{formatBytes(clip.byteSize)}</span>
      </footer>
    </article>
  );
}

export default memo(ClipItemInner);
