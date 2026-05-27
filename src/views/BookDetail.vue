<template>
  <div class="book-detail">
    <!-- Header -->
    <div class="page-header">
      <div class="header-left">
        <el-button text @click="router.push('/books')">
          <el-icon><ArrowLeft /></el-icon>
        </el-button>
        <h2>{{ bookName }}</h2>
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
        v-model="filters.category"
        placeholder="收入类别"
        clearable
        style="width: 140px"
        @change="handleFilterChange"
      >
        <el-option
          v-for="(label, key) in IncomeCategoryLabels"
          :key="key"
          :label="label"
          :value="key"
        />
      </el-select>
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
        placeholder="搜索描述/备注"
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
      <el-table-column prop="category" label="类别" width="130">
        <template #default="{ row }">
          {{ IncomeCategoryLabels[row.category as IncomeCategory] || row.category }}
        </template>
      </el-table-column>
      <el-table-column prop="description" label="描述" min-width="160" show-overflow-tooltip />
      <el-table-column label="数量" width="100" align="center">
        <template #default="{ row }">
          {{ row.quantity != null ? row.quantity + (row.unit ? ' ' + row.unit : '') : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="unit_price" label="单价" width="90" align="right">
        <template #default="{ row }">
          {{ row.unit_price != null ? '¥' + (row.unit_price / 100).toFixed(2) : '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="size_info" label="尺寸" width="100" />
      <el-table-column prop="total_amount" label="总金额" width="120" align="right" sortable>
        <template #default="{ row }">
          <span style="font-weight: 600; color: #303133">
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
      width="650px"
      destroy-on-close
    >
      <el-form
        ref="formRef"
        :model="form"
        :rules="formRules"
        label-width="80px"
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

        <el-form-item label="类别" prop="category">
          <el-select v-model="form.category" style="width: 100%" filterable allow-create>
            <el-option
              v-for="(label, key) in IncomeCategoryLabels"
              :key="key"
              :label="label"
              :value="key"
            />
          </el-select>
        </el-form-item>

        <el-form-item label="描述" prop="description">
          <el-input v-model="form.description" placeholder="项目描述" />
        </el-form-item>

        <el-form-item label="数量" prop="quantity">
          <el-input-number v-model="form.quantity" :min="0" style="width: 160px" />
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

        <el-form-item label="尺寸">
          <el-input v-model="form.size_info" placeholder="例如：200×300cm" />
        </el-form-item>

        <el-form-item label="备注">
          <el-input v-model="form.remark" type="textarea" :rows="2" placeholder="可选备注" />
        </el-form-item>

        <!-- Image section -->
        <el-form-item label="图片">
          <div class="image-upload">
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
              <el-button
                class="image-remove"
                circle
                size="small"
                type="danger"
                :icon="Delete"
                @click="removeExistingImage(img.id, idx)"
              />
            </div>
            <div
              v-for="(file, idx) in newImages"
              :key="'new-' + idx"
              class="image-preview"
            >
              <img :src="file.preview" style="width: 80px; height: 80px; object-fit: cover; border-radius: 4px" />
              <el-button
                class="image-remove"
                circle
                size="small"
                type="danger"
                :icon="Delete"
                @click="newImages.splice(idx, 1)"
              />
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
    <el-dialog v-model="showSettleDialog" title="标记结清" width="400px">
      <el-form
        ref="settleFormRef"
        :model="settleForm"
        :rules="settleRules"
        label-width="80px"
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
          <el-descriptions-item label="类别">
            {{ IncomeCategoryLabels[detailRecord.category as IncomeCategory] || detailRecord.category }}
          </el-descriptions-item>
          <el-descriptions-item label="描述">{{ detailRecord.description || '-' }}</el-descriptions-item>
          <el-descriptions-item label="总金额">¥{{ (detailRecord.total_amount / 100).toFixed(2) }}</el-descriptions-item>
          <el-descriptions-item label="数量">{{ detailRecord.quantity ?? '-' }}</el-descriptions-item>
          <el-descriptions-item label="单价">{{ detailRecord.unit_price != null ? '¥' + (detailRecord.unit_price / 100).toFixed(2) : '-' }}</el-descriptions-item>
          <el-descriptions-item label="尺寸">{{ detailRecord.size_info || '-' }}</el-descriptions-item>
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
import { ref, reactive, computed, onMounted, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { save } from "@tauri-apps/plugin-dialog";
import { ElMessage, ElMessageBox } from "element-plus";
import type { FormInstance, FormRules } from "element-plus";
import type { IncomeRecord, IncomeImage, IncomeCategory, PaginatedRecords } from "@/types";
import { Plus, Delete, ArrowLeft } from "@element-plus/icons-vue";
import { IncomeCategoryLabels, PaymentMethods } from "@/types";
import { safeInvoke } from "@/utils/invoke";

const route = useRoute();
const router = useRouter();
const bookId = Number(route.params.id);
const bookName = ref("");

const tableRef = ref();
const records = ref<IncomeRecord[]>([]);
const selectedIds = ref<number[]>([]);
const loading = ref(false);
const total = ref(0);
const totalUnsettled = ref(0);
const bookTotalUnsettled = ref(0);
const currentPage = ref(1);
const pageSize = ref(20);

const filters = reactive({
  category: undefined as string | undefined,
  settlement_status: undefined as string | undefined,
  dateRange: null as [string, string] | null,
  keyword: "",
});

const selectedUnsettled = computed(() => {
  return records.value.filter(
    r => selectedIds.value.includes(r.id) && r.settlement_status === "unsettled"
  );
});

const hasActiveFilters = computed(() => {
  return !!(filters.category || filters.settlement_status || filters.dateRange || filters.keyword);
});

const IMAGE_MISSING = "__MISSING__";
const imageSrcMap = ref<Record<number, string>>({});

onMounted(async () => {
  await fetchBookName();
  await fetchRecords();
});

async function fetchBookName() {
  try {
    const books = await safeInvoke<any[]>("list_books");
    const book = books.find((b: any) => b.id === bookId);
    if (book) bookName.value = book.name;
  } catch { /* ignore */ }
}

async function fetchRecords() {
  loading.value = true;
  try {
    const [date_from, date_to] = filters.dateRange || [null, null];
    const res = await safeInvoke<PaginatedRecords>("list_records", {
      bookId,
      category: filters.category || null,
      settlementStatus: filters.settlement_status || null,
      dateFrom: date_from,
      dateTo: date_to,
      keyword: filters.keyword || null,
      page: currentPage.value,
      pageSize: pageSize.value,
    });
    records.value = res.records;
    total.value = res.total;
    totalUnsettled.value = res.total_unsettled;
    bookTotalUnsettled.value = res.book_total_unsettled;
    // Preload images returned with records
    for (const record of records.value) {
      for (const img of record.images) {
        loadImageSrc(img.id);
      }
    }
  } catch (e: any) {
    ElMessage.error(e || "加载失败");
  } finally {
    loading.value = false;
  }
}

function handlePageChange(page: number) {
  currentPage.value = page;
  fetchRecords();
}

function handleSizeChange(size: number) {
  pageSize.value = size;
  currentPage.value = 1;
  fetchRecords();
}

function handleFilterChange() {
  currentPage.value = 1;
  fetchRecords();
}

function onSelectionChange(rows: IncomeRecord[]) {
  selectedIds.value = rows.map(r => r.id);
}

function getImageUrl(imageId: number): string {
  const val = imageSrcMap.value[imageId];
  if (val === IMAGE_MISSING) return "";
  return val || "";
}

function isImageMissing(imageId: number): boolean {
  return imageSrcMap.value[imageId] === IMAGE_MISSING;
}

async function loadImageSrc(imageId: number) {
  if (imageSrcMap.value[imageId]) return;
  try {
    const dataUrl = await safeInvoke<string>("read_image_base64", { imageId });
    imageSrcMap.value[imageId] = dataUrl;
  } catch {
    imageSrcMap.value[imageId] = IMAGE_MISSING;
  }
}

// ===== Record Form (T13) =====

const showFormDialog = ref(false);
const isEditing = ref(false);
const editingId = ref<number | null>(null);
const saving = ref(false);
const formRef = ref<FormInstance>();
const form = reactive({
  date: "",
  category: "" as string,
  description: "",
  quantity: undefined as number | undefined,
  unit: "",
  unit_price: undefined as number | undefined,
  size_info: "",
  total_amount: 0,
  remark: "",
});
// Auto-calculate total amount (form fields are in yuan, backend stores cents)
watch(
  () => [form.quantity, form.unit_price],
  ([qty, price]) => {
    if (qty != null && price != null) {
      form.total_amount = Math.round(qty * price * 100) / 100;
    }
  }
);

const formRules: FormRules = {
  date: [{ required: true, message: "请选择日期", trigger: "blur" }],
  category: [{ required: true, message: "请选择类别", trigger: "change" }],
  description: [{ required: true, message: "请输入描述", trigger: "blur" }],
  quantity: [{ required: true, message: "请输入数量", trigger: "blur" }],
  unit: [{ required: true, message: "请选择单位", trigger: "change" }],
  unit_price: [{ required: true, message: "请输入单价", trigger: "blur" }],
  total_amount: [{ required: true, message: "请输入金额", trigger: "blur" }],
};

const existingImages = ref<IncomeImage[]>([]);
const removedImageIds = ref<number[]>([]);
const newImages = ref<{ file: File; preview: string }[]>([]);

function openCreate() {
  isEditing.value = false;
  editingId.value = null;
  form.date = new Date().toISOString().slice(0, 10);
  form.category = "";
  form.description = "";
  form.quantity = undefined;
  form.unit = "";
  form.unit_price = undefined;
  form.size_info = "";
  form.total_amount = 0;
  form.remark = "";
  existingImages.value = [];
  removedImageIds.value = [];
  newImages.value = [];
  showFormDialog.value = true;
}

function openEdit(record: IncomeRecord) {
  isEditing.value = true;
  editingId.value = record.id;
  form.date = record.date;
  form.category = record.category;
  form.description = record.description;
  form.quantity = record.quantity;
  form.unit = record.unit || "";
  form.unit_price = record.unit_price != null ? record.unit_price / 100 : undefined;
  form.size_info = record.size_info;
  form.total_amount = record.total_amount / 100;
  form.remark = record.remark;
  existingImages.value = [...record.images];
  removedImageIds.value = [];
  newImages.value = [];
  showFormDialog.value = true;
  // Preload existing images
  for (const img of record.images) {
    loadImageSrc(img.id);
  }
}

function onFileSelect(file: any) {
  const reader = new FileReader();
  reader.onload = (e) => {
    newImages.value.push({
      file: file.raw,
      preview: e.target!.result as string,
    });
  };
  reader.readAsDataURL(file.raw);
}

function removeExistingImage(imageId: number, _idx: number) {
  removedImageIds.value.push(imageId);
  existingImages.value = existingImages.value.filter(i => i.id !== imageId);
}

async function handleSave() {
  if (!formRef.value) return;
  const valid = await formRef.value.validate().catch(() => false);
  if (!valid) return;

  saving.value = true;
  try {
    const payload = {
      bookId,
      date: form.date,
      category: form.category,
      description: form.description,
      quantity: form.quantity,
      unit: form.unit,
      unitPrice: form.unit_price != null ? Math.round(form.unit_price * 100) : null,
      sizeInfo: form.size_info,
      totalAmount: Math.round(form.total_amount * 100),
      remark: form.remark,
    };

    // Prepare image data
    const imageData = await Promise.all(
      newImages.value.map(async (img) => {
        const buffer = await img.file.arrayBuffer();
        return {
          file_bytes: Array.from(new Uint8Array(buffer)),
          file_name: img.file.name,
        };
      })
    );

    if (isEditing.value && editingId.value) {
      const keepImageIds = existingImages.value.map(i => i.id);
      await safeInvoke("update_record_with_images", {
        id: editingId.value,
        ...payload,
        keepImageIds,
        newImages: imageData,
      });
      ElMessage.success("记录已更新");
    } else {
      await safeInvoke("create_record_with_images", {
        ...payload,
        images: imageData,
      });
      ElMessage.success("记录已创建");
    }

    showFormDialog.value = false;
    fetchRecords();
  } catch (e: any) {
    ElMessage.error(e || "操作失败");
  } finally {
    saving.value = false;
  }
}

async function handleDelete(id: number) {
  try {
    await safeInvoke("delete_record", { id });
    ElMessage.success("已删除");
    fetchRecords();
  } catch (e: any) {
    ElMessage.error(e || "删除失败");
  }
}

// ===== Settlement (T15) =====

const showSettleDialog = ref(false);
const settlingId = ref(0);
const settling = ref(false);
const settleFormRef = ref<FormInstance>();
const settleForm = reactive({ payment_date: "", payment_method: "" });
const settleRules: FormRules = {
  payment_date: [{ required: true, message: "请选择收款日期", trigger: "change" }],
  payment_method: [{ required: true, message: "请选择或输入收款方式", trigger: "change" }],
};
const unitOptions = ["份", "张", "块", "个", "本", "套", "卷", "米", "平方米", "次", "项"];
const paymentMethods = PaymentMethods;

function openSettle(record: IncomeRecord) {
  settlingId.value = record.id;
  settleForm.payment_date = new Date().toISOString().slice(0, 10);
  settleForm.payment_method = "";
  showSettleDialog.value = true;
}

async function handleSettle() {
  if (!settleFormRef.value) return;
  const valid = await settleFormRef.value.validate().catch(() => false);
  if (!valid) return;

  settling.value = true;
  try {
    await safeInvoke("settle_record", {
      id: settlingId.value,
      paymentDate: settleForm.payment_date,
      paymentMethod: settleForm.payment_method,
    });
    ElMessage.success("已标记为结清");
    showSettleDialog.value = false;
    fetchRecords();
  } catch (e: any) {
    ElMessage.error(e || "操作失败");
  } finally {
    settling.value = false;
  }
}

async function batchSettle() {
  for (const id of selectedIds.value) {
    const record = records.value.find(r => r.id === id);
    if (!record || record.settlement_status === "settled") continue;
    const today = new Date().toISOString().slice(0, 10);
    try {
      await safeInvoke("settle_record", {
        id,
        paymentDate: today,
        paymentMethod: "批量结清",
      });
    } catch { /* skip failed ones */ }
  }
  ElMessage.success("批量结清完成");
  selectedIds.value = [];
  fetchRecords();
}

async function handleUnsettle(record: IncomeRecord) {
  try {
    await ElMessageBox.confirm(
      "确定要将该记录回退为未结清状态吗？",
      "确认回退",
      { confirmButtonText: "确定回退", cancelButtonText: "取消", type: "warning" }
    );
    await safeInvoke("unsettle_record", { id: record.id });
    ElMessage.success("已回退为未结清");
    fetchRecords();
  } catch (e: any) {
    if (e === "cancel" || e?.message === "cancel") return;
    ElMessage.error(e || "操作失败");
  }
}

// ===== Export (T17) =====

async function exportSelected() {
  try {
    const savePath = await save({
      title: "保存账单",
      defaultPath: `账单_${bookName.value}_${new Date().toISOString().slice(0, 10)}.xlsx`,
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (!savePath) return;

    await safeInvoke("export_excel", {
      bookId,
      recordIds: selectedUnsettled.value.map(r => r.id),
      savePath: savePath as string,
    });
    ElMessage.success("导出成功");
  } catch (e: any) {
    ElMessage.error(e || "导出失败");
  }
}

async function exportAllUnsettled() {
  try {
    const savePath = await save({
      title: "保存全部未结清账单",
      defaultPath: `全部未结清_${bookName.value}_${new Date().toISOString().slice(0, 10)}.xlsx`,
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (!savePath) return;

    await safeInvoke("export_all_unsettled", {
      bookId,
      savePath: savePath as string,
    });
    ElMessage.success("导出成功");
  } catch (e: any) {
    ElMessage.error(e || "导出失败");
  }
}

// ===== Detail View =====
const showPreview = ref(false);
const previewImagesList = ref<IncomeImage[]>([]);
const previewIndex = ref(0);

function previewImages(images: IncomeImage[], idx: number) {
  previewImagesList.value = images;
  previewIndex.value = idx;
  showPreview.value = true;
  // Preload all images
  for (const img of images) {
    loadImageSrc(img.id);
  }
}

const showDetailDialog = ref(false);
const detailRecord = ref<IncomeRecord | null>(null);

async function viewDetail(record: IncomeRecord) {
  try {
    detailRecord.value = await safeInvoke<IncomeRecord>("get_record", { id: record.id });
    showDetailDialog.value = true;
    // Preload detail images
    for (const img of detailRecord.value.images) {
      loadImageSrc(img.id);
    }
  } catch (e: any) {
    ElMessage.error(e || "加载失败");
  }
}
</script>

<style scoped lang="scss">
.book-detail {
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;

    .header-left {
      display: flex;
      align-items: center;
      gap: 8px;
      h2 { font-size: 20px; }
    }

    .header-right {
      display: flex;
      gap: 8px;
    }
  }

  .filter-bar {
    display: flex;
    gap: 12px;
    margin-bottom: 16px;
    flex-wrap: wrap;
  }

  .table-container {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 150px);

    .el-table {
      flex: 1;
    }
  }

  .pagination-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 0 0;

    .unsettled-total {
      font-size: 14px;
      color: #606266;

      strong {
        color: #f56c6c;
        font-size: 16px;
      }
    }
  }
}

.image-upload {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;

  .image-preview {
    position: relative;

    .image-remove {
      position: absolute;
      top: -6px;
      right: -6px;
      width: 18px;
      height: 18px;
    }
  }

  .upload-trigger {
    width: 80px;
    height: 80px;
    border: 1px dashed #d9d9d9;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: #999;
    font-size: 20px;
    transition: border-color 160ms ease, color 160ms ease, background-color 160ms ease;

    &:hover {
      background: #f5f9ff;
      border-color: var(--color-primary);
      color: var(--color-primary);
    }
  }
}

.img-missing-tag {
  display: inline-block;
  width: 32px;
  height: 32px;
  line-height: 32px;
  text-align: center;
  font-size: 10px;
  color: #f56c6c;
  background: #fef0f0;
  border-radius: 3px;
  margin-right: 2px;
  cursor: default;
}

.img-missing-placeholder {
  width: 120px;
  height: 120px;
  border-radius: 6px;
  background: #f5f7fa;
  border: 1px dashed #dcdfe6;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 8px;
  span {
    font-size: 12px;
    color: #f56c6c;
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
  background: #f5f7fa;
  border: 2px solid transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  color: #f56c6c;
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
