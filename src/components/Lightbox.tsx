import { useEffect } from "react";

import { assetUrl } from "../api";
import type { Clip } from "../types";
import { formatBytes, relativeTime } from "../utils";

interface Props {
  clip: Clip;
  onCopy: () => void;
  onClose: () => void;
}

export default function Lightbox({ clip, onCopy, onClose }: Props) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  const src = clip.imagePath ? assetUrl(clip.imagePath) : null;

  return (
    <div className="lightbox" onClick={onClose} role="dialog" aria-modal="true">
      <div className="lightbox__frame" onClick={(event) => event.stopPropagation()}>
        {src ? (
          <img className="lightbox__img" src={src} alt="剪贴板图片" draggable={false} />
        ) : (
          <div className="lightbox__empty">原图文件已丢失</div>
        )}
        <div className="lightbox__meta">
          <span>
            {clip.width} × {clip.height}
          </span>
          <span className="clip__dot">·</span>
          <span>{formatBytes(clip.byteSize)}</span>
          <span className="clip__dot">·</span>
          <span>{relativeTime(clip.createdAt)}</span>
          <span className="lightbox__spacer" />
          <button type="button" className="ghost-btn" onClick={onCopy}>
            复制到剪贴板
          </button>
          <button type="button" className="ghost-btn ghost-btn--primary" onClick={onClose}>
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
