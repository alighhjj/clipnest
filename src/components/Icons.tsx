interface IconProps {
  size?: number;
}

const base = (size: number) => ({
  width: size,
  height: size,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.7,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
});

export function SearchIcon({ size = 16 }: IconProps) {
  return (
    <svg {...base(size)}>
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.2-3.2" />
    </svg>
  );
}

export function GearIcon({ size = 16 }: IconProps) {
  return (
    <svg {...base(size)}>
      <circle cx="12" cy="12" r="3.2" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 8.9 19.3a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.7 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1.03-1.56V3a2 2 0 1 1 4 0v.09A1.7 1.7 0 0 0 15.1 4.7a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9v.09c0 .7.42 1.32 1.07 1.58" />
    </svg>
  );
}

export function PinIcon({ size = 15, filled = false }: IconProps & { filled?: boolean }) {
  return (
    <svg {...base(size)} fill={filled ? "currentColor" : "none"}>
      <path d="M15.5 3.5 20.5 8.5l-2.6.7-3.1 3.1-.6 3.6-1.7 1.7-2.5-2.5-2.5-2.5 1.7-1.7 3.6-.6 3.1-3.1z" />
      <path d="m7.5 16.5-4 4" />
    </svg>
  );
}

export function CopyIcon({ size = 15 }: IconProps) {
  return (
    <svg {...base(size)}>
      <rect x="9" y="9" width="11" height="11" rx="2.2" />
      <path d="M6 15H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v1" />
    </svg>
  );
}

export function TrashIcon({ size = 15 }: IconProps) {
  return (
    <svg {...base(size)}>
      <path d="M4 7h16" />
      <path d="M9.5 7V5.4A1.4 1.4 0 0 1 10.9 4h2.2a1.4 1.4 0 0 1 1.4 1.4V7" />
      <path d="M6.5 7 7.4 19a1.6 1.6 0 0 0 1.6 1.5h6a1.6 1.6 0 0 0 1.6-1.5L17.5 7" />
      <path d="M10.5 11v5.5M13.5 11v5.5" />
    </svg>
  );
}

export function TextIcon({ size = 15 }: IconProps) {
  return (
    <svg {...base(size)}>
      <path d="M4.5 6.5V5h15v1.5" />
      <path d="M12 5v14" />
      <path d="M9 19h6" />
    </svg>
  );
}

export function ImageIcon({ size = 15 }: IconProps) {
  return (
    <svg {...base(size)}>
      <rect x="3.5" y="4.5" width="17" height="15" rx="2.4" />
      <circle cx="9" cy="10" r="1.6" />
      <path d="m4.5 17.5 4.4-4.2a1.6 1.6 0 0 1 2.2 0l2.3 2.2 2.1-2a1.6 1.6 0 0 1 2.2 0l1.8 1.7" />
    </svg>
  );
}

export function StarIcon({ size = 15 }: IconProps) {
  return (
    <svg {...base(size)}>
      <path d="m12 4 2.4 4.9 5.4.8-3.9 3.8.9 5.4-4.8-2.6-4.8 2.6.9-5.4L4.2 9.7l5.4-.8z" />
    </svg>
  );
}

export function LayersIcon({ size = 15 }: IconProps) {
  return (
    <svg {...base(size)}>
      <path d="m12 3 8.5 4.6L12 12.2 3.5 7.6z" />
      <path d="m3.5 12.4 8.5 4.6 8.5-4.6" />
      <path d="m3.5 16.9 8.5 4.6 8.5-4.6" />
    </svg>
  );
}

export function CloseIcon({ size = 15 }: IconProps) {
  return (
    <svg {...base(size)}>
      <path d="M6 6l12 12M18 6 6 18" />
    </svg>
  );
}

export function MinimizeIcon({ size = 15 }: IconProps) {
  return (
    <svg {...base(size)}>
      <path d="M6 12h12" />
    </svg>
  );
}

export function ClipboardMark({ size = 22 }: IconProps) {
  return (
    <svg {...base(size)} strokeWidth={1.6}>
      <rect x="5" y="4.5" width="14" height="16" rx="2.6" />
      <path d="M9.2 4.5V3.6A1.1 1.1 0 0 1 10.3 2.5h3.4a1.1 1.1 0 0 1 1.1 1.1v.9" />
      <path d="M8.6 10.5h6.8M8.6 14h4.4" />
    </svg>
  );
}
