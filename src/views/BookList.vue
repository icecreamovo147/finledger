<template>
  <div class="book-list">
    <div class="page-header">
      <h2>账本管理</h2>
      <el-button type="primary" @click="openCreate">
        <el-icon><Plus /></el-icon>新增账本
      </el-button>
    </div>

    <div v-loading="loading" class="book-grid">
      <el-empty v-if="!loading && books.length === 0" description="暂无账本，点击上方按钮创建" />
      <div
        v-for="book in books"
        :key="book.id"
        class="book-card"
        @click="router.push(`/books/${book.id}`)"
      >
        <div class="book-card-header">
          <h3>{{ book.name }}</h3>
          <div class="book-actions" @click.stop>
            <el-button text size="small" @click="openEdit(book)">编辑</el-button>
            <el-popconfirm title="删除账本将同时删除所有记录，确定？" @confirm="handleDelete(book.id)">
              <template #reference>
                <el-button text type="danger" size="small">删除</el-button>
              </template>
            </el-popconfirm>
          </div>
        </div>
        <p v-if="book.remark" class="book-remark">{{ book.remark }}</p>
        <div class="book-stats">
          <div class="stat">
            <span class="stat-value">¥{{ formatAmount(book.total_unsettled || 0) }}</span>
            <span class="stat-label">未结清</span>
          </div>
          <div class="stat">
            <span class="stat-value">{{ book.record_count || 0 }}</span>
            <span class="stat-label">记录数</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 新增/编辑弹窗 -->
    <el-dialog
      v-model="showDialog"
      :title="isEditing ? '编辑账本' : '新增账本'"
      width="450px"
    >
      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-position="top"
      >
        <el-form-item label="账本名称" prop="name">
          <el-input v-model="form.name" placeholder="例如：XX 公司" />
        </el-form-item>
        <el-form-item label="备注" prop="remark">
          <el-input
            v-model="form.remark"
            type="textarea"
            :rows="2"
            placeholder="可选备注信息"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showDialog = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="handleSave">
          确定
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { ElMessage } from "element-plus";
import { Plus } from "@element-plus/icons-vue";
import type { FormInstance, FormRules } from "element-plus";
import type { AccountBook } from "@/types";

const router = useRouter();
const books = ref<AccountBook[]>([]);
const loading = ref(false);

const showDialog = ref(false);
const isEditing = ref(false);
const editingId = ref<number | null>(null);
const saving = ref(false);
const formRef = ref<FormInstance>();
const form = reactive({ name: "", remark: "" });
const rules: FormRules = {
  name: [{ required: true, message: "请输入账本名称", trigger: "blur" }],
};

onMounted(() => fetchBooks());

async function fetchBooks() {
  loading.value = true;
  try {
    books.value = await invoke<AccountBook[]>("list_books");
  } catch (e: any) {
    ElMessage.error(e || "加载失败");
  } finally {
    loading.value = false;
  }
}

function openCreate() {
  isEditing.value = false;
  editingId.value = null;
  form.name = "";
  form.remark = "";
  showDialog.value = true;
}

function openEdit(book: AccountBook) {
  isEditing.value = true;
  editingId.value = book.id;
  form.name = book.name;
  form.remark = book.remark;
  showDialog.value = true;
}

async function handleSave() {
  if (!formRef.value) return;
  const valid = await formRef.value.validate().catch(() => false);
  if (!valid) return;

  saving.value = true;
  try {
    if (isEditing.value && editingId.value) {
      await invoke("update_book", {
        id: editingId.value,
        name: form.name,
        remark: form.remark,
      });
      ElMessage.success("账本已更新");
    } else {
      await invoke("create_book", { name: form.name, remark: form.remark });
      ElMessage.success("账本已创建");
    }
    showDialog.value = false;
    fetchBooks();
  } catch (e: any) {
    ElMessage.error(e || "操作失败");
  } finally {
    saving.value = false;
  }
}

async function handleDelete(id: number) {
  try {
    await invoke("delete_book", { id });
    ElMessage.success("账本已删除");
    fetchBooks();
  } catch (e: any) {
    ElMessage.error(e || "删除失败");
  }
}

function formatAmount(val: number): string {
  return val.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
</script>

<style scoped lang="scss">
.book-list {
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 24px;

    h2 { font-size: 20px; }
  }
}

.book-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 20px;

  .el-empty {
    grid-column: 1 / -1;
    justify-self: center;
  }
}

.book-card {
  background: #fff;
  border-radius: 10px;
  padding: 24px;
  cursor: pointer;
  border: 1px solid #ebeef5;
  transition: box-shadow 180ms ease, transform 180ms ease, border-color 180ms ease;

  &:hover {
    border-color: #dcdfe6;
    box-shadow: 0 8px 22px rgba(31, 45, 61, 0.08);
    transform: translateY(-2px);
  }

  .book-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;

    h3 { font-size: 17px; color: #303133; }

    .book-actions {
      opacity: 0.72;
      transition: opacity 140ms ease;
    }
  }

  &:hover .book-actions { opacity: 1; }

  .book-remark {
    color: #909399;
    font-size: 13px;
    margin-bottom: 16px;
    min-height: 20px;
  }

  .book-stats {
    display: flex;
    gap: 32px;
    padding-top: 16px;
    border-top: 1px solid #f2f3f5;

    .stat {
      display: flex;
      flex-direction: column;

      .stat-value { font-size: 18px; font-weight: 600; color: #303133; }
      .stat-label { font-size: 12px; color: #909399; margin-top: 2px; }
    }
  }
}
</style>
