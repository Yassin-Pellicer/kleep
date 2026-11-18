import { useEffect, useRef, useState } from "react";
import type { AppEntry } from "../../types";

export function useAppList(apps: AppEntry[]) {
  const listRef = useRef<HTMLUListElement>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => {
    setSelectedIndex(0);
  }, [apps]);

  useEffect(() => {
    const list = listRef.current;
    if (!list) return;

    const selected = list.querySelector<HTMLElement>(
      `[data-index="${selectedIndex}"]`,
    );
    if (!selected) return;

    // Reveal the whole row plus a little breathing room, so arrow navigation
    // never stops with an item flush against the edge.
    const margin = 8;
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
    setSelectedIndex((index) =>
      apps.length === 0 ? 0 : (index + delta + apps.length) % apps.length,
    );
  };

  const selectedApp =
    apps.length === 0
      ? undefined
      : apps[Math.min(selectedIndex, apps.length - 1)];

  return {
    listRef,
    selectedIndex,
    setSelectedIndex,
    moveSelection,
    selectedApp,
  };
}
