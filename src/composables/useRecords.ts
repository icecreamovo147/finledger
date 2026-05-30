import { ref, reactive, computed } from "vue";
import { safeInvoke } from "@/utils/invoke";
import type { AccountBook, IncomeRecord, PaginatedRecords } from "@/types";

export const IMAGE_MISSING = "__MISSING__";

export function todayLocal(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function useRecords(bookId: number) {
  const bookName = ref("");

  const records = ref<IncomeRecord[]>([]);
  const selectedIds = ref<number[]>([]);
  const loading = ref(false);
  const total = ref(0);
  const totalUnsettled = ref(0);
  const bookTotalUnsettled = ref(0);
  const currentPage = ref(1);
  const pageSize = ref(20);

  const filters = reactive({
    settlement_status: undefined as string | undefined,
    dateRange: null as [string, string] | null,
    keyword: "",
  });

  const selectedUnsettled = computed(() =>
    records.value.filter(
      (r) => selectedIds.value.includes(r.id) && r.settlement_status === "unsettled",
    ),
  );

  const hasActiveFilters = computed(
    () => !!(filters.settlement_status || filters.dateRange || filters.keyword),
  );

  // ---- Image cache (shared with form & settlement composables) ----
  const imageSrcMap = ref<Record<number, string>>({});

  function getImageUrl(imageId: number): string {
    const val = imageSrcMap.value[imageId];
    return val === IMAGE_MISSING ? "" : val || "";
  }

  function isImageMissing(imageId: number): boolean {
    return imageSrcMap.value[imageId] === IMAGE_MISSING;
  }

  async function loadImageSrc(imageId: number) {
    if (imageSrcMap.value[imageId]) return;
    try {
      imageSrcMap.value[imageId] = await safeInvoke<string>("read_image_base64", { imageId });
    } catch {
      imageSrcMap.value[imageId] = IMAGE_MISSING;
    }
  }

  // ---- Data fetching ----
  async function fetchBookName() {
    try {
      const book = await safeInvoke<AccountBook>("get_book", { id: bookId });
      bookName.value = book.name;
    } catch {
      /* ignore */
    }
  }

  async function fetchRecords() {
    loading.value = true;
    try {
      const [date_from, date_to] = filters.dateRange || [null, null];
      const res = await safeInvoke<PaginatedRecords>("list_records", {
        bookId,
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
      // Let caller handle error display
      throw e;
    } finally {
      loading.value = false;
    }
  }

  function handlePageChange(page: number) {
    currentPage.value = page;
    fetchRecords().catch(() => {});
  }

  function handleSizeChange(size: number) {
    pageSize.value = size;
    currentPage.value = 1;
    fetchRecords().catch(() => {});
  }

  function handleFilterChange() {
    currentPage.value = 1;
    fetchRecords().catch(() => {});
  }

  function onSelectionChange(rows: IncomeRecord[]) {
    selectedIds.value = rows.map((r) => r.id);
  }

  return {
    bookName,
    records,
    selectedIds,
    loading,
    total,
    totalUnsettled,
    bookTotalUnsettled,
    currentPage,
    pageSize,
    filters,
    selectedUnsettled,
    hasActiveFilters,
    // image cache
    imageSrcMap,
    getImageUrl,
    isImageMissing,
    loadImageSrc,
    // actions
    fetchBookName,
    fetchRecords,
    handlePageChange,
    handleSizeChange,
    handleFilterChange,
    onSelectionChange,
  };
}
