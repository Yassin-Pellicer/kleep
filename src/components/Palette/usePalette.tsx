import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { launchApp } from "../AppList/apps";
import { useAppList } from "../AppList/useAppList";
import { useAppSearch } from "../AppList/useAppSearch";
import { useAssistant } from "../Chat/useAssistant";
import { useClipboard } from "../Clipboard/useClipboard";
import { useClipboardList } from "../Clipboard/useClipboardList";
import { useVoice } from "../Voice/useVoice";
import type { AppEntry, ClipboardEntry, VoiceTranscript } from "../../types";
import { parseDictionaryInput } from "./parseDictionaryInput";

const RESET_DELAY = 160;

export function usePalette() {
  const inputRef = useRef<HTMLInputElement>(null);
  const resetTimer = useRef<number | null>(null);
  const [query, setQuery] = useState("");
  const [isOpen, setIsOpen] = useState(false);
  const [showCount, setShowCount] = useState(0);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const handledTranscript = useRef<VoiceTranscript | null>(null);

  const trimmedQuery = query.trim();
  const dictionaryTerm = parseDictionaryInput(trimmedQuery);
  const isSingleWord =
    dictionaryTerm !== null ||
    (trimmedQuery.length > 0 && !/\s/.test(trimmedQuery));

  const { apps, resolvedQuery } = useAppSearch(query);
  const {
    listRef,
    selectedIndex,
    setSelectedIndex,
    moveSelection,
    selectedApp,
  } = useAppList(apps);

  const assistant = useAssistant(showCount);
  const voice = useVoice();
  const clipboard = useClipboard(
    showCount,
    isOpen && trimmedQuery.length === 0,
  );
  const {
    listRef: clipboardListRef,
    selectedIndex: clipboardSelectedIndex,
    setSelectedIndex: setClipboardSelectedIndex,
    moveSelection: moveClipboardSelection,
    selected: clipboardSelected,
  } = useClipboardList(clipboard.entries);
  const appsMatch =
    trimmedQuery.length > 0 &&
    resolvedQuery === trimmedQuery &&
    apps.length > 0;
  const appSearchPending =
    trimmedQuery.length > 0 && resolvedQuery !== trimmedQuery;
  const isChatPrompt = trimmedQuery.length > 0 && !isSingleWord;
  const hasChat =
    assistant.messages.length > 0 ||
    assistant.status === "loading" ||
    assistant.error !== null;

  // While chatting, app search and dictionary are suppressed. A multi-word
  // draft only becomes chat mode when no app matches it.
  const draftChat = !hasChat && !appsMatch && isChatPrompt;
  const chatActive = hasChat || draftChat;
  const hasApps = !chatActive && appsMatch;
  const dictionaryEnabled = isSingleWord && !chatActive && !hasApps;
  // Hide clipboard history as soon as a chat begins, including voice chats
  // (recording/transcribing) that haven't produced a message yet.
  const voiceBusy = voice.status !== "idle";
  const clipboardActive =
    isOpen &&
    trimmedQuery.length === 0 &&
    clipboard.loaded &&
    !chatActive &&
    !voiceBusy;
  const isExpanded =
    hasApps ||
    dictionaryEnabled ||
    chatActive ||
    clipboardActive ||
    voiceBusy ||
    Boolean(voice.error);
  const dictionaryQuery = dictionaryEnabled
    ? (dictionaryTerm ?? trimmedQuery)
    : "";

  const dismiss = () => {
    void voice.cancel();
    void voice.stopSpeaking();
    voice.clearAutoTranscript();
    void invoke("hide_palette");
  };

  const exitChat = () => {
    assistant.clear();
    setQuery("");
    setLaunchError(null);
    void voice.cancel();
    void voice.stopSpeaking();
    voice.clearAutoTranscript();
    requestAnimationFrame(() => inputRef.current?.focus());
  };

  const submitTranscript = async (transcript: VoiceTranscript) => {
    const text = transcript.text.trim();
    if (!text) {
      voice.setError("I didn't catch that. Try again.");
      requestAnimationFrame(() => inputRef.current?.focus());
      return;
    }

    setQuery("");
    const reply = await assistant.send(text, { voice: true });
    requestAnimationFrame(() => inputRef.current?.focus());
    if (reply) {
      await voice.speak(reply.content, transcript.language);
    }
  };

  const handleVoiceClick = async () => {
    if (voice.status === "recording") {
      const transcript = await voice.stop();
      if (!transcript) return;
      await submitTranscript(transcript);
      return;
    }

    if (voice.status === "idle") {
      await voice.start();
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  };

  const launch = (app: AppEntry) => {
    void launchApp(app.id)
      .then(() => dismiss())
      .catch((error: unknown) => {
        setLaunchError(
          typeof error === "string" ? error : "Couldn't open the app.",
        );
      });
  };

  const pasteClipboardEntry = (entry: ClipboardEntry) => {
    void clipboard.paste(entry.id).catch((error: unknown) => {
      setLaunchError(
        typeof error === "string"
          ? error
          : "Couldn't paste into the active input.",
      );
    });
  };

  const deleteClipboardEntry = (entry: ClipboardEntry) => {
    void clipboard.remove(entry.id).catch((error: unknown) => {
      setLaunchError(
        typeof error === "string" ? error : "Couldn't delete the entry.",
      );
    });
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (
      clipboardActive &&
      (event.key === "ArrowDown" || event.key === "ArrowUp")
    ) {
      event.preventDefault();
      moveClipboardSelection(event.key === "ArrowDown" ? 1 : -1);
      return;
    }

    if (hasApps && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
      event.preventDefault();
      moveSelection(event.key === "ArrowDown" ? 1 : -1);
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      if (chatActive) {
        exitChat();
      } else {
        dismiss();
      }
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();

      if (clipboardActive) {
        if (clipboardSelected) {
          pasteClipboardEntry(clipboardSelected);
        } else {
          dismiss();
        }
        return;
      }

      if (voice.status === "recording") {
        void handleVoiceClick();
        return;
      }

      if (chatActive) {
        if (assistant.status === "loading") {
          return;
        }

        if (trimmedQuery.length > 0) {
          void voice.stopSpeaking();
          void assistant.send(trimmedQuery);
          setQuery("");
        }
        return;
      }

      // Wait for the app search to settle before acting on a single word so
      // Enter cannot dismiss the palette right before an app match appears.
      if (isSingleWord && appSearchPending) {
        return;
      }

      if (hasApps && selectedApp) {
        launch(selectedApp);
        return;
      }

      dismiss();
    }
  };

  useEffect(() => {
    setLaunchError(null);
    voice.setError(null);
  }, [trimmedQuery]);

  useEffect(() => {
    const cancelReset = () => {
      if (resetTimer.current !== null) {
        window.clearTimeout(resetTimer.current);
        resetTimer.current = null;
      }
    };

    const focusInput = () => {
      inputRef.current?.focus();
      inputRef.current?.select();
    };

    const unlisteners = [
      listen("palette-shown", () => {
        cancelReset();
        setQuery("");
        setShowCount((count) => count + 1);
        setIsOpen(true);
        voice.setError(null);
        voice.clearAutoTranscript();
        void voice.cancel();
        requestAnimationFrame(focusInput);
      }),
      listen("palette-hide", () => {
        setIsOpen(false);
        void voice.cancel();
        void voice.stopSpeaking();
        voice.clearAutoTranscript();
        cancelReset();
        resetTimer.current = window.setTimeout(() => {
          setQuery("");
          resetTimer.current = null;
        }, RESET_DELAY);
      }),
    ];

    // Handles cold starts, where the window is already visible before the
    // palette-shown event can be received.
    void getCurrentWindow().isVisible().then((visible) => {
      if (visible) setIsOpen(true);
    });

    window.addEventListener("focus", focusInput);

    return () => {
      window.removeEventListener("focus", focusInput);
      cancelReset();
      for (const unlisten of unlisteners) {
        void unlisten.then((stop) => stop());
      }
    };
  }, []);

  // A recording that stopped itself because the speaker paused arrives here.
  useEffect(() => {
    const transcript = voice.autoTranscript;
    if (!transcript || handledTranscript.current === transcript) return;

    handledTranscript.current = transcript;
    voice.clearAutoTranscript();
    void submitTranscript(transcript);
  }, [voice.autoTranscript]);

  return {
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
    clipboard,
    clipboardActive,
    clipboardListRef,
    clipboardSelectedIndex,
    setClipboardSelectedIndex,
    pasteClipboardEntry,
    deleteClipboardEntry,
  };
}
