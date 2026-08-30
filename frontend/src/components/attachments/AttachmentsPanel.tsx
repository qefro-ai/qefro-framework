import { useRef, useState } from "react";
import { api, tokenHeader } from "../../api";
import { fileIcon, fileSize } from "../../format";
import { ActionMenu } from "../ui/ActionMenu";

type FileRow = Record<string, unknown>;

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
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [drag, setDrag] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  async function upload(files: FileList | File[]) {
    setError("");
    const list = Array.from(files);
    for (const file of list) {
      try {
        setProgress(0);
        await api.uploadAttachment(slug, id, file, setProgress);
        setProgress(1);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Upload failed.");
      }
    }
    setProgress(null);
    onChanged();
  }

  async function download(file: FileRow) {
    const res = await fetch(`/api/v1/attachments/${file.id}`, { headers: tokenHeader() });
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = String(file.filename ?? "file");
    a.click();
    URL.revokeObjectURL(url);
  }

  const mime = (file: FileRow) => String(file.content_type ?? file.mime_type ?? file.mime ?? "");
  const name = (file: FileRow) => String(file.filename ?? file.id);
  const isImage = (file: FileRow) => mime(file).startsWith("image/") || /\.(png|jpe?g|gif|webp)$/i.test(name(file));

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
          if (e.dataTransfer.files.length) void upload(e.dataTransfer.files);
        }}
      >
        <p className="muted">Drop files here or</p>
        <button type="button" className="ghost" onClick={() => inputRef.current?.click()}>
          + Add attachment
        </button>
        <input
          ref={inputRef}
          type="file"
          multiple
          className="sr-only"
          onChange={(e) => {
            if (e.target.files?.length) void upload(e.target.files);
            e.target.value = "";
          }}
        />
      </div>
      {progress != null ? <progress value={progress} max={1} aria-label="Upload progress" /> : null}
      {error ? (
        <p className="error" role="alert">
          {error}
        </p>
      ) : null}
      {items.length === 0 ? (
        <p className="empty">No files attached.</p>
      ) : (
        <ul className="attachment-list">
          {items.map((file) => (
            <li key={String(file.id)}>
              {isImage(file) ? (
                <img className="thumb" alt="" src={`/api/v1/attachments/${file.id}`} />
              ) : (
                <span className="file-glyph" aria-hidden="true">
                  {fileIcon(name(file), mime(file))}
                </span>
              )}
              <button type="button" className="ghost linkish" onClick={() => void download(file)}>
                {name(file)}
              </button>
              <span className="muted attachment-size">{fileSize(file.size)}</span>
              <ActionMenu
                items={[
                  { key: "download", label: "Download", onSelect: () => void download(file) },
                  {
                    key: "remove",
                    label: "Remove",
                    danger: true,
                    onSelect: async () => {
                      await api.deleteAttachment(String(file.id));
                      onChanged();
                    },
                  },
                ]}
              />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
