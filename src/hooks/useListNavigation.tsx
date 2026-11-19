import { useEffect, useRef, useState } from "react";

/**
 * Keyboard selection plus scroll-into-view for a listbox.
 *
 * The margin reveals the whole row plus a little breathing room, so arrow
 * navigation never stops with an item flush against the edge.
 */
export function useListNavigation<T>(items: T[], margin = 8) {
  const listRef = useRef<HTMLUListElement>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => {
    setSelectedIndex(0);
  }, [items]);

  useEffect(() => {
    const list = listRef.current;
    if (!list) return;

    const selected = list.querySelector<HTMLElement>(
      `[data-index="${selectedIndex}"]`,
    );
    if (!selected) return;

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
  }, [selectedIndex, margin]);

  const moveSelection = (delta: number) => {
    setSelectedIndex((index) => {
      if (items.length === 0) return 0;
      return Math.min(Math.max(index + delta, 0), items.length - 1);
    });
  };

  const selected =
    items.length === 0
      ? undefined
      : items[Math.min(selectedIndex, items.length - 1)];

  return {
    listRef,
    selectedIndex,
    setSelectedIndex,
    moveSelection,
    selected,
  };
}
