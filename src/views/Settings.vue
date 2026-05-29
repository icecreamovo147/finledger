<template>
  <div class="settings">
    <!-- Data Backup -->
    <div class="setting-section accent-primary">
      <div class="section-header">
        <h3>数据备份</h3>
        <p class="section-desc">包含账本、记录、结清状态和附件图片，建议定期备份</p>
      </div>
      <div class="backup-actions">
        <el-button type="primary" :loading="backingUp" @click="handleBackup">
          <el-icon><Download /></el-icon>备份数据
        </el-button>
        <el-button @click="router.push('/backup-management')">
          <el-icon><Files /></el-icon>备份管理
        </el-button>
      </div>
    </div>

    <!-- Data Restore -->
    <div class="setting-section accent-danger">
      <div class="section-header">
        <h3>数据恢复</h3>
        <p class="section-desc">从备份文件恢复数据，当前数据将被覆盖</p>
      </div>
      <el-popconfirm
        title="恢复备份将会覆盖当前所有数据，确定继续？"
        :width="260"
        popper-class="restore-popconfirm"
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

    <!-- Appearance -->
    <div class="setting-section accent-success">
      <div class="section-header">
        <h3>外观设置</h3>
        <p class="section-desc">选择应用的主题模式</p>
      </div>
      <el-radio-group
        :model-value="themeStore.mode"
        class="theme-mode-group"
        @update:model-value="themeStore.setMode($event)"
      >
        <el-radio-button value="light">
          <el-icon style="margin-right: 4px; vertical-align: middle;"><Sunny /></el-icon>
          <span style="vertical-align: middle;">亮色</span>
        </el-radio-button>
        <el-radio-button value="dark">
          <el-icon style="margin-right: 4px; vertical-align: middle;"><Moon /></el-icon>
          <span style="vertical-align: middle;">暗色</span>
        </el-radio-button>
        <el-radio-button value="auto">
          <el-icon style="margin-right: 4px; vertical-align: middle;"><Monitor /></el-icon>
          <span style="vertical-align: middle;">跟随系统</span>
        </el-radio-button>
      </el-radio-group>
      <p v-if="themeStore.mode === 'auto'" class="system-hint">
        当前系统偏好：{{ themeStore.systemDark ? '暗色' : '亮色' }}
      </p>
    </div>

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
import { useRouter } from "vue-router";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Download, Upload, Sunny, Moon, Monitor, Files } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { safeInvoke } from "@/utils/invoke";
import { useThemeStore } from "@/stores/theme";

const router = useRouter();
const themeStore = useThemeStore();
const backingUp = ref(false);
const restoring = ref(false);

async function handleBackup() {
  try {
    const dir = await openDialog({
      title: "选择备份目录",
      directory: true,
      multiple: false,
      recursive: false,
      canCreateDirectories: true,
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
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.setting-section {
  position: relative;
  min-height: 154px;
  padding: 22px;
  overflow: hidden;
  border: 1px solid var(--border-color);
  border-radius: 14px;
  background: var(--card-bg);
  box-shadow: var(--card-shadow);

  &::before {
    position: absolute;
    top: 0;
    left: 0;
    width: 4px;
    height: 100%;
    content: "";
    background: var(--border-hover);
  }

  &.accent-primary::before {
    background: var(--color-primary);
  }

  &.accent-danger::before {
    background: var(--color-danger);
  }

  &.accent-success::before {
    background: var(--color-success);
  }

  .section-header {
    margin-bottom: 16px;

    h3 {
      color: var(--text-heading);
      font-size: 18px;
      margin-bottom: 4px;
    }

    .section-desc {
      font-size: 14px;
      color: var(--text-tertiary);
    }
  }
}

.about-info {
  font-size: 14px;
  color: var(--text-secondary);
  line-height: 1.8;
}

.backup-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.system-hint {
  margin-top: 8px;
  font-size: 14px;
  color: var(--text-tertiary);
}

.theme-mode-group {
  display: inline-flex;
  gap: 8px;

  :deep(.el-radio-button__inner) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    min-width: 108px;
    height: 40px;
    padding: 0 18px;
    color: var(--text-secondary);
    font-size: 15px;
    font-weight: 700;
    line-height: 1;
    border: 0;
    border-radius: 8px;
    background: transparent;
    box-shadow: none;
    transition: background-color 180ms ease, color 180ms ease, box-shadow 180ms ease;
  }

  :deep(.el-icon),
  :deep(span) {
    vertical-align: initial !important;
  }

  :deep(.el-radio-button:first-child .el-radio-button__inner),
  :deep(.el-radio-button:last-child .el-radio-button__inner) {
    border-radius: 8px;
  }

  :deep(.el-radio-button__original-radio:checked + .el-radio-button__inner) {
    color: #ffffff;
    background: var(--color-primary);
    box-shadow: 0 10px 24px rgba(37, 99, 235, 0.24);
  }
}

:global(.restore-popconfirm .el-popconfirm__action) {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  white-space: nowrap;
}

:global(.restore-popconfirm .el-popconfirm__action .el-button) {
  flex: 0 0 auto;
  margin-left: 0;
}

@media (max-width: 1100px) {
  .settings {
    grid-template-columns: 1fr;
  }
}
</style>
