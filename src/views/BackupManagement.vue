<template>
  <div class="backup-management">
    <div class="backup-overview-strip">
      <div class="strip-item">
        <span>备份总数</span>
        <strong>{{ overview?.total_count ?? 0 }}</strong>
      </div>
      <div class="strip-item">
        <span>自动备份</span>
        <strong>{{ overview?.auto_count ?? 0 }}</strong>
      </div>
      <div class="strip-item">
        <span>手动备份</span>
        <strong>{{ overview?.manual_count ?? 0 }}</strong>
      </div>
      <div class="strip-item">
        <span>总大小</span>
        <strong>{{ formatSize(overview?.total_size_bytes ?? 0) }}</strong>
      </div>
    </div>

    <!-- 自动备份配置区 -->
    <div class="setting-section">
      <div class="section-header">
        <h3>自动备份配置</h3>
        <p class="section-desc">自动备份会在应用运行时按计划执行，关闭应用后不会执行</p>
      </div>

      <el-form label-width="100px" class="backup-form">
        <el-form-item label="自动备份">
          <el-switch v-model="form.enabled" />
        </el-form-item>

        <el-form-item label="备份目录">
          <div class="dir-row">
            <el-input
              :model-value="form.target_dir || '未选择'"
              readonly
              placeholder="请选择备份目录"
            />
            <el-button @click="handleSelectDir">选择目录</el-button>
          </div>
        </el-form-item>

        <el-form-item label="备份频率">
          <el-select v-model="form.frequency" style="width: 200px" @change="onFrequencyChange">
            <el-option label="每隔 N 分钟" value="interval_minutes" />
            <el-option label="每隔 N 小时" value="interval_hours" />
            <el-option label="每天" value="daily" />
            <el-option label="每周" value="weekly" />
            <el-option label="每月" value="monthly" />
          </el-select>
        </el-form-item>

        <el-form-item v-if="form.frequency === 'interval_minutes'" label="间隔分钟">
          <el-input-number
            v-model="intervalValue"
            :min="1"
            :max="10080"
          />
          <span class="form-hint">每隔多少分钟执行一次</span>
        </el-form-item>

        <el-form-item v-if="form.frequency === 'interval_hours'" label="间隔小时">
          <el-input-number
            v-model="intervalHours"
            :min="1"
            :max="168"
          />
          <span class="form-hint">每隔多少小时执行一次</span>
        </el-form-item>

        <el-form-item v-if="form.frequency === 'weekly'" label="执行日期">
          <el-select v-model="form.day_of_week" style="width: 200px">
            <el-option
              v-for="d in weekdayOptions"
              :key="d.value"
              :label="d.label"
              :value="d.value"
            />
          </el-select>
        </el-form-item>

        <el-form-item v-if="form.frequency === 'monthly'" label="执行日期">
          <el-select v-model="form.day_of_month" style="width: 200px">
            <el-option
              v-for="d in 28"
              :key="d"
              :label="`每月 ${d} 日`"
              :value="d"
            />
          </el-select>
          <span class="form-hint">为保证每月都能执行，最多可选 28 日</span>
        </el-form-item>

        <el-form-item v-if="!isInterval" label="执行时间">
          <el-time-picker
            v-model="timeValue"
            format="HH:mm"
            value-format="HH:mm"
            placeholder="选择时间"
            style="width: 200px"
          />
        </el-form-item>

        <el-form-item label="保留份数">
          <el-input-number
            v-model="form.retention_count"
            :min="1"
            :max="999"
          />
          <span class="form-hint">保留最近 N 份自动备份</span>
        </el-form-item>

        <el-form-item>
          <el-button type="primary" :loading="saving" @click="handleSave">
            保存设置
          </el-button>
          <el-button :loading="backingUp" @click="handleBackupNow">
            立即备份
          </el-button>
        </el-form-item>
      </el-form>
    </div>

    <!-- 备份概览区 -->
    <div class="setting-section">
      <div class="section-header section-header-row">
        <h3>备份概览</h3>
        <el-button :loading="refreshing" @click="handleRefresh">
          刷新
        </el-button>
      </div>

      <el-descriptions :column="2" border class="overview-desc">
        <el-descriptions-item label="自动备份状态">
          <el-tag :type="overview?.settings.enabled ? 'success' : 'info'">
            {{ overview?.settings.enabled ? '已开启' : '已关闭' }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="备份目录">
          {{ overview?.settings.target_dir || '未配置' }}
        </el-descriptions-item>
        <el-descriptions-item label="下次备份时间">
          <span v-if="isFirstAutoRun">即将执行（首次）</span>
          <span v-else>{{ overview?.next_backup_at || '-' }}</span>
        </el-descriptions-item>
        <el-descriptions-item label="最近备份时间">
          {{ overview?.latest_backup_at || '-' }}
        </el-descriptions-item>
        <el-descriptions-item label="最近备份结果">
          <el-tag
            v-if="overview?.last_run_state.last_status"
            :type="statusTagType(overview.last_run_state.last_status)"
          >
            {{ statusLabel(overview.last_run_state.last_status) }}
          </el-tag>
          <span v-else>-</span>
        </el-descriptions-item>
        <el-descriptions-item label="最近备份消息">
          {{ overview?.last_run_state.last_message || '-' }}
        </el-descriptions-item>
        <el-descriptions-item label="备份总数">
          {{ overview?.total_count ?? 0 }}
        </el-descriptions-item>
        <el-descriptions-item label="备份总大小">
          {{ formatSize(overview?.total_size_bytes ?? 0) }}
        </el-descriptions-item>
        <el-descriptions-item label="自动备份">
          {{ overview?.auto_count ?? 0 }} 份
        </el-descriptions-item>
        <el-descriptions-item label="手动备份">
          {{ overview?.manual_count ?? 0 }} 份
        </el-descriptions-item>
        <el-descriptions-item label="最早备份">
          {{ overview?.oldest_backup_at || '-' }}
        </el-descriptions-item>
        <el-descriptions-item label="最新备份">
          {{ overview?.latest_backup_at || '-' }}
        </el-descriptions-item>
      </el-descriptions>
    </div>

    <!-- 备份历史区 -->
    <div class="setting-section">
      <div class="section-header section-header-row">
        <h3>备份历史</h3>
        <el-button :loading="refreshing" @click="handleRefresh">
          刷新
        </el-button>
      </div>

      <el-table :data="overview?.backups ?? []" stripe style="width: 100%">
        <el-table-column prop="file_name" label="文件名" min-width="200" show-overflow-tooltip />
        <el-table-column label="类型" width="100">
          <template #default="{ row }">
            <el-tag
              :type="row.backup_type === 'auto' ? 'primary' : row.backup_type === 'manual' ? 'success' : 'info'"
              size="small"
            >
              {{ row.backup_type === 'auto' ? '自动' : row.backup_type === 'manual' ? '手动' : '未知' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="created_at" label="创建时间" width="180" />
        <el-table-column label="大小" width="100">
          <template #default="{ row }">
            {{ formatSize(row.size_bytes) }}
          </template>
        </el-table-column>
        <el-table-column label="校验状态" width="100">
          <template #default="{ row }">
            <el-tag :type="row.is_valid ? 'success' : 'danger'" size="small">
              {{ row.is_valid ? '正常' : '异常' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="160" fixed="right">
          <template #default="{ row }">
            <el-popconfirm
              title="恢复备份将会覆盖当前所有数据，确定继续？"
              :width="260"
              confirm-button-text="确定恢复"
              cancel-button-text="取消"
              :disabled="!row.is_valid"
              @confirm="handleRestore(row)"
            >
              <template #reference>
                <el-button type="primary" size="small" :disabled="!row.is_valid">
                  恢复
                </el-button>
              </template>
            </el-popconfirm>
            <el-popconfirm
              :title="row.backup_type === 'manual' ? '手动备份不会被系统自动淘汰，确定删除？' : '确定删除此备份？'"
              :width="280"
              confirm-button-text="确定删除"
              cancel-button-text="取消"
              @confirm="handleDelete(row)"
            >
              <template #reference>
                <el-button type="danger" size="small">删除</el-button>
              </template>
            </el-popconfirm>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <p class="footer-hint">
      自动备份会在应用运行时按计划执行。关闭应用后不会执行自动备份，请定期手动备份重要数据。
    </p>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { safeInvoke } from "@/utils/invoke";
import type { BackupOverview, BackupFileInfo } from "@/types";

const overview = ref<BackupOverview | null>(null);
const saving = ref(false);
const backingUp = ref(false);
const refreshing = ref(false);

const weekdayOptions = [
  { value: 1, label: "每周一" },
  { value: 2, label: "每周二" },
  { value: 3, label: "每周三" },
  { value: 4, label: "每周四" },
  { value: 5, label: "每周五" },
  { value: 6, label: "每周六" },
  { value: 7, label: "每周日" },
];

const form = ref({
  enabled: false,
  target_dir: null as string | null,
  frequency: "daily",
  time_of_day: "23:00",
  day_of_week: null as number | null,
  day_of_month: null as number | null,
  interval_minutes: null as number | null,
  retention_mode: "count",
  retention_count: 10,
  retention_days: 30,
  retention_size_mb: 2048,
});

const timeValue = computed({
  get: () => form.value.time_of_day,
  set: (val: string) => {
    form.value.time_of_day = val;
  },
});

const isInterval = computed(() =>
  form.value.frequency === "interval_minutes" || form.value.frequency === "interval_hours"
);

const isFirstAutoRun = computed(() => {
  if (!overview.value?.settings.enabled) return false;
  const freq = overview.value?.settings.frequency;
  if (freq !== "interval_minutes" && freq !== "interval_hours") return false;
  return !overview.value?.last_run_state.last_auto_run_at;
});

const intervalValue = computed({
  get: () => form.value.interval_minutes ?? 60,
  set: (val: number) => {
    form.value.interval_minutes = val;
  },
});

const intervalHours = computed({
  get: () => Math.floor((form.value.interval_minutes ?? 360) / 60),
  set: (val: number) => {
    form.value.interval_minutes = val * 60;
  },
});

function onFrequencyChange(freq: string) {
  if (freq === "interval_minutes" && !form.value.interval_minutes) {
    form.value.interval_minutes = 60;
  } else if (freq === "interval_hours" && !form.value.interval_minutes) {
    form.value.interval_minutes = 360;
  } else if (freq === "weekly" && !form.value.day_of_week) {
    form.value.day_of_week = 1;
  } else if (freq === "monthly" && !form.value.day_of_month) {
    form.value.day_of_month = 1;
  }
}

async function loadOverview() {
  try {
    overview.value = await safeInvoke<BackupOverview>("get_backup_overview");
    // Sync form with loaded settings
    form.value = { ...overview.value.settings };
  } catch (e: any) {
    ElMessage.error(e || "加载备份概览失败");
  }
}

async function handleRefresh() {
  try {
    refreshing.value = true;
    await loadOverview();
    ElMessage.success("备份列表已刷新");
  } finally {
    refreshing.value = false;
  }
}

async function handleSelectDir() {
  const dir = await openDialog({
    title: "选择备份目录",
    directory: true,
    multiple: false,
    canCreateDirectories: true,
  });
  if (dir) {
    form.value.target_dir = dir as string;
  }
}

async function handleSave() {
  try {
    saving.value = true;
    await safeInvoke("update_backup_settings", { settings: { ...form.value } });
    ElMessage.success("备份设置已保存");
    await loadOverview();
  } catch (e: any) {
    ElMessage.error(e || "保存失败");
  } finally {
    saving.value = false;
  }
}

async function handleBackupNow() {
  try {
    backingUp.value = true;
    const path = await safeInvoke<string>("run_backup_now", { backupType: "manual" });
    ElMessage.success(`备份成功: ${path}`);
    await loadOverview();
  } catch (e: any) {
    ElMessage.error(e || "备份失败");
  } finally {
    backingUp.value = false;
  }
}

async function handleRestore(row: BackupFileInfo) {
  try {
    const msg = await safeInvoke<string>("restore_backup", { backupPath: row.path });
    ElMessage.success(msg || "恢复成功");
  } catch (e: any) {
    ElMessage.error(e || "恢复失败");
  }
}

async function handleDelete(row: BackupFileInfo) {
  try {
    await safeInvoke("delete_backup_file", { path: row.path });
    ElMessage.success("删除成功");
    await loadOverview();
  } catch (e: any) {
    ElMessage.error(e || "删除失败");
  }
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0) + " " + units[i];
}

function statusTagType(status: string): string {
  switch (status) {
    case "success":
      return "success";
    case "failed":
      return "danger";
    case "skipped":
      return "warning";
    default:
      return "info";
  }
}

function statusLabel(status: string): string {
  switch (status) {
    case "success":
      return "成功";
    case "failed":
      return "失败";
    case "skipped":
      return "跳过";
    default:
      return status;
  }
}

onMounted(loadOverview);
</script>

<style scoped lang="scss">
.backup-management {
  display: grid;
  grid-template-columns: minmax(380px, 0.86fr) minmax(0, 1.14fr);
  gap: 16px;
}

.backup-overview-strip {
  display: grid;
  grid-column: 1 / -1;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
}

.strip-item {
  padding: 18px;
  border: 1px solid var(--border-color);
  border-radius: 14px;
  background: var(--card-bg);
  box-shadow: var(--card-shadow);

  span,
  strong {
    display: block;
  }

  span {
    margin-bottom: 8px;
    color: var(--text-tertiary);
    font-size: 12px;
  }

  strong {
    color: var(--text-heading);
    font-size: 22px;
  }
}

.setting-section {
  padding: 22px;
  border: 1px solid var(--border-color);
  border-radius: 14px;
  background: var(--card-bg);
  box-shadow: var(--card-shadow);

  &:last-of-type {
    grid-column: 1 / -1;
  }

  .section-header {
    margin-bottom: 16px;

    h3 {
      color: var(--text-heading);
      font-size: 18px;
      margin-bottom: 4px;
    }

    .section-desc {
      font-size: 13px;
      color: var(--text-tertiary);
    }
  }
}

.section-header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.backup-form {
  max-width: none;
}

.dir-row {
  display: flex;
  gap: 8px;
  width: 100%;
}

.form-hint {
  margin-left: 12px;
  font-size: 13px;
  color: var(--text-tertiary);
}

.overview-desc {
  max-width: none;
}

.footer-hint {
  grid-column: 1 / -1;
  margin-top: 0;
  font-size: 13px;
  color: var(--text-tertiary);
}

@media (max-width: 1200px) {
  .backup-management,
  .backup-overview-strip {
    grid-template-columns: 1fr;
  }

  .backup-overview-strip,
  .setting-section:last-of-type,
  .footer-hint {
    grid-column: auto;
  }
}
</style>
