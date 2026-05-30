import { ref, reactive, watch } from "vue";
import { ElMessage } from "element-plus";
import type { FormInstance, FormRules } from "element-plus";
import type { IncomeRecord, IncomeImage } from "@/types";
import { safeInvoke } from "@/utils/invoke";
import { todayLocal } from "./useRecords";

interface PendingImage {
  tempId: string;
  originalName: string;
  previewUrl: string;
}

export function useRecordForm(
  bookId: number,
  deps: {
    loadImageSrc: (id: number) => Promise<void>;
    refreshRecords: () => Promise<void>;
  },
) {
  const showFormDialog = ref(false);
  const isEditing = ref(false);
  const editingId = ref<number | null>(null);
  const saving = ref(false);
  const formRef = ref<FormInstance>();

  const form = reactive({
    date: "",
    service_content: "",
    specification: "",
    quantity: undefined as number | undefined,
    unit: "",
    unit_price: undefined as number | undefined,
    total_amount: 0,
    remark: "",
  });

  // Auto-calculate total amount (form fields are in yuan, backend stores fen/cents).
  // Use integer arithmetic to avoid IEEE 754 rounding errors that could cause a
  // ±1 fen mismatch with the backend's checked_mul validation.
  watch(
    () => [form.quantity, form.unit_price],
    ([qty, price]) => {
      if (qty != null && price != null) {
        const qtyCents = Math.round(qty * 100);
        const priceCents = Math.round(price * 100);
        form.total_amount = (qtyCents * priceCents) / 10000;
      }
    },
  );

  const formRules: FormRules = {
    date: [{ required: true, message: "请选择日期", trigger: "blur" }],
    service_content: [{ required: true, message: "请输入服务项目及内容", trigger: "blur" }],
    quantity: [{ required: true, message: "请输入数量", trigger: "blur" }],
    unit: [{ required: true, message: "请选择单位", trigger: "change" }],
    unit_price: [{ required: true, message: "请输入单价", trigger: "blur" }],
    total_amount: [{ required: true, message: "请输入金额", trigger: "blur" }],
  };

  // ---- Images ----
  const existingImages = ref<IncomeImage[]>([]);
  const removedImageIds = ref<number[]>([]);
  const newImages = ref<PendingImage[]>([]);
  const isDragOver = ref(false);
  const imageSessionId = ref("");
  const imageSessionCommitted = ref(false);

  function openCreate() {
    isEditing.value = false;
    editingId.value = null;
    form.date = todayLocal();
    form.service_content = "";
    form.specification = "";
    form.quantity = undefined;
    form.unit = "";
    form.unit_price = undefined;
    form.total_amount = 0;
    form.remark = "";
    existingImages.value = [];
    removedImageIds.value = [];
    newImages.value = [];
    imageSessionId.value = crypto.randomUUID();
    imageSessionCommitted.value = false;
    showFormDialog.value = true;
  }

  function openEdit(record: IncomeRecord) {
    isEditing.value = true;
    editingId.value = record.id;
    form.date = record.date;
    form.service_content = record.service_content;
    form.specification = record.specification;
    form.quantity = record.quantity;
    form.unit = record.unit || "";
    form.unit_price = record.unit_price != null ? record.unit_price / 100 : undefined;
    form.total_amount = record.total_amount / 100;
    form.remark = record.remark;
    existingImages.value = [...record.images];
    removedImageIds.value = [];
    newImages.value = [];
    imageSessionId.value = crypto.randomUUID();
    imageSessionCommitted.value = false;
    showFormDialog.value = true;
    // Preload existing images
    for (const img of record.images) {
      deps.loadImageSrc(img.id);
    }
  }

  async function addImageFile(file: File) {
    if (!file.type.startsWith("image/")) return;
    try {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      const staged = await safeInvoke<{
        temp_id: string;
        original_name: string;
        preview_data_url: string;
      }>("stage_image_bytes", {
        sessionId: imageSessionId.value,
        fileName: file.name,
        fileBytes: bytes,
      });
      newImages.value.push({
        tempId: staged.temp_id,
        originalName: staged.original_name,
        previewUrl: staged.preview_data_url,
      });
    } catch (e: any) {
      ElMessage.warning(e || "暂存图片失败，请重试");
    }
  }

  async function addImagePath(path: string) {
    try {
      const staged = await safeInvoke<{
        temp_id: string;
        original_name: string;
        preview_data_url: string;
      }>("stage_image_from_path", {
        sessionId: imageSessionId.value,
        path,
      });
      newImages.value.push({
        tempId: staged.temp_id,
        originalName: staged.original_name,
        previewUrl: staged.preview_data_url,
      });
    } catch (e: any) {
      ElMessage.warning(e || "暂存图片失败，请重试");
    }
  }

  function onFileSelect(uploadFile: any) {
    addImageFile(uploadFile.raw as File);
  }

  function onDialogDragOver(e: DragEvent) {
    if (e.dataTransfer?.types.includes("Files")) {
      isDragOver.value = true;
    }
  }

  function onDragLeave() {
    isDragOver.value = false;
  }

  async function onDrop(e: DragEvent) {
    isDragOver.value = false;
    const files = e.dataTransfer?.files;
    if (!files) return;
    for (let i = 0; i < files.length; i++) {
      await addImageFile(files[i]);
    }
  }

  async function onPaste(e: ClipboardEvent) {
    const items = e.clipboardData?.items;
    if (!items) return;
    for (let i = 0; i < items.length; i++) {
      if (items[i].type.startsWith("image/")) {
        const blob = items[i].getAsFile();
        if (blob) await addImageFile(blob);
      }
    }
  }

  async function removeNewImage(idx: number) {
    const img = newImages.value[idx];
    if (!img) return;
    try {
      await safeInvoke("delete_staged_image", {
        sessionId: imageSessionId.value,
        tempId: img.tempId,
      });
    } catch {
      /* best-effort */
    }
    newImages.value.splice(idx, 1);
  }

  function removeExistingImage(imageId: number) {
    removedImageIds.value.push(imageId);
    existingImages.value = existingImages.value.filter((i) => i.id !== imageId);
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
        serviceContent: form.service_content.trim(),
        specification: form.specification.trim(),
        quantity: form.quantity,
        unit: form.unit,
        unitPrice: form.unit_price != null ? Math.round(form.unit_price * 100) : null,
        totalAmount: Math.round(form.total_amount * 100),
        remark: form.remark,
      };

      const tempImageIds = newImages.value.map((img) => img.tempId);

      if (isEditing.value && editingId.value) {
        const keepImageIds = existingImages.value.map((i) => i.id);
        await safeInvoke("update_record_with_staged_images", {
          id: editingId.value,
          ...payload,
          keepImageIds,
          sessionId: imageSessionId.value,
          tempImageIds,
        });
        ElMessage.success("记录已更新");
      } else {
        await safeInvoke("create_record_with_staged_images", {
          ...payload,
          sessionId: imageSessionId.value,
          tempImageIds,
        });
        ElMessage.success("记录已创建");
      }

      imageSessionCommitted.value = true;
      showFormDialog.value = false;
      await deps.refreshRecords();
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
      await deps.refreshRecords();
    } catch (e: any) {
      ElMessage.error(e || "删除失败");
    }
  }

  // Clean up staging session when dialog closes without saving
  watch(showFormDialog, async (visible) => {
    if (!visible && imageSessionId.value && !imageSessionCommitted.value) {
      try {
        await safeInvoke("cancel_image_staging_session", {
          sessionId: imageSessionId.value,
        });
      } catch {
        /* best-effort */
      }
    }
  });

  return {
    showFormDialog,
    isEditing,
    editingId,
    saving,
    formRef,
    form,
    formRules,
    existingImages,
    removedImageIds,
    newImages,
    isDragOver,
    imageSessionId,
    imageSessionCommitted,
    openCreate,
    openEdit,
    addImageFile,
    addImagePath,
    onFileSelect,
    onDialogDragOver,
    onDragLeave,
    onDrop,
    onPaste,
    removeNewImage,
    removeExistingImage,
    handleSave,
    handleDelete,
  };
}
