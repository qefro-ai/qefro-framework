import { useEffect, useRef, useState } from "react";
import { api } from "../../api";
import { fileIcon, fileSize, relativeTime } from "../../format";
import { friendlyError } from "../../friendlyError";
import { ActionMenu } from "../ui/ActionMenu";
import { Button } from "../ui/Button";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { EmptyState } from "../ui/EmptyState";
import { showSnackbar } from "../ui/Snackbar";

type FileRow = Record<string, unknown>;
type UploadState = {
  key: string;
  file: File;
  progress: number;
  status: "uploading" | "error";
  error?: string;
  controller: AbortController;
};

function mimeOf(file: FileRow) {
  return String(file.content_type ?? file.mime_type ?? file.mime ?? "");
}
function nameOf(file: FileRow) {
  return String(file.filename ?? file.id);
}
function isImage(file: FileRow) {
  return mimeOf(file).startsWith("image/") || /\.(png|jpe?g|gif|webp)$/i.test(nameOf(file));
}
function isPdf(file: FileRow) {
  return mimeOf(file).includes("pdf") || nameOf(file).toLowerCase().endsWith(".pdf");
}
function isText(file: FileRow) {
  const mime = mimeOf(file);
  return mime.startsWith("text/") || mime === "application/json" || /\.(txt|csv|json)$/i.test(nameOf(file));
}

export function AttachmentsPanel({
  slug,
  id,
  items,
  onChanged,
}: {
  slug: string;
  id: string;
  items: FileRow[];
  onChanged: () => void;
}) {
  const [drag, setDrag] = useState(false);
  const [uploads, setUploads] = useState<UploadState[]>([]);
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const [preview, setPreview] = useState<{ file: FileRow; url?: string; text?: string; kind: string } | null>(null);
  const [pendingDelete, setPendingDelete] = useState<FileRow | null>(null);
  const [edit, setEdit] = useState<{ file: FileRow; filename: string; description: string } | null>(null);
  const [replaceTarget, setReplaceTarget] = useState<FileRow | null>(null);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const replaceRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let cancelled = false;
    const urls: string[] = [];
    async function loadThumbs() {
      const next: Record<string, string> = {};
      for (const file of items) {
        if (!isImage(file)) continue;
        try {
          const blob = await api.downloadAttachment(String(file.id), true);
          if (cancelled) return;
          const url = URL.createObjectURL(blob);
          urls.push(url);
          next[String(file.id)] = url;
        } catch {
          /* preview is best-effort */
        }
      }
      if (!cancelled) setThumbs(next);
    }
    void loadThumbs();
    return () => {
      cancelled = true;
      for (const url of urls) URL.revokeObjectURL(url);
    };
  }, [items]);

  async function uploadOne(file: File, key: string, controller: AbortController) {
    try {
      await api.uploadAttachment(slug, id, file, (n) => {
        setUploads((rows) => rows.map((row) => (row.key === key ? { ...row, progress: n } : row)));
      }, controller.signal);
      setUploads((rows) => rows.filter((row) => row.key !== key));
      showSnackbar(`${file.name} uploaded`);
      onChanged();
    } catch (err) {
      if (controller.signal.aborted) {
        setUploads((rows) => rows.filter((row) => row.key !== key));
        return;
      }
      setUploads((rows) =>
        rows.map((row) =>
          row.key === key
            ? { ...row, status: "error", error: friendlyError(err) || "Upload failed" }
            : row,
        ),
      );
    }
  }

  function enqueue(files: FileList | File[]) {
    const list = Array.from(files);
    if (!list.length) return;
    const next: UploadState[] = list.map((file) => ({
      key: `${file.name}-${file.size}-${file.lastModified}-${Math.random()}`,
      file,
      progress: 0,
      status: "uploading",
      controller: new AbortController(),
    }));
    setUploads((rows) => [...rows, ...next]);
    for (const row of next) void uploadOne(row.file, row.key, row.controller);
  }

  async function download(file: FileRow) {
    try {
      const blob = await api.downloadAttachment(String(file.id));
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = nameOf(file);
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      showSnackbar(friendlyError(err) || "Download failed", "error");
    }
  }

  async function openPreview(file: FileRow) {
    try {
      const blob = await api.downloadAttachment(String(file.id), true);
      if (isImage(file)) {
        setPreview({ file, kind: "image", url: URL.createObjectURL(blob) });
      } else if (isPdf(file)) {
        setPreview({ file, kind: "pdf", url: URL.createObjectURL(blob) });
      } else if (isText(file)) {
        setPreview({ file, kind: "text", text: await blob.text() });
      } else {
        setPreview({ file, kind: "unavailable" });
      }
    } catch (err) {
      showSnackbar(friendlyError(err) || "Preview failed", "error");
    }
  }

  function closePreview() {
    if (preview?.url) URL.revokeObjectURL(preview.url);
    setPreview(null);
  }

  return (
    <div className="attachments">
      <div
        className={`dropzone ${drag ? "is-drag" : ""}`}
        onDragOver={(e) => {
          e.preventDefault();
          setDrag(true);
        }}
        onDragLeave={() => setDrag(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDrag(false);
          if (e.dataTransfer.files.length) enqueue(e.dataTransfer.files);
        }}
      >
        <p className="muted">Drop files here or</p>
        <Button variant="tonal" onClick={() => inputRef.current?.click()}>
          Upload files
        </Button>
        <input
          ref={inputRef}
          type="file"
          multiple
          className="sr-only"
          aria-label="Upload files"
          onChange={(e) => {
            if (e.target.files?.length) enqueue(e.target.files);
            e.target.value = "";
          }}
        />
      </div>

      {uploads.map((row) => (
        <div key={row.key} className="attachment-upload" aria-live="polite">
          <span className="file-glyph" aria-hidden="true">
            {fileIcon(row.file.name, row.file.type)}
          </span>
          <div className="attachment-meta">
            <strong>{row.file.name}</strong>
            {row.status === "uploading" ? (
              <>
                <span className="muted">Uploading… {Math.round(row.progress * 100)}%</span>
                <progress value={row.progress} max={1} aria-label={`Uploading ${row.file.name}`} />
              </>
            ) : (
              <span className="error" role="alert">
                {row.file.name} could not be uploaded.
              </span>
            )}
          </div>
          {row.status === "uploading" ? (
            <Button variant="text" onClick={() => row.controller.abort()}>
              Cancel
            </Button>
          ) : (
            <Button
              variant="text"
              onClick={() => {
                const controller = new AbortController();
                setUploads((rows) =>
                  rows.map((item) =>
                    item.key === row.key
                      ? { ...item, status: "uploading", progress: 0, error: undefined, controller }
                      : item,
                  ),
                );
                void uploadOne(row.file, row.key, controller);
              }}
            >
              Retry
            </Button>
          )}
        </div>
      ))}

      {items.length === 0 && uploads.length === 0 ? (
        <EmptyState
          title="No files attached yet."
          action={
            <Button variant="tonal" onClick={() => inputRef.current?.click()}>
              Upload file
            </Button>
          }
        />
      ) : (
        <ul className="attachment-list">
          {items.map((file) => (
            <li key={String(file.id)}>
              {isImage(file) && thumbs[String(file.id)] ? (
                <img className="thumb" alt="" src={thumbs[String(file.id)]} />
              ) : (
                <span className="file-glyph" aria-hidden="true">
                  {fileIcon(nameOf(file), mimeOf(file))}
                </span>
              )}
              <div className="attachment-meta">
                <strong>{nameOf(file)}</strong>
                <span className="muted attachment-size">
                  {[
                    mimeOf(file).split("/").pop()?.toUpperCase(),
                    file.size != null ? fileSize(file.size) : "",
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
                <span className="muted">
                  {[
                    file.uploaded_by_name ? `Uploaded by ${file.uploaded_by_name}` : "",
                    relativeTime(file.created_at),
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </div>
              <div className="attachment-actions">
                <Button variant="text" onClick={() => void openPreview(file)}>
                  Open
                </Button>
                <Button variant="text" onClick={() => void download(file)}>
                  Download
                </Button>
                <ActionMenu
                  items={[
                    {
                      key: "replace",
                      label: "Replace",
                      onSelect: () => {
                        setReplaceTarget(file);
                        replaceRef.current?.click();
                      },
                    },
                    {
                      key: "edit",
                      label: "Edit details",
                      onSelect: () =>
                        setEdit({
                          file,
                          filename: nameOf(file),
                          description: String(file.description ?? ""),
                        }),
                    },
                    {
                      key: "remove",
                      label: "Delete",
                      danger: true,
                      onSelect: () => setPendingDelete(file),
                    },
                  ]}
                />
              </div>
            </li>
          ))}
        </ul>
      )}

      <input
        ref={replaceRef}
        type="file"
        className="sr-only"
        aria-label="Replace file"
        onChange={async (e) => {
          const next = e.target.files?.[0];
          const target = replaceTarget;
          e.target.value = "";
          if (!next || !target) return;
          setBusy(true);
          try {
            await api.replaceAttachment(String(target.id), next);
            showSnackbar(`${next.name} replaced`);
            onChanged();
          } catch (err) {
            showSnackbar(friendlyError(err) || "Replace failed", "error");
          } finally {
            setBusy(false);
            setReplaceTarget(null);
          }
        }}
      />

      <ConfirmDialog
        open={Boolean(pendingDelete)}
        title="Delete attachment?"
        message={
          pendingDelete
            ? `${nameOf(pendingDelete)} will be permanently removed.`
            : undefined
        }
        confirmLabel="Delete"
        cancelLabel="Cancel"
        danger
        confirmDisabled={busy}
        onCancel={() => setPendingDelete(null)}
        onConfirm={async () => {
          if (!pendingDelete) return;
          setBusy(true);
          try {
            await api.deleteAttachment(String(pendingDelete.id));
            showSnackbar("Attachment deleted");
            setPendingDelete(null);
            onChanged();
          } catch (err) {
            showSnackbar(friendlyError(err) || "Delete failed", "error");
          } finally {
            setBusy(false);
          }
        }}
      />

      <ConfirmDialog
        open={Boolean(edit)}
        title="Edit attachment"
        confirmLabel="Save"
        confirmDisabled={busy || !edit?.filename.trim()}
        onCancel={() => setEdit(null)}
        onConfirm={async () => {
          if (!edit) return;
          setBusy(true);
          try {
            await api.updateAttachment(String(edit.file.id), {
              filename: edit.filename.trim(),
              description: edit.description,
            });
            showSnackbar("Attachment updated");
            setEdit(null);
            onChanged();
          } catch (err) {
            showSnackbar(friendlyError(err) || "Update failed", "error");
          } finally {
            setBusy(false);
          }
        }}
      >
        {edit ? (
          <div className="stack">
            <label>
              Filename
              <input
                value={edit.filename}
                onChange={(e) => setEdit({ ...edit, filename: e.target.value })}
              />
            </label>
            <label>
              Description
              <input
                value={edit.description}
                onChange={(e) => setEdit({ ...edit, description: e.target.value })}
              />
            </label>
          </div>
        ) : null}
      </ConfirmDialog>

      {preview ? (
        <div className="palette-backdrop" onClick={closePreview} role="presentation">
          <div
            className="dialog file-preview"
            role="dialog"
            aria-modal="true"
            aria-labelledby="file-preview-title"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 id="file-preview-title">{nameOf(preview.file)}</h3>
            {preview.kind === "image" && preview.url ? (
              <img alt={nameOf(preview.file)} src={preview.url} className="preview-image" />
            ) : null}
            {preview.kind === "pdf" && preview.url ? (
              <iframe title={nameOf(preview.file)} src={preview.url} className="preview-frame" />
            ) : null}
            {preview.kind === "text" ? <pre className="preview-text">{preview.text}</pre> : null}
            {preview.kind === "unavailable" ? (
              <p className="muted">Preview unavailable</p>
            ) : null}
            <div className="dialog-actions">
              <Button variant="text" onClick={closePreview}>
                Close
              </Button>
              <Button onClick={() => void download(preview.file)}>Download</Button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
