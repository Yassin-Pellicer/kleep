import type { RefObject } from "react";
import type { AppEntry } from "../../types";
import "./AppList.css";

type AppListProps = {
  apps: AppEntry[];
  listRef: RefObject<HTMLUListElement | null>;
  selectedIndex: number;
  onLaunch: (app: AppEntry) => void;
};

export function AppList({
  apps,
  listRef,
  selectedIndex,
  onLaunch,
}: AppListProps) {
  if (apps.length === 0) return null;

  return (
    <ul
      className="app-list"
      role="listbox"
      aria-label="Applications"
      ref={listRef}
    >
      {apps.map((app, index) => (
        <li
          key={app.id}
          data-index={index}
          role="option"
          aria-selected={index === selectedIndex}
          className={`app-list__item${
            index === selectedIndex ? " app-list__item--selected" : ""
          }`}
          onClick={() => onLaunch(app)}
        >
          <span className="app-list__icon">
            <span className="app-list__icon-fallback" aria-hidden="true">
              {app.name.charAt(0).toUpperCase()}
            </span>
            {app.icon && (
              <img
                src={app.icon}
                alt=""
                draggable={false}
                onError={(event) => event.currentTarget.remove()}
              />
            )}
          </span>
          <span className="app-list__text">
            <span className="app-list__name">{app.name}</span>
            {app.description && (
              <span className="app-list__description">{app.description}</span>
            )}
          </span>
        </li>
      ))}
    </ul>
  );
}
