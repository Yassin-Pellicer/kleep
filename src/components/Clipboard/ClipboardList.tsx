import { useEffect, useRef, useState, type RefObject } from "react";
import type { ClipboardEntry } from "../../types";
import { CheckIcon } from "../icons/CheckIcon";
import { CopyIcon } from "../icons/CopyIcon";
import { formatTime } from "../../lib/formatTime";
import "./ClipboardList.css";

type ClipboardListProps = {
  entries: ClipboardEntry[];
  listRef: RefObject<HTMLUListElement | null>;
  selectedIndex: number;
  onActivate: (entry: ClipboardEntry) => void;
  onCopy: (entry: ClipboardEntry) => Promise<void>;
  onDelete: (entry: ClipboardEntry) => void;
};

export function ClipboardList({
  entries,
  listRef,
  selectedIndex,
  onActivate,
  onCopy,
  onDelete,
}: ClipboardListProps) {
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const copiedTimer = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (copiedTimer.current !== null) {
        window.clearTimeout(copiedTimer.current);
      }
    },
    [],
  );

  const copy = async (entry: ClipboardEntry) => {
    try {
      await onCopy(entry);
    } catch {
      return;
    }

    setCopiedId(entry.id);
    if (copiedTimer.current !== null) {
      window.clearTimeout(copiedTimer.current);
    }
    copiedTimer.current = window.setTimeout(() => setCopiedId(null), 1200);
  };

  if (entries.length === 0) {
    return (
      <div className="clipboard-list clipboard-list--empty">
        Clipboard history is empty
      </div>
    );
  }

  return (
    <ul
      className="clipboard-list"
      role="listbox"
      aria-label="Clipboard history"
      ref={listRef}
    >
      {entries.map((entry, index) => (
        <li
          key={entry.id}
          data-index={index}
          role="option"
          aria-selected={index === selectedIndex}
          className={`clipboard-list__item${
            index === selectedIndex ? " clipboard-list__item--selected" : ""
          }`}
          title={entry.text}
          onClick={() => onActivate(entry)}
        >
          <span className="clipboard-list__text">
            {entry.text.replace(/\s+/g, " ").trim()}
          </span>
          <span className="clipboard-list__time">
            {formatTime(entry.timestamp)}
          </span>
          <span className="clipboard-list__actions">
            <button
              type="button"
              className="clipboard-list__copy"
              aria-label={
                copiedId === entry.id
                  ? "Copied to clipboard"
                  : "Copy to clipboard"
              }
              title={copiedId === entry.id ? "Copied" : "Copy"}
              onMouseDown={(event) => event.preventDefault()}
              onClick={(event) => {
                event.stopPropagation();
                void copy(entry);
              }}
            >
              {copiedId === entry.id ? <CheckIcon /> : <CopyIcon />}
            </button>
            <button
              type="button"
              className="clipboard-list__delete"
              aria-label="Delete clipboard entry"
              title="Delete"
              onMouseDown={(event) => event.preventDefault()}
              onClick={(event) => {
                event.stopPropagation();
                onDelete(entry);
              }}
            >
              ×
            </button>
          </span>
        </li>
      ))}
    </ul>
  );
}
