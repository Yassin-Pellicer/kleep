import { AppList } from "../AppList/AppList";
import { Chat } from "../Chat/Chat";
import { Dictionary } from "../Dictionary/Dictionary";
import { VoiceButton } from "../Voice/VoiceButton";
import { usePalette } from "./usePalette";
import "./Palette.css";

function SearchIcon() {
  return (
    <svg
      className="palette__icon"
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.5-3.5" />
    </svg>
  );
}

export function Palette() {
  const {
    inputRef,
    query,
    setQuery,
    isOpen,
    showCount,
    isExpanded,
    hasApps,
    apps,
    listRef,
    selectedIndex,
    setSelectedIndex,
    launchError,
    launch,
    handleKeyDown,
    dismiss,
    dictionaryQuery,
    dictionaryEnabled,
    assistant,
    chatActive,
    exitChat,
    voice,
    handleVoiceClick,
  } = usePalette();

  return (
    <div className="palette-overlay" onClick={dismiss}>
      <div
        className={`palette${isOpen ? " palette--open" : ""}${
          isExpanded ? " palette--expanded" : ""
        }`}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="palette__bar">
          <SearchIcon />
          <input
            ref={inputRef}
            className="palette__input"
            type="text"
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
            onKeyDown={handleKeyDown}
            placeholder={chatActive ? "Ask anything…" : "Type to search…"}
            autoFocus
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
          />
          <VoiceButton
            status={voice.status}
            onClick={() => {
              void handleVoiceClick();
            }}
            disabled={assistant.status === "loading"}
          />
          <kbd className="palette__hint">↵</kbd>
        </div>
        <div className="palette__results">
          <div className="palette__results-inner" aria-live="polite">
            {voice.error && (
              <p className="palette__voice-error" role="alert">
                {voice.error}
              </p>
            )}
            {launchError && (
              <p className="palette__launch-error" role="alert">
                {launchError}
              </p>
            )}
            {hasApps ? (
              <AppList
                apps={apps}
                listRef={listRef}
                selectedIndex={selectedIndex}
                onSelectedIndexChange={setSelectedIndex}
                onLaunch={launch}
              />
            ) : chatActive ? (
              <Chat
                messages={assistant.messages}
                status={assistant.status}
                error={assistant.error}
                onExit={exitChat}
              />
            ) : dictionaryEnabled ? (
              <Dictionary
                query={dictionaryQuery}
                enabled={dictionaryEnabled}
                resetKey={showCount}
              />
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}
