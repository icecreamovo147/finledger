import { defineStore } from "pinia";
import { ref, computed, watch, onScopeDispose } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";

export type ThemeMode = "light" | "dark" | "auto";

const STORAGE_KEY = "theme-mode";

export const useThemeStore = defineStore("theme", () => {
  const mode = ref<ThemeMode>(
    (localStorage.getItem(STORAGE_KEY) as ThemeMode) || "auto"
  );

  const systemDark = ref(
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );

  const resolvedTheme = computed(() =>
    mode.value === "auto"
      ? systemDark.value
        ? "dark"
        : "light"
      : mode.value
  );

  function applyTheme() {
    const html = document.documentElement;
    if (resolvedTheme.value === "dark") {
      html.classList.add("dark");
    } else {
      html.classList.remove("dark");
    }
    html.style.colorScheme = resolvedTheme.value;
  }

  async function applyWindowTheme() {
    if (!isTauri()) {
      return;
    }

    try {
      const currentWindow = getCurrentWindow();
      await currentWindow.setTheme(mode.value === "auto" ? null : resolvedTheme.value);

      if (mode.value === "auto") {
        const windowTheme = await currentWindow.theme();
        if (windowTheme) {
          systemDark.value = windowTheme === "dark";
        }
      }
    } catch (error) {
      console.warn("Failed to update native window theme", error);
    }
  }

  function setMode(newMode: ThemeMode) {
    mode.value = newMode;
  }

  function cycleMode() {
    const next: Record<ThemeMode, ThemeMode> = {
      light: "dark",
      dark: "auto",
      auto: "light",
    };
    setMode(next[mode.value]);
  }

  // Persist mode changes
  watch(mode, (val) => {
    localStorage.setItem(STORAGE_KEY, val);
  });

  // Apply theme when the effective theme or follow-system mode changes.
  watch([resolvedTheme, mode], () => {
    // Add transition class for smooth switching
    document.documentElement.classList.add("theme-transition");
    applyTheme();
    void applyWindowTheme();
    setTimeout(() => {
      document.documentElement.classList.remove("theme-transition");
    }, 300);
  }, { immediate: true });

  // Listen for system theme changes
  const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  function onSystemThemeChange(e: MediaQueryListEvent) {
    systemDark.value = e.matches;
  }
  mediaQuery.addEventListener("change", onSystemThemeChange);

  let unlistenTauriTheme: UnlistenFn | null = null;
  if (isTauri()) {
    void getCurrentWindow()
      .onThemeChanged(({ payload }) => {
        if (mode.value === "auto") {
          systemDark.value = payload === "dark";
        }
      })
      .then((unlisten) => {
        unlistenTauriTheme = unlisten;
      })
      .catch((error) => {
        console.warn("Failed to listen for native theme changes", error);
      });
  }

  onScopeDispose(() => {
    mediaQuery.removeEventListener("change", onSystemThemeChange);
    unlistenTauriTheme?.();
  });

  return { mode, systemDark, resolvedTheme, setMode, cycleMode };
});
