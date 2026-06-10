<template>
  <div class="book-detail">
    <!-- Header -->
    <div class="page-header">
      <div class="header-left">
        <el-button class="back-btn" text @click="router.push('/books')">
          <el-icon><ArrowLeft /></el-icon>
        </el-button>
        <span class="current-book-name">{{ bookName || "当前账本" }}</span>
      </div>
      <div class="header-right">
        <el-button
          type="success"
          :disabled="selectedIds.length === 0"
          @click="batchSettle"
        >
          批量结清 ({{ selectedIds.length }})
        </el-button>
        <el-button
          type="warning"
          :disabled="selectedUnsettled.length === 0"
          @click="exportSelected"
        >
          导出账单
        </el-button>
        <el-button
          type="warning"
          plain
          @click="exportAllUnsettled"
        >
          导出全部未结清
        </el-button>
        <el-button type="primary" @click="openCreate">
          <el-icon><Plus /></el-icon>新增记录
        </el-button>
      </div>
    </div>

    <!-- Filter Bar -->
    <div class="filter-bar">
      <el-select
        v-model="filters.settlement_status"
        placeholder="结算状态"
        clearable
        style="width: 120px"
        @change="handleFilterChange"
      >
        <el-option label="未结清" value="unsettled" />
        <el-option label="已结清" value="settled" />
      </el-select>
      <el-date-picker
        v-model="filters.dateRange"
        type="daterange"
        range-separator="至"
        start-placeholder="开始日期"
        end-placeholder="结束日期"
        format="YYYY-MM-DD"
        value-format="YYYY-MM-DD"
        @change="handleFilterChange"
      />
      <el-input
        v-model="filters.keyword"
        placeholder="搜索服务项目/备注"
        clearable
        style="width: 200px"
        @change="handleFilterChange"
        @clear="handleFilterChange"
      />
    </div>

    <!-- Table -->
    <div class="table-container">
    <el-table
      ref="tableRef"
      :data="records"
      border
      stripe
      v-loading="loading"
      @selection-change="onSelectionChange"
    >
      <el-table-column type="selection" width="50" />
      <el-table-column prop="date" label="日期" width="110" sortable />
      <el-table-column prop="service_content" label="服务项目及内容" min-width="180" show-overflow-tooltip />
      <el-table-column prop="specification" label="规格" width="120" show-overflow-tooltip />
      <el-table-column label="数量" width="100" align="center">
        <template #default="{ row }">
          {{ row.quantity != null ? row.quantity + (row.unit ? ' ' + row.unit : '') : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="unit_price" label="单价" width="90" align="right">
        <template #default="{ row }">
          {{ row.unit_price != null ? '¥' + (row.unit_price / 100).toLocaleString('zh-CN', { minimumFractionDigits: 2 }) : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="total_amount" label="总金额" width="120" align="right" sortable>
        <template #default="{ row }">
          <span style="font-weight: 600; color: var(--text-heading)">
            ¥{{ (row.total_amount / 100).toLocaleString('zh-CN', { minimumFractionDigits: 2 }) }}
          </span>
        </template>
      </el-table-column>
      <el-table-column prop="settlement_status" label="状态" width="90" align="center">
        <template #default="{ row }">
          <el-tag :type="row.settlement_status === 'settled' ? 'success' : 'warning'" size="small">
            {{ row.settlement_status === 'settled' ? '已结清' : '未结清' }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="remark" label="备注" min-width="120" show-overflow-tooltip />
      <el-table-column label="图片" width="100" align="center">
        <template #default="{ row }">
          <div v-if="row.images?.length" class="table-images">
            <template v-for="(img, idx) in row.images" :key="img.id">
              <img
                v-if="!isImageMissing(img.id)"
                :src="getImageUrl(img.id)"
                style="width: 32px; height: 32px; border-radius: 3px; object-fit: cover; cursor: pointer; margin-right: 2px"
                @mouseenter="loadImageSrc(img.id)"
                @click="previewImages(row.images, idx)"
              />
              <span v-else class="img-missing-tag" title="文件不存在">缺失</span>
            </template>
          </div>
          <span v-else style="color: #c0c4cc">-</span>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="220" fixed="right">
        <template #default="{ row }">
          <template v-if="row.settlement_status === 'settled'">
            <el-button text size="small" type="info" @click="viewDetail(row)">查看</el-button>
            <el-button text size="small" @click="handleUnsettle(row)">回退</el-button>
          </template>
          <template v-else>
            <el-button text size="small" type="primary" @click="openEdit(row)">编辑</el-button>
            <el-button text size="small" type="success" @click="openSettle(row)">结清</el-button>
            <el-popconfirm title="确定删除？" @confirm="handleDelete(row.id)">
              <template #reference>
                <el-button text size="small" type="danger">删除</el-button>
              </template>
            </el-popconfirm>
          </template>
        </template>
      </el-table-column>
    </el-table>

    <!-- Pagination -->
    <div class="pagination-bar">
      <span class="unsettled-total">
        账本未结清总额：<strong>¥{{ (bookTotalUnsettled / 100).toLocaleString('zh-CN', { minimumFractionDigits: 2 }) }}</strong>
        <template v-if="hasActiveFilters">
          ｜ 筛选未结清：<strong>¥{{ (totalUnsettled / 100).toLocaleString('zh-CN', { minimumFractionDigits: 2 }) }}</strong>
        </template>
      </span>
      <el-pagination
        v-model:current-page="currentPage"
        v-model:page-size="pageSize"
        :page-sizes="[10, 20, 50, 100]"
        :total="total"
        layout="total, sizes, prev, pager, next, jumper"
        background
        @current-change="handlePageChange"
        @size-change="handleSizeChange"
      />
    </div>
    </div>

    <!-- Record Form Dialog (T13) -->
    <el-dialog
      v-model="showFormDialog"
      :title="isEditing ? '编辑记录' : '新增记录'"
      width="720px"
      destroy-on-close
      @paste="onPaste"
      @dragover.prevent="onDialogDragOver"
      @drop.prevent="onDrop"
    >
      <el-form
        ref="formRef"
        :model="form"
        :rules="formRules"
        label-width="130px"
        @submit.prevent
      >
        <el-form-item label="日期" prop="date">
          <el-date-picker
            v-model="form.date"
            type="date"
            placeholder="选择日期"
            format="YYYY-MM-DD"
            value-format="YYYY-MM-DD"
            style="width: 100%"
          />
        </el-form-item>

        <el-form-item label="服务项目及内容" prop="service_content">
          <el-input
            v-model="form.service_content"
            type="textarea"
            :autosize="{ minRows: 3, maxRows: 6 }"
            placeholder="请输入服务项目及内容"
          />
        </el-form-item>

        <el-form-item label="规格" prop="specification">
          <el-input
            v-model="form.specification"
            type="textarea"
            :autosize="{ minRows: 3, maxRows: 6 }"
            placeholder="请输入规格"
          />
        </el-form-item>

        <el-form-item label="数量" prop="quantity">
          <el-input-number v-model="form.quantity" :min="0" :precision="0" style="width: 160px" />
          <el-select
            v-model="form.unit"
            prop="unit"
            style="width: 120px; margin-left: 8px"
            allow-create
            filterable
            placeholder="单位"
          >
            <el-option
              v-for="u in unitOptions"
              :key="u"
              :label="u"
              :value="u"
            />
          </el-select>
        </el-form-item>

        <el-form-item label="单价" prop="unit_price">
          <el-input-number v-model="form.unit_price" :min="0" :precision="2" style="width: 200px">
            <template #suffix>元</template>
          </el-input-number>
        </el-form-item>

        <el-form-item label="总金额" prop="total_amount">
          <el-input-number
            v-model="form.total_amount"
            :min="0"
            :precision="2"
            style="width: 200px"
            :disabled="form.quantity != null && form.unit_price != null"
          >
            <template #suffix>元</template>
          </el-input-number>
        </el-form-item>

        <el-form-item label="备注">
          <el-input v-model="form.remark" type="textarea" :autosize="{ minRows: 3, maxRows: 6 }" placeholder="可选备注" />
        </el-form-item>

        <!-- Image section -->
        <el-form-item label="图片">
          <div
            class="image-upload"
            :class="{ 'drag-over': isDragOver }"
            @dragover.prevent="onDialogDragOver"
            @dragleave="onDragLeave"
            @drop.prevent="onDrop"
          >
            <div
              v-for="(img, idx) in existingImages"
              :key="img.id"
              class="image-preview"
            >
              <img
                v-if="!isImageMissing(img.id)"
                :src="getImageUrl(img.id)"
                style="width: 80px; height: 80px; border-radius: 4px; object-fit: cover; cursor: pointer"
                @click="previewImages(existingImages, idx)"
              />
              <div v-else class="img-missing-placeholder" style="width: 80px; height: 80px; margin-right: 0">
                <span style="font-size: 11px">文件不存在</span>
              </div>
              <span
                class="image-remove"
                title="移除图片"
                @click="removeExistingImage(img.id)"
              ><el-icon><Delete /></el-icon></span>
            </div>
            <div
              v-for="(img, idx) in newImages"
              :key="img.tempId"
              class="image-preview"
            >
              <img
                :src="img.previewUrl"
                style="width: 80px; height: 80px; object-fit: cover; border-radius: 4px; cursor: pointer"
                @click="previewNewImagesByIndex(idx)"
              />
              <span
                class="image-remove"
                title="移除图片"
                @click="removeNewImage(idx)"
              ><el-icon><Delete /></el-icon></span>
            </div>
            <el-upload
              :auto-upload="false"
              :show-file-list="false"
              accept="image/*"
              @change="onFileSelect"
            >
              <div class="upload-trigger">
                <el-icon><Plus /></el-icon>
              </div>
            </el-upload>
          </div>
          <div class="upload-hint">支持拖拽图片、粘贴图片（Ctrl+V）</div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showFormDialog = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="handleSave">
          确定
        </el-button>
      </template>
    </el-dialog>

    <!-- Settlement Dialog (T15) -->
    <el-dialog v-model="showSettleDialog" title="标记结清" width="460px">
      <el-form
        ref="settleFormRef"
        :model="settleForm"
        :rules="settleRules"
        label-width="90px"
      >
        <el-form-item label="收款日期" prop="payment_date">
          <el-date-picker
            v-model="settleForm.payment_date"
            type="date"
            placeholder="选择收款日期"
            format="YYYY-MM-DD"
            value-format="YYYY-MM-DD"
            style="width: 100%"
          />
        </el-form-item>
        <el-form-item label="收款方式" prop="payment_method">
          <el-select
            v-model="settleForm.payment_method"
            style="width: 100%"
            allow-create
            filterable
            placeholder="选择或输入收款方式"
          >
            <el-option
              v-for="m in paymentMethods"
              :key="m"
              :label="m"
              :value="m"
            />
          </el-select>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showSettleDialog = false">取消</el-button>
        <el-button type="primary" :loading="settling" @click="handleSettle">
          确认结清
        </el-button>
      </template>
    </el-dialog>

    <!-- Image viewer for record detail -->
    <el-dialog v-model="showDetailDialog" title="记录详情" width="700px">
      <div v-if="detailRecord" class="record-detail">
        <el-descriptions :column="2" border>
          <el-descriptions-item label="日期">{{ detailRecord.date }}</el-descriptions-item>
          <el-descriptions-item label="服务项目及内容">{{ detailRecord.service_content || '-' }}</el-descriptions-item>
          <el-descriptions-item label="规格">{{ detailRecord.specification || '-' }}</el-descriptions-item>
          <el-descriptions-item label="总金额">¥{{ (detailRecord.total_amount / 100).toLocaleString('zh-CN', { minimumFractionDigits: 2 }) }}</el-descriptions-item>
          <el-descriptions-item label="数量">{{ detailRecord.quantity ?? '-' }}</el-descriptions-item>
          <el-descriptions-item label="单价">{{ detailRecord.unit_price != null ? '¥' + (detailRecord.unit_price / 100).toLocaleString('zh-CN', { minimumFractionDigits: 2 }) : '-' }}</el-descriptions-item>
          <el-descriptions-item label="状态">
            <el-tag :type="detailRecord.settlement_status === 'settled' ? 'success' : 'warning'" size="small">
              {{ detailRecord.settlement_status === 'settled' ? '已结清' : '未结清' }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item v-if="detailRecord.payment_date" label="收款日期">{{ detailRecord.payment_date }}</el-descriptions-item>
          <el-descriptions-item v-if="detailRecord.payment_method" label="收款方式">{{ detailRecord.payment_method }}</el-descriptions-item>
          <el-descriptions-item label="备注" :span="2">{{ detailRecord.remark || '-' }}</el-descriptions-item>
        </el-descriptions>
        <div v-if="detailRecord.images?.length" class="detail-images">
          <h4>图片凭证 ({{ detailRecord.images.length }} 张)</h4>
          <div class="image-list">
            <template v-for="(img, idx) in detailRecord.images" :key="img.id">
              <img
                v-if="!isImageMissing(img.id)"
                :src="getImageUrl(img.id)"
                style="width: 120px; height: 120px; border-radius: 6px; object-fit: cover; cursor: pointer; margin-right: 8px"
                @click="previewImages(detailRecord.images, idx)"
              />
              <div v-else class="img-missing-placeholder">
                <span>文件不存在</span>
              </div>
            </template>
          </div>
        </div>
      </div>
    </el-dialog>

    <!-- Image Preview Dialog -->
    <el-dialog v-model="showPreview" title="图片预览" width="80vw" destroy-on-close>
      <div v-if="previewImagesList.length" class="preview-container">
        <div class="preview-main">
          <img
            v-if="!isImageMissing(previewImagesList[previewIndex].id)"
            :src="getImageUrl(previewImagesList[previewIndex].id)"
            class="preview-big"
          />
          <div v-else class="img-missing-placeholder large">
            <span>文件不存在</span>
          </div>
        </div>
        <div class="preview-thumbs" v-if="previewImagesList.length > 1">
          <template v-for="(img, idx) in previewImagesList" :key="img.id">
            <img
              v-if="!isImageMissing(img.id)"
              :src="getImageUrl(img.id)"
              :class="{ active: idx === previewIndex }"
              @click="previewIndex = idx"
              @mouseenter="loadImageSrc(img.id)"
            />
            <div
              v-else
              class="thumb-missing"
              :class="{ active: idx === previewIndex }"
              @click="previewIndex = idx"
            >
              缺失
            </div>
          </template>
        </div>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { useRoute, useRouter } from "vue-router";
import { save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { Plus, Delete, ArrowLeft } from "@element-plus/icons-vue";
import { PaymentMethods } from "@/types";
import { useRecords } from "@/composables/useRecords";
import { useRecordForm } from "@/composables/useRecordForm";
import { useSettlement } from "@/composables/useSettlement";

const route = useRoute();
const router = useRouter();
const bookId = Number(route.params.id);

// ---- Data & Filtering ----
const rec = useRecords(bookId);
const {
  bookName, records, selectedIds, loading, total, totalUnsettled,
  bookTotalUnsettled, currentPage, pageSize, filters,
  selectedUnsettled, hasActiveFilters,
  getImageUrl, isImageMissing, loadImageSrc,
  fetchBookName, fetchRecords, handlePageChange, handleSizeChange,
  handleFilterChange, onSelectionChange,
} = rec;

// Wrap fetchRecords to show errors (composable throws, component handles UI)
async function safeFetchRecords() {
  try { await fetchRecords(); } catch (e: any) { ElMessage.error(e || "加载失败"); }
}

// ---- Record Form ----
const formCtx = useRecordForm(bookId, {
  loadImageSrc,
  refreshRecords: safeFetchRecords,
});
const {
  showFormDialog, isEditing, saving, formRef, formRules, form,
  existingImages, newImages, isDragOver,
  openCreate, openEdit,
  addImagePath,
  onFileSelect, onDialogDragOver, onDragLeave, onDrop, onPaste,
  removeNewImage, removeExistingImage,
  handleSave, handleDelete,
} = formCtx;

// ---- Settlement & Export ----
const stl = useSettlement(bookId, bookName, {
  records,
  selectedIds,
  selectedUnsettled,
  imageSrcMap: rec.imageSrcMap,
  refreshRecords: safeFetchRecords,
});
const {
  showSettleDialog, settling, settleFormRef, settleForm, settleRules,
  openSettle, handleSettle, batchSettle, handleUnsettle,
  showPreview, previewImagesList, previewIndex,
  previewImages, previewNewImages,
  showDetailDialog, detailRecord, viewDetail,
} = stl;

// ---- Constants ----
const unitOptions = ["份", "张", "块", "个", "本", "套", "卷", "米", "平方米", "次", "项"];
const paymentMethods = PaymentMethods;
const tableRef = ref();

// ---- Export wrappers (pass Tauri save dialog) ----
async function exportSelected() {
  await stl.exportSelected(save);
}
async function exportAllUnsettled() {
  await stl.exportAllUnsettled(save);
}

// ---- Tauri drag-drop (uses form composable internals) ----
let unlistenDragDrop: UnlistenFn | undefined;
let lastDropTime = 0;

onMounted(async () => {
  await registerTauriDragDrop();
  await fetchBookName();
  await safeFetchRecords();
});

onUnmounted(() => {
  unlistenDragDrop?.();
});

async function registerTauriDragDrop() {
  try {
    unlistenDragDrop = await getCurrentWindow().onDragDropEvent(async ({ payload }) => {
      if (!showFormDialog.value) return;
      const now = Date.now();
      if (now - lastDropTime < 500) return;
      lastDropTime = now;
      if (payload.type === "enter" || payload.type === "over") {
        isDragOver.value = true;
        return;
      }
      if (payload.type === "leave") {
        isDragOver.value = false;
        return;
      }
      isDragOver.value = false;
      for (const path of payload.paths) {
        await addImagePath(path);
      }
    });
  } catch {
    // Browser-only dev mode
  }
}

// ---- Preview wrapper for new images ----
function previewNewImagesByIndex(idx: number) {
  previewNewImages(newImages.value.map(img => ({
    tempId: img.tempId,
    originalName: img.originalName,
    previewUrl: img.previewUrl,
  })), idx);
}
</script>

<style scoped lang="scss">
.book-detail {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;

  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 14px;
    padding: 16px;
    border: 1px solid var(--border-color);
    border-radius: 14px;
    background: var(--card-bg);
    box-shadow: var(--card-shadow);

    .header-left {
      display: flex;
      align-items: center;
      min-width: 0;
      gap: 10px;

      .back-btn {
        flex-shrink: 0;
        width: 34px;
        height: 34px;
        border-radius: 8px;
        background: var(--card-bg-subtle);
      }

      .current-book-name {
        min-width: 0;
        color: var(--text-heading);
        font-size: 18px;
        font-weight: 650;
        line-height: 1.4;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
    }

    .header-right {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
      justify-content: flex-end;
    }
  }

  .filter-bar {
    display: flex;
    gap: 12px;
    margin-bottom: 14px;
    padding: 16px;
    flex-wrap: wrap;
    border: 1px solid var(--border-color);
    border-radius: 14px;
    background: var(--card-bg);
    box-shadow: var(--card-shadow);
  }

  .table-container {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    padding: 12px;
    border: 1px solid var(--border-color);
    border-radius: 14px;
    background: var(--card-bg);
    box-shadow: var(--card-shadow);
    overflow: hidden;

    .el-table {
      flex: 1;
      min-height: 0;
      border-radius: 0;
    }
  }

  .pagination-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 0 0;

    .unsettled-total {
      font-size: 14px;
      color: var(--text-secondary);

      strong {
        color: var(--color-danger);
        font-size: 16px;
      }
    }
  }
}

.image-upload {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 8px;
  border: 2px dashed transparent;
  border-radius: 8px;
  min-height: 96px;
  transition: border-color 160ms ease, background-color 160ms ease;

  &.drag-over {
    border-color: var(--color-primary);
    background: var(--hover-bg);
  }

  .image-preview {
    position: relative;

    .image-remove {
      position: absolute;
      top: -8px;
      right: -8px;
      width: 20px;
      height: 20px;
      display: flex;
      align-items: center;
      justify-content: center;
      background: var(--color-danger);
      color: #fff;
      border-radius: 50%;
      cursor: pointer;
      font-size: 12px;
      box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
      transition: transform 120ms ease;

      &:hover {
        transform: scale(1.15);
      }
    }
  }

  .upload-trigger {
    width: 80px;
    height: 80px;
    border: 1px dashed var(--input-border);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: var(--text-tertiary);
    font-size: 20px;
    transition: border-color 160ms ease, color 160ms ease, background-color 160ms ease;

    &:hover {
      background: var(--hover-bg);
      border-color: var(--color-primary);
      color: var(--color-primary);
    }
  }
}

.upload-hint {
  margin-top: 6px;
  font-size: 12px;
  color: var(--text-tertiary);
  line-height: 1.4;
}

.img-missing-tag {
  display: inline-block;
  width: 32px;
  height: 32px;
  line-height: 32px;
  text-align: center;
  font-size: 10px;
  color: var(--color-danger);
  background: var(--hover-bg);
  border-radius: 3px;
  margin-right: 2px;
  cursor: default;
}

.img-missing-placeholder {
  width: 120px;
  height: 120px;
  border-radius: 6px;
  background: var(--hover-bg);
  border: 1px dashed var(--border-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 8px;
  span {
    font-size: 12px;
    color: var(--color-danger);
  }
  &.large {
    width: 100%;
    max-width: 400px;
    height: 300px;
    margin: 0 auto;
    span { font-size: 16px; }
  }
}

.thumb-missing {
  width: 56px;
  height: 56px;
  border-radius: 4px;
  background: var(--hover-bg);
  border: 2px solid transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  color: var(--color-danger);
  cursor: pointer;
  transition: border-color 160ms ease;
  &.active {
    border-color: var(--color-primary);
  }
  &:hover {
    border-color: var(--color-primary);
  }
}

.detail-images {
  margin-top: 20px;

  h4 { margin-bottom: 8px; font-size: 14px; }

  .image-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
}

.preview-container {
  text-align: center;

  .preview-main {
    margin-bottom: 16px;
    .preview-big {
      max-width: 100%;
      max-height: 60vh;
      object-fit: contain;
      border-radius: 6px;
    }
  }

  .preview-thumbs {
    display: flex;
    justify-content: center;
    gap: 8px;
    flex-wrap: wrap;

    img {
      width: 56px;
      height: 56px;
      object-fit: cover;
      border-radius: 4px;
      cursor: pointer;
      border: 2px solid transparent;
      transition: border-color 160ms ease, transform 160ms ease;

      &.active {
        border-color: var(--color-primary);
      }

      &:hover {
        border-color: var(--color-primary);
        transform: translateY(-1px);
      }
    }
  }
}
</style>
