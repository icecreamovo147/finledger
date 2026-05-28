<template>
  <router-view />
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { ElMessageBox } from "element-plus";
import { useRouter } from "vue-router";
import { useThemeStore } from "@/stores/theme";

const router = useRouter();
const themeStore = useThemeStore();

onMounted(async () => {
  // Initialize theme store (triggers DOM class application via watcher)
  themeStore.resolvedTheme;

  try {
    const integrityError = await invoke<string | null>("check_db_integrity");
    if (integrityError) {
      try {
        await ElMessageBox.confirm(
          `${integrityError}。建议立即从备份恢复数据，或前往设置页执行数据恢复。`,
          "数据库完整性警告",
          {
            confirmButtonText: "前往设置页恢复",
            cancelButtonText: "暂时忽略",
            type: "warning",
          }
        );
        router.push("/settings");
      } catch {
        // User clicked "暂时忽略"
      }
    }
  } catch {
    // integrity check command failed, don't block app
  }
});
</script>
