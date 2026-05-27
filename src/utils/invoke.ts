import { invoke } from "@tauri-apps/api/core";
import { useAuthStore } from "@/stores/auth";

export async function safeInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const authStore = useAuthStore();
  const token = authStore.token;
  if (!token) {
    throw new Error("会话无效或已过期，请重新登录");
  }
  return invoke<T>(cmd, { ...args, token });
}
