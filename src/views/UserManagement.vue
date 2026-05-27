<template>
  <div class="user-management">
    <div class="page-header">
      <h2>用户管理</h2>
      <el-button type="primary" @click="showCreateDialog = true">
        <el-icon><Plus /></el-icon>新增用户
      </el-button>
    </div>

    <el-table :data="users" border stripe v-loading="loading">
      <el-table-column prop="id" label="ID" width="80" />
      <el-table-column prop="username" label="用户名" />
      <el-table-column prop="created_at" label="创建时间" width="180" />
      <el-table-column label="操作" width="200">
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
    <el-dialog v-model="showPwdDialog" title="修改密码" width="400px">
      <el-form
        ref="pwdFormRef"
        :model="pwdForm"
        :rules="pwdRules"
        label-position="top"
      >
        <el-form-item label="旧密码" prop="oldPassword">
          <el-input
            v-model="pwdForm.oldPassword"
            type="password"
            placeholder="请输入旧密码"
            show-password
          />
        </el-form-item>
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
import { ref, reactive, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useAuthStore } from "@/stores/auth";
import { ElMessage } from "element-plus";
import type { FormInstance, FormRules } from "element-plus";
import type { User } from "@/types";

const authStore = useAuthStore();
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
const pwdForm = reactive({ oldPassword: "", newPassword: "" });
const pwdRules: FormRules = {
  oldPassword: [{ required: true, message: "请输入旧密码", trigger: "blur" }],
  newPassword: [
    { required: true, message: "请输入新密码", trigger: "blur" },
    { min: 4, message: "密码至少 4 位", trigger: "blur" },
  ],
};

onMounted(() => {
  fetchUsers();
});

async function fetchUsers() {
  loading.value = true;
  try {
    users.value = await invoke<User[]>("list_users");
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
    await invoke("create_user", {
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
    await invoke("delete_user", {
      userId,
      currentUserId: authStore.user!.id,
    });
    ElMessage.success("用户已删除");
    fetchUsers();
  } catch (e: any) {
    ElMessage.error(e || "删除失败");
  }
}

function openChangePwd(user: User) {
  targetUserId.value = user.id;
  pwdForm.oldPassword = "";
  pwdForm.newPassword = "";
  showPwdDialog.value = true;
}

async function handleChangePwd() {
  if (!pwdFormRef.value) return;
  const valid = await pwdFormRef.value.validate().catch(() => false);
  if (!valid) return;

  changingPwd.value = true;
  try {
    await invoke("change_password", {
      userId: targetUserId.value,
      oldPassword: pwdForm.oldPassword,
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
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 20px;

    h2 {
      font-size: 20px;
    }
  }
}
</style>
