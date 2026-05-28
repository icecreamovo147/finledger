import { defineStore } from "pinia";
import { ref, computed, watch, onScopeDispose } from "vue";

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

  // Apply theme when resolvedTheme changes
  watch(resolvedTheme, () => {
    // Add transition class for smooth switching
    document.documentElement.classList.add("theme-transition");
    applyTheme();
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

  onScopeDispose(() => {
    mediaQuery.removeEventListener("change", onSystemThemeChange);
  });

  return { mode, systemDark, resolvedTheme, setMode, cycleMode };
});
