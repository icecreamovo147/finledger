<template>
  <div class="settings">
    <h2>设置</h2>

    <!-- Data Backup -->
    <div class="setting-section">
      <div class="section-header">
        <h3>数据备份</h3>
        <p class="section-desc">包含账本、记录、结清状态和附件图片，建议定期备份</p>
      </div>
      <el-button type="primary" :loading="backingUp" @click="handleBackup">
        <el-icon><Download /></el-icon>备份数据
      </el-button>
    </div>

    <el-divider />

    <!-- Data Restore -->
    <div class="setting-section">
      <div class="section-header">
        <h3>数据恢复</h3>
        <p class="section-desc">从备份文件恢复数据，当前数据将被覆盖</p>
      </div>
      <el-popconfirm
        title="恢复备份将会覆盖当前所有数据，确定继续？"
        confirm-button-text="确定恢复"
        cancel-button-text="取消"
        @confirm="handleRestore"
      >
        <template #reference>
          <el-button type="danger" :loading="restoring">
            <el-icon><Upload /></el-icon>恢复数据
          </el-button>
        </template>
      </el-popconfirm>
    </div>

    <el-divider />

    <!-- About -->
    <div class="setting-section">
      <div class="section-header">
        <h3>关于</h3>
      </div>
      <p class="about-info">FinLedger v0.1.0 — 广告公司记账软件</p>
      <p class="about-info">技术栈：Tauri 2 + Vue 3 + Rust + SQLite</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Download, Upload } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { safeInvoke } from "@/utils/invoke";

const backingUp = ref(false);
const restoring = ref(false);

async function handleBackup() {
  try {
    const dir = await openDialog({
      title: "选择备份目录",
      directory: true,
    });
    if (!dir) return;

    backingUp.value = true;
    const path = await safeInvoke<string>("backup_database", {
      targetDir: dir as string,
    });
    ElMessage.success(`备份成功: ${path}`);
  } catch (e: any) {
    ElMessage.error(e || "备份失败");
  } finally {
    backingUp.value = false;
  }
}

async function handleRestore() {
  try {
    const file = await openDialog({
      title: "选择备份文件",
      filters: [
        { name: "FinLedger 备份", extensions: ["flbackup"] },
        { name: "旧版数据库备份", extensions: ["db"] },
      ],
    });
    if (!file) return;

    restoring.value = true;
    const msg = await safeInvoke<string>("restore_database", {
      backupPath: file as string,
    });
    ElMessage.success(msg || "数据恢复成功");
  } catch (e: any) {
    ElMessage.error(e || "恢复失败");
  } finally {
    restoring.value = false;
  }
}
</script>

<style scoped lang="scss">
.settings {
  h2 {
    font-size: 20px;
    margin-bottom: 24px;
  }
}

.setting-section {
  margin-bottom: 20px;

  .section-header {
    margin-bottom: 16px;

    h3 {
      font-size: 16px;
      margin-bottom: 4px;
    }

    .section-desc {
      font-size: 13px;
      color: #909399;
    }
  }
}

.about-info {
  font-size: 14px;
  color: #606266;
  line-height: 1.8;
}
</style>
