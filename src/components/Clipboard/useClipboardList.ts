import { useEffect, useRef, useState } from "react";
import type { ClipboardEntry } from "../../types";

export function useClipboardList(entries: ClipboardEntry[]) {
  const listRef = useRef<HTMLUListElement>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => {
    setSelectedIndex(0);
  }, [entries]);

  useEffect(() => {
    const list = listRef.current;
    if (!list) return;

    const selected = list.querySelector<HTMLElement>(
      `[data-index="${selectedIndex}"]`,
    );
    if (!selected) return;

    const margin = 12;
    const listRect = list.getBoundingClientRect();
    const itemRect = selected.getBoundingClientRect();
    const itemTop = itemRect.top - listRect.top + list.scrollTop;
    const itemBottom = itemTop + itemRect.height;
    const viewTop = list.scrollTop;
    const viewBottom = viewTop + list.clientHeight;

    if (itemTop - margin < viewTop) {
      list.scrollTo({ top: itemTop - margin, behavior: "auto" });
    } else if (itemBottom + margin > viewBottom) {
      list.scrollTo({
        top: itemBottom + margin - list.clientHeight,
        behavior: "auto",
      });
    }
  }, [selectedIndex]);

  const moveSelection = (delta: number) => {
    setSelectedIndex((index) => {
      if (entries.length === 0) return 0;
      return Math.min(Math.max(index + delta, 0), entries.length - 1);
    });
  };

  const selected =
    entries.length === 0
      ? undefined
      : entries[Math.min(selectedIndex, entries.length - 1)];

  return {
    listRef,
    selectedIndex,
    setSelectedIndex,
    moveSelection,
    selected,
  };
}
