<template>
  <div class="user-management">
    <div class="table-panel">
      <div class="panel-heading">
        <div>
          <h3>成员与权限</h3>
          <p>管理可访问 FinLedger 的本地用户账号。</p>
        </div>
        <el-tag type="info">{{ users.length }} 个用户</el-tag>
      </div>

      <el-table :data="users" stripe v-loading="loading">
        <el-table-column prop="id" label="ID" width="80" />
        <el-table-column prop="username" label="用户名" />
        <el-table-column prop="created_at" label="创建时间" width="180" />
        <el-table-column label="操作" width="200" align="right">
          <template #default="{ row }">
            <el-button text type="primary" size="small" @click="openChangePwd(row)">
              修改密码
            </el-button>
            <el-popconfirm
              title="确定删除该用户？"
              @confirm="handleDelete(row.id)"
            >
              <template #reference>
                <el-button
                  text
                  type="danger"
                  size="small"
                  :disabled="row.id === authStore.user?.id"
                >
                  删除
                </el-button>
              </template>
            </el-popconfirm>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <!-- 新增用户弹窗 -->
    <el-dialog v-model="showCreateDialog" title="新增用户" width="400px">
      <el-form
        ref="createFormRef"
        :model="createForm"
        :rules="createRules"
        label-position="top"
      >
        <el-form-item label="用户名" prop="username">
          <el-input v-model="createForm.username" placeholder="请输入用户名" />
        </el-form-item>
        <el-form-item label="密码" prop="password">
          <el-input
            v-model="createForm.password"
            type="password"
            placeholder="请输入密码"
            show-password
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showCreateDialog = false">取消</el-button>
        <el-button type="primary" :loading="creating" @click="handleCreate">
          确定
        </el-button>
      </template>
    </el-dialog>

    <!-- 修改密码弹窗 -->
    <el-dialog v-model="showPwdDialog" title="重置密码" width="400px">
      <el-form
        ref="pwdFormRef"
        :model="pwdForm"
        :rules="pwdRules"
        label-position="top"
      >
        <el-form-item label="新密码" prop="newPassword">
          <el-input
            v-model="pwdForm.newPassword"
            type="password"
            placeholder="请输入新密码"
            show-password
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showPwdDialog = false">取消</el-button>
        <el-button type="primary" :loading="changingPwd" @click="handleChangePwd">
          确定
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onBeforeUnmount, onMounted } from "vue";
import { useAuthStore } from "@/stores/auth";
import { usePageHeaderStore } from "@/stores/pageHeader";
import { ElMessage } from "element-plus";
import { Plus } from "@element-plus/icons-vue";
import type { FormInstance, FormRules } from "element-plus";
import type { User } from "@/types";
import { safeInvoke } from "@/utils/invoke";

const authStore = useAuthStore();
const pageHeaderStore = usePageHeaderStore();
const users = ref<User[]>([]);
const loading = ref(false);

// Create
const showCreateDialog = ref(false);
const createFormRef = ref<FormInstance>();
const creating = ref(false);
const createForm = reactive({ username: "", password: "" });
const createRules: FormRules = {
  username: [
    { required: true, message: "请输入用户名", trigger: "blur" },
    { min: 2, max: 50, message: "用户名长度 2-50 个字符", trigger: "blur" },
  ],
  password: [
    { required: true, message: "请输入密码", trigger: "blur" },
    { min: 4, message: "密码至少 4 位", trigger: "blur" },
  ],
};

// Change password
const showPwdDialog = ref(false);
const pwdFormRef = ref<FormInstance>();
const changingPwd = ref(false);
const targetUserId = ref(0);
const pwdForm = reactive({ newPassword: "" });
const pwdRules: FormRules = {
  newPassword: [
    { required: true, message: "请输入新密码", trigger: "blur" },
    { min: 4, message: "密码至少 4 位", trigger: "blur" },
  ],
};

onMounted(() => {
  pageHeaderStore.setActions([
    {
      key: "create-user",
      label: "新增用户",
      icon: Plus,
      type: "primary",
      onClick: () => {
        showCreateDialog.value = true;
      },
    },
  ]);
  fetchUsers();
});

onBeforeUnmount(() => {
  pageHeaderStore.clearActions();
});

async function fetchUsers() {
  loading.value = true;
  try {
    users.value = await safeInvoke<User[]>("list_users");
  } catch (e: any) {
    ElMessage.error(e || "加载失败");
  } finally {
    loading.value = false;
  }
}

async function handleCreate() {
  if (!createFormRef.value) return;
  const valid = await createFormRef.value.validate().catch(() => false);
  if (!valid) return;

  creating.value = true;
  try {
    await safeInvoke("create_user", {
      username: createForm.username,
      password: createForm.password,
    });
    ElMessage.success("用户创建成功");
    showCreateDialog.value = false;
    createForm.username = "";
    createForm.password = "";
    fetchUsers();
  } catch (e: any) {
    ElMessage.error(e || "创建失败");
  } finally {
    creating.value = false;
  }
}

async function handleDelete(userId: number) {
  try {
    await safeInvoke("delete_user", { userId });
    ElMessage.success("用户已删除");
    fetchUsers();
  } catch (e: any) {
    ElMessage.error(e || "删除失败");
  }
}

function openChangePwd(user: User) {
  targetUserId.value = user.id;
  pwdForm.newPassword = "";
  showPwdDialog.value = true;
}

async function handleChangePwd() {
  if (!pwdFormRef.value) return;
  const valid = await pwdFormRef.value.validate().catch(() => false);
  if (!valid) return;

  changingPwd.value = true;
  try {
    await safeInvoke("admin_reset_password", {
      targetUserId: targetUserId.value,
      newPassword: pwdForm.newPassword,
    });
    ElMessage.success("密码修改成功");
    showPwdDialog.value = false;
  } catch (e: any) {
    ElMessage.error(e || "修改失败");
  } finally {
    changingPwd.value = false;
  }
}
</script>

<style scoped lang="scss">
.user-management {
  height: 100%;
}

.table-panel {
  padding: 22px;
  border: 1px solid var(--border-color);
  border-radius: 14px;
  background: var(--card-bg);
  box-shadow: var(--card-shadow);
}

.panel-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;

  h3 {
    margin-bottom: 5px;
    color: var(--text-heading);
    font-size: 18px;
  }

  p {
    color: var(--text-secondary);
    font-size: 13px;
  }
}
</style>
