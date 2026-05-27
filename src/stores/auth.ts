import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface User {
  id: number;
  username: string;
  created_at: string;
}

export const useAuthStore = defineStore("auth", () => {
  const user = ref<User | null>(null);
  const token = ref<string | null>(null);

  const isLoggedIn = computed(() => !!user.value && !!token.value);

  async function login(username: string, password: string, remember: boolean) {
    const result = await invoke<{ user: User; token: string }>("login", {
      username,
      password,
      remember,
    });
    user.value = result.user;
    token.value = result.token;
    localStorage.setItem("auth_token", result.token);
  }

  async function tryAutoLogin(): Promise<boolean> {
    const saved = localStorage.getItem("auth_token");
    if (!saved) return false;
    try {
      const u = await invoke<User>("validate_session", { token: saved });
      user.value = u;
      token.value = saved;
      return true;
    } catch {
      localStorage.removeItem("auth_token");
      return false;
    }
  }

  async function logout() {
    if (token.value) {
      try {
        await invoke("logout", { token: token.value });
      } catch { /* ignore */ }
    }
    user.value = null;
    token.value = null;
    localStorage.removeItem("auth_token");
  }

  return { user, token, isLoggedIn, login, tryAutoLogin, logout };
});
