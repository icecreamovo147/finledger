<template>
    <div class="book-list">
        <!-- Search Bar -->
        <div class="filter-bar">
            <el-input
                v-model="keyword"
                placeholder="搜索账本名称或备注..."
                clearable
                @input="onSearchInput"
                @clear="onSearchClear"
                @keydown.enter="onSearchNow"
            >
                <template #prefix>
                    <el-icon><Search /></el-icon>
                </template>
            </el-input>
            <el-button type="primary" :icon="Search" @click="onSearchNow">
                搜索
            </el-button>
        </div>

        <div v-loading="loading" class="book-list-panel">
            <el-empty
                v-if="!loading && books.length === 0"
                description="暂无账本，点击上方按钮创建"
            />

            <template v-else>
                <div class="book-list-head">
                    <span>账本</span>
                    <span>未结清金额</span>
                    <span>记录数</span>
                    <span>更新时间</span>
                    <span></span>
                </div>

                <div
                    v-for="book in books"
                    :key="book.id"
                    class="book-row"
                    role="button"
                    tabindex="0"
                    @click="openBook(book)"
                    @keydown.enter.prevent="openBook(book)"
                >
                    <div class="book-main">
                        <div class="book-mark">{{ book.name.slice(0, 1) }}</div>
                        <div class="book-copy">
                            <div class="book-title">{{ book.name }}</div>
                            <div class="book-remark">
                                {{ book.remark || "无备注" }}
                            </div>
                        </div>
                    </div>

                    <div class="metric amount">
                        ¥{{ formatAmount(book.total_unsettled || 0) }}
                    </div>
                    <div class="metric">{{ book.record_count || 0 }}</div>
                    <div class="muted-text">
                        {{ formatDate(book.updated_at || book.created_at) }}
                    </div>

                    <div class="row-actions" @click.stop>
                        <el-tooltip content="查看记录" placement="top">
                            <el-button
                                text
                                circle
                                :icon="ArrowRight"
                                @click="openBook(book)"
                            />
                        </el-tooltip>
                        <el-tooltip content="编辑账本" placement="top">
                            <el-button
                                text
                                circle
                                :icon="Edit"
                                @click="openEdit(book)"
                            />
                        </el-tooltip>
                        <el-popconfirm
                            title="删除账本将同时删除所有记录，确定？"
                            @confirm="handleDelete(book.id)"
                        >
                            <template #reference>
                                <el-button
                                    text
                                    circle
                                    type="danger"
                                    :icon="Delete"
                                />
                            </template>
                        </el-popconfirm>
                    </div>
                </div>
            </template>
        </div>

        <div v-if="total > 0" class="pagination-wrapper">
            <el-pagination
                v-model:current-page="currentPage"
                v-model:page-size="pageSize"
                :page-sizes="[10, 20, 50, 100]"
                :total="total"
                layout="total, sizes, prev, pager, next, jumper"
                background
                @current-change="fetchBooks"
                @size-change="handleSizeChange"
            />
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
                @submit.prevent
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
import { computed, ref, reactive, onBeforeUnmount, onMounted } from "vue";
import { useRouter } from "vue-router";
import { ElMessage } from "element-plus";
import { ArrowRight, Delete, Edit, Plus, Search } from "@element-plus/icons-vue";
import type { FormInstance, FormRules } from "element-plus";
import type { AccountBook, PaginatedBooks } from "@/types";
import { safeInvoke } from "@/utils/invoke";
import { usePageHeaderStore } from "@/stores/pageHeader";

const router = useRouter();
const pageHeaderStore = usePageHeaderStore();
const books = ref<AccountBook[]>([]);
const loading = ref(false);
const total = ref(0);
const currentPage = ref(1);
const pageSize = ref(10);
const keyword = ref("");
let searchTimer: ReturnType<typeof setTimeout> | null = null;

const showDialog = ref(false);
const isEditing = ref(false);
const editingId = ref<number | null>(null);
const saving = ref(false);
const formRef = ref<FormInstance>();
const form = reactive({ name: "", remark: "" });
const rules: FormRules = {
    name: [{ required: true, message: "请输入账本名称", trigger: "blur" }],
};

onMounted(() => {
    pageHeaderStore.setActions([
        {
            key: "create-book",
            label: "新增账本",
            icon: Plus,
            type: "primary",
            onClick: openCreate,
        },
    ]);
    fetchBooks();
});

onBeforeUnmount(() => {
    if (searchTimer) clearTimeout(searchTimer);
    pageHeaderStore.clearActions();
});

async function fetchBooks() {
    loading.value = true;
    try {
        const res = await safeInvoke<PaginatedBooks>("list_books", {
            page: currentPage.value,
            pageSize: pageSize.value,
            keyword: keyword.value || null,
        });
        books.value = res.books;
        total.value = res.total;
    } catch (e: any) {
        ElMessage.error(e || "加载失败");
    } finally {
        loading.value = false;
    }
}

function handleSizeChange() {
    currentPage.value = 1;
    fetchBooks();
}

function doSearch() {
    if (searchTimer) clearTimeout(searchTimer);
    currentPage.value = 1;
    fetchBooks();
}

function onSearchInput() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(doSearch, 300);
}

function onSearchNow() {
    doSearch();
}

function onSearchClear() {
    keyword.value = "";
    doSearch();
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

function openBook(book: AccountBook) {
    router.push(`/books/${book.id}`);
}

async function handleSave() {
    if (!formRef.value) return;
    const valid = await formRef.value.validate().catch(() => false);
    if (!valid) return;

    saving.value = true;
    try {
        if (isEditing.value && editingId.value) {
            await safeInvoke("update_book", {
                id: editingId.value,
                name: form.name,
                remark: form.remark,
            });
            ElMessage.success("账本已更新");
        } else {
            await safeInvoke("create_book", {
                name: form.name,
                remark: form.remark,
            });
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
        await safeInvoke("delete_book", { id });
        ElMessage.success("账本已删除");
        fetchBooks();
    } catch (e: any) {
        ElMessage.error(e || "删除失败");
    }
}

function formatAmount(val: number): string {
    return (val / 100).toLocaleString("zh-CN", {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
    });
}

function formatDate(val?: string): string {
    if (!val) return "-";
    return val.slice(0, 16);
}
</script>

<style scoped lang="scss">
.book-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    gap: 14px;
}

.filter-bar {
    display: flex;
    gap: 12px;
    padding: 16px;
    flex-wrap: wrap;
    flex-shrink: 0;
    border: 1px solid var(--border-color);
    border-radius: 14px;
    background: var(--card-bg);
    box-shadow: var(--card-shadow);

    .el-input {
        max-width: 320px;
    }
}

.list-summary {
    display: flex;
    align-items: center;
    gap: 22px;
    min-height: 44px;
    padding: 0 4px;
    flex-shrink: 0;

    > div {
        display: flex;
        align-items: baseline;
        gap: 8px;
    }

    strong {
        color: var(--text-heading);
        font-size: 20px;
        font-weight: 700;
    }
}

.summary-label {
    color: var(--text-tertiary);
    font-size: 13px;
}

.book-list-panel {
    flex: 1;
    min-height: 0;
    border: 1px solid var(--border-color);
    border-radius: 8px;
    overflow: hidden;
    background: var(--card-bg);
    display: flex;
    flex-direction: column;

    .el-empty {
        flex: 1;
        min-height: 240px;
        display: flex;
        justify-content: center;
    }
}

.book-list-head,
.book-row {
    display: grid;
    grid-template-columns: minmax(260px, 1fr) 160px 92px 150px 132px;
    align-items: center;
    column-gap: 18px;
}

.book-list-head {
    height: 42px;
    padding: 0 18px;
    color: var(--text-tertiary);
    font-size: 12px;
    border-bottom: 1px solid var(--border-color);
    background: var(--card-bg-subtle);
}

.book-row {
    min-height: 72px;
    padding: 0 18px;
    border-bottom: 1px solid var(--border-color);
    cursor: pointer;
    transition:
        background-color 150ms ease,
        box-shadow 150ms ease;

    &:last-child {
        border-bottom: 0;
    }

    &:hover,
    &:focus-visible {
        outline: none;
        background: var(--hover-bg);
        box-shadow: inset 3px 0 0 var(--color-primary);
    }
}

.book-main {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
}

.book-mark {
    width: 36px;
    height: 36px;
    flex: 0 0 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    background: var(--card-bg-subtle);
    color: var(--color-primary);
    font-weight: 700;
}

.book-copy {
    min-width: 0;
}

.book-title {
    color: var(--text-heading);
    font-size: 15px;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.book-remark {
    margin-top: 4px;
    color: var(--text-tertiary);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.muted-text {
    color: var(--text-tertiary);
    font-size: 13px;
}

.metric {
    color: var(--text-heading);
    font-size: 14px;
    font-weight: 600;

    &.amount {
        color: var(--color-warning);
    }
}

.row-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;

    :deep(.el-button) {
        width: 30px;
        height: 30px;
    }
}

.pagination-wrapper {
    display: flex;
    justify-content: center;
    padding: 2px 0;
    flex-shrink: 0;
}

@media (max-width: 900px) {
    .book-list-head {
        display: none;
    }

    .book-row {
        grid-template-columns: minmax(0, 1fr) auto;
        grid-template-areas:
            "main actions"
            "amount actions"
            "count actions";
        row-gap: 6px;
        min-height: 98px;
        padding: 14px 16px;
    }

    .book-main {
        grid-area: main;
    }

    .metric.amount {
        grid-area: amount;
    }

    .metric:not(.amount) {
        grid-area: count;
        color: var(--text-tertiary);
        font-size: 13px;

        &::before {
            content: "记录数 ";
        }
    }

    .book-row > .muted-text {
        display: none;
    }

    .row-actions {
        grid-area: actions;
    }
}
</style>
