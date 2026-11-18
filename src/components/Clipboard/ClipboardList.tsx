import type { RefObject } from "react";
import type { ClipboardEntry } from "../../types";
import "./ClipboardList.css";

type ClipboardListProps = {
  entries: ClipboardEntry[];
  listRef: RefObject<HTMLUListElement | null>;
  selectedIndex: number;
  onSelectedIndexChange: (index: number) => void;
  onActivate: (entry: ClipboardEntry) => void;
  onDelete: (entry: ClipboardEntry) => void;
};

function formatTime(timestamp: number): string {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return "";

  const now = new Date();
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  return date.toLocaleDateString([], {
    month: "short",
    day: "numeric",
  });
}

export function ClipboardList({
  entries,
  listRef,
  selectedIndex,
  onSelectedIndexChange,
  onActivate,
  onDelete,
}: ClipboardListProps) {
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
          onMouseEnter={() => onSelectedIndexChange(index)}
          onClick={() => onActivate(entry)}
        >
          <span className="clipboard-list__text">
            {entry.text.replace(/\s+/g, " ").trim()}
          </span>
          <span className="clipboard-list__time">
            {formatTime(entry.timestamp)}
          </span>
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
        </li>
      ))}
    </ul>
  );
}
