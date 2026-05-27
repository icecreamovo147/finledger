<template>
  <router-view />
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { ElMessageBox, ElMessage } from "element-plus";
import { useRouter } from "vue-router";

const router = useRouter();

onMounted(async () => {
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
