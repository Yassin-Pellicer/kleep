import { AppList } from "../AppList/AppList";
import { Chat } from "../Chat/Chat";
import { ClipboardList } from "../Clipboard/ClipboardList";
import { Dictionary } from "../Dictionary/Dictionary";
import { VoiceButton } from "../Voice/VoiceButton";
import { VoiceWaveform } from "../Voice/VoiceWaveform";
import { SearchIcon } from "../icons/SearchIcon";
import { usePalette } from "./usePalette";
import "./Palette.css";

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
    clipboard,
    clipboardActive,
    clipboardListRef,
    clipboardSelectedIndex,
    pasteClipboardEntry,
    copyClipboardEntry,
    deleteClipboardEntry,
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
          <div className="palette__field">
            <input
              ref={inputRef}
              className={`palette__input${
                voice.status === "recording"
                  ? " palette__input--recording"
                  : ""
              }`}
              type="text"
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              onKeyDown={handleKeyDown}
              placeholder={
                voice.status === "recording" ? "" : "Type to search…"
              }
              autoFocus
              autoComplete="off"
              autoCorrect="off"
              spellCheck={false}
            />
            {voice.status === "recording" && query.length === 0 && (
              <VoiceWaveform level={voice.level} />
            )}
          </div>
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
            {clipboardActive ? (
              <ClipboardList
                entries={clipboard.entries}
                listRef={clipboardListRef}
                selectedIndex={clipboardSelectedIndex}
                onActivate={pasteClipboardEntry}
                onCopy={copyClipboardEntry}
                onDelete={deleteClipboardEntry}
              />
            ) : hasApps ? (
              <AppList
                apps={apps}
                listRef={listRef}
                selectedIndex={selectedIndex}
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
