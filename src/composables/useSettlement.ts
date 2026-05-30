import { ref, reactive, type Ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import type { FormInstance, FormRules } from "element-plus";
import type { IncomeRecord, IncomeImage } from "@/types";
import { safeInvoke } from "@/utils/invoke";
import { IMAGE_MISSING, todayLocal } from "./useRecords";

export function useSettlement(
  bookId: number,
  bookName: Ref<string>,
  deps: {
    records: Ref<IncomeRecord[]>;
    selectedIds: Ref<number[]>;
    selectedUnsettled: Ref<IncomeRecord[]>;
    imageSrcMap: Ref<Record<number, string>>;
    refreshRecords: () => Promise<void>;
  },
) {
  // ---- Settlement ----
  const showSettleDialog = ref(false);
  const settlingId = ref(0);
  const settling = ref(false);
  const settleFormRef = ref<FormInstance>();
  const settleForm = reactive({ payment_date: "", payment_method: "" });
  const settleRules: FormRules = {
    payment_date: [{ required: true, message: "请选择收款日期", trigger: "change" }],
    payment_method: [{ required: true, message: "请选择或输入收款方式", trigger: "change" }],
  };

  function openSettle(record: IncomeRecord) {
    settlingId.value = record.id;
    settleForm.payment_date = todayLocal();
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
      await deps.refreshRecords();
    } catch (e: any) {
      ElMessage.error(e || "操作失败");
    } finally {
      settling.value = false;
    }
  }

  async function batchSettle() {
    let successCount = 0;
    let failCount = 0;
    for (const id of deps.selectedIds.value) {
      const record = deps.records.value.find((r) => r.id === id);
      if (!record || record.settlement_status === "settled") {
        failCount++;
        continue;
      }
      try {
        await safeInvoke("settle_record", {
          id,
          paymentDate: todayLocal(),
          paymentMethod: "批量结清",
        });
        successCount++;
      } catch {
        failCount++;
      }
    }
    if (failCount > 0) {
      ElMessage.warning(`批量结清完成：${successCount} 条成功，${failCount} 条失败`);
    } else {
      ElMessage.success(`批量结清完成，共 ${successCount} 条`);
    }
    deps.selectedIds.value = [];
    await deps.refreshRecords();
  }

  async function handleUnsettle(record: IncomeRecord) {
    try {
      await ElMessageBox.confirm(
        "确定要将该记录回退为未结清状态吗？",
        "确认回退",
        { confirmButtonText: "确定回退", cancelButtonText: "取消", type: "warning" },
      );
      await safeInvoke("unsettle_record", { id: record.id });
      ElMessage.success("已回退为未结清");
      await deps.refreshRecords();
    } catch (e: any) {
      if (e === "cancel" || e?.message === "cancel") return;
      ElMessage.error(e || "操作失败");
    }
  }

  // ---- Export ----
  function sanitizeFileName(name: string): string {
    return (
      name
        .replace(/[<>:"/\\|?*\x00-\x1F]/g, "_")
        .replace(/[. ]+$/g, "")
        .trim() || "未命名账本"
    );
  }

  async function exportSelected(saveDialog: typeof import("@tauri-apps/plugin-dialog").save) {
    try {
      const savePath = await saveDialog({
        title: "保存账单",
        defaultPath: `账单_${sanitizeFileName(bookName.value)}_${todayLocal()}.xlsx`,
        filters: [{ name: "Excel", extensions: ["xlsx"] }],
      });
      if (!savePath) return;

      await safeInvoke("export_excel", {
        bookId,
        recordIds: deps.selectedUnsettled.value.map((r) => r.id),
        savePath: savePath as string,
      });
      ElMessage.success("导出成功");
    } catch (e: any) {
      ElMessage.error(e || "导出失败");
    }
  }

  async function exportAllUnsettled(saveDialog: typeof import("@tauri-apps/plugin-dialog").save) {
    try {
      const savePath = await saveDialog({
        title: "保存全部未结清账单",
        defaultPath: `全部未结清_${sanitizeFileName(bookName.value)}_${todayLocal()}.xlsx`,
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

  // ---- Preview & Detail ----
  const showPreview = ref(false);
  const previewImagesList = ref<IncomeImage[]>([]);
  const previewIndex = ref(0);

  function previewImages(images: IncomeImage[], idx: number) {
    previewImagesList.value = images;
    previewIndex.value = idx;
    showPreview.value = true;
    for (const img of images) {
      loadDetailImage(img.id);
    }
  }

  function previewNewImages(
    newImagesList: { tempId: string; originalName: string; previewUrl: string }[],
    startIndex?: number,
  ) {
    const list: IncomeImage[] = newImagesList.map((img, i) => ({
      id: -1 - i,
      record_id: 0,
      file_path: "",
      original_name: img.originalName,
      created_at: "",
    }));
    newImagesList.forEach((img, i) => {
      deps.imageSrcMap.value[-1 - i] = img.previewUrl;
    });
    previewImagesList.value = list;
    previewIndex.value = startIndex ?? 0;
    showPreview.value = true;
  }

  function getImageUrl(imageId: number): string {
    const val = deps.imageSrcMap.value[imageId];
    return val === IMAGE_MISSING ? "" : val || "";
  }

  function isImageMissing(imageId: number): boolean {
    return deps.imageSrcMap.value[imageId] === IMAGE_MISSING;
  }

  async function loadDetailImage(imageId: number) {
    if (deps.imageSrcMap.value[imageId]) return;
    try {
      deps.imageSrcMap.value[imageId] = await safeInvoke<string>("read_image_base64", { imageId });
    } catch {
      deps.imageSrcMap.value[imageId] = IMAGE_MISSING;
    }
  }

  // ---- Detail View ----
  const showDetailDialog = ref(false);
  const detailRecord = ref<IncomeRecord | null>(null);

  async function viewDetail(record: IncomeRecord) {
    try {
      detailRecord.value = await safeInvoke<IncomeRecord>("get_record", { id: record.id });
      showDetailDialog.value = true;
      for (const img of detailRecord.value.images) {
        loadDetailImage(img.id);
      }
    } catch (e: any) {
      ElMessage.error(e || "加载失败");
    }
  }

  return {
    showSettleDialog,
    settlingId,
    settling,
    settleFormRef,
    settleForm,
    settleRules,
    openSettle,
    handleSettle,
    batchSettle,
    handleUnsettle,
    exportSelected,
    exportAllUnsettled,
    showPreview,
    previewImagesList,
    previewIndex,
    previewImages,
    previewNewImages,
    getImageUrl,
    isImageMissing,
    loadDetailImage,
    showDetailDialog,
    detailRecord,
    viewDetail,
  };
}
