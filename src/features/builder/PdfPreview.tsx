/**
 * PdfPreview — renders a compiled PDF inline and offers a download.
 *
 * The backend hands us the PDF as a base64 string (no data-URL prefix, matching
 * `typst_compile` / `pdf_render_page`). We wrap it in a `data:` URL for the
 * `<embed>` and build an object URL for the download so a click saves a real
 * `.pdf` rather than a giant href. The object URL is torn down on unmount /
 * when the bytes change so we don't leak blobs.
 */

import { useEffect, useMemo, useState } from "react";
import { Download } from "lucide-react";

/** Decode a base64 string into a `Blob` of `application/pdf`. */
function base64ToPdfBlob(base64: string): Blob {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return new Blob([bytes], { type: "application/pdf" });
}

interface PdfPreviewProps {
  /** Base64-encoded PDF bytes (no `data:` prefix). */
  base64: string;
  /** File name suggested by the download button. */
  fileName?: string;
}

export function PdfPreview({
  base64,
  fileName = "program.pdf",
}: PdfPreviewProps) {
  // Object URL for the download anchor — created from the bytes, revoked on change.
  const [downloadUrl, setDownloadUrl] = useState<string | null>(null);

  useEffect(() => {
    const url = URL.createObjectURL(base64ToPdfBlob(base64));
    setDownloadUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [base64]);

  // Data URL feeds the inline embed (cheap, no cleanup needed).
  const dataUrl = useMemo(
    () => `data:application/pdf;base64,${base64}`,
    [base64],
  );

  return (
    <div className="flex h-full flex-col overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] shadow-[var(--shadow-soft)]">
      <div className="flex items-center justify-between border-b border-[var(--color-border)] px-4 py-2.5">
        <span className="text-sm font-semibold">Forhåndsvisning</span>
        {downloadUrl && (
          <a
            href={downloadUrl}
            download={fileName}
            className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-xs font-bold text-[var(--color-accent-fg)] transition-all hover:brightness-110"
          >
            <Download size={13} />
            Last ned PDF
          </a>
        )}
      </div>
      <embed
        title="PDF-forhåndsvisning"
        aria-label="PDF-forhåndsvisning"
        src={dataUrl}
        type="application/pdf"
        className="min-h-0 flex-1"
      />
    </div>
  );
}
