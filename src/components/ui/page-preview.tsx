import { cn } from "@/lib/cn";

type PaperSize = "a4" | "letter";

// portrait width:height ratios
const ASPECT: Record<PaperSize, string> = {
  a4: "1 / 1.4142",
  letter: "1 / 1.2941",
};

interface PagePreviewProps {
  /** Rendered page image (PNG/data URL). Omit to show an empty paper surface. */
  src?: string;
  paper?: PaperSize;
  /** Rendered width in px; height follows the paper aspect ratio. */
  width?: number;
  pageNumber?: number;
  label?: string;
  selected?: boolean;
  onClick?: () => void;
  className?: string;
}

// The document-app primitive: a paper-aspect surface that will hold a rendered
// PDF page (Phase 1.2 wires `src` to pdfium output). Used in the splitter grid,
// the builder live-preview, and library thumbnails.
export function PagePreview({
  src,
  paper = "a4",
  width = 160,
  pageNumber,
  label,
  selected = false,
  onClick,
  className,
}: PagePreviewProps) {
  const interactive = typeof onClick === "function";
  return (
    <figure className={cn("flex flex-col items-center gap-1.5", className)}>
      <div
        role={interactive ? "button" : undefined}
        tabIndex={interactive ? 0 : undefined}
        onClick={onClick}
        onKeyDown={
          interactive
            ? (e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onClick?.();
                }
              }
            : undefined
        }
        style={{ width, aspectRatio: ASPECT[paper] }}
        className={cn(
          "relative overflow-hidden rounded-sm bg-white shadow-[var(--shadow-popover)] ring-1 transition-shadow",
          selected ? "ring-2 ring-[var(--color-accent)]" : "ring-black/10",
          interactive &&
            "cursor-pointer hover:shadow-[var(--shadow-elevated)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-accent)]",
        )}
      >
        {src ? (
          <img
            src={src}
            alt={label ?? `Side ${pageNumber ?? ""}`}
            className="h-full w-full object-contain"
          />
        ) : (
          <div className="grid h-full w-full place-items-center text-[10px] text-neutral-400">
            {paper.toUpperCase()}
          </div>
        )}
        {pageNumber != null && (
          <span className="absolute right-1 bottom-1 rounded bg-black/60 px-1.5 py-0.5 text-[10px] font-medium text-white">
            {pageNumber}
          </span>
        )}
      </div>
      {label && (
        <figcaption className="max-w-full truncate text-[var(--text-ui-xs)] text-[var(--color-fg-muted)]">
          {label}
        </figcaption>
      )}
    </figure>
  );
}
