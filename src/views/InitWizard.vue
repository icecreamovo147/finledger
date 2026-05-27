<template>
  <div class="init-wizard">
    <div class="init-card">
      <img :src="logoUrl" alt="" class="brand-logo" />
      <h1>欢迎使用 FinLedger</h1>
      <p class="subtitle">首次使用，请创建管理员账号</p>

      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-position="top"
        @submit.prevent="handleSubmit"
      >
        <el-form-item label="用户名" prop="username">
          <el-input
            v-model="form.username"
            placeholder="请输入管理员用户名"
            :prefix-icon="User"
          />
        </el-form-item>

        <el-form-item label="密码" prop="password">
          <el-input
            v-model="form.password"
            type="password"
            placeholder="请输入密码"
            show-password
          />
        </el-form-item>

        <el-form-item label="确认密码" prop="confirmPassword">
          <el-input
            v-model="form.confirmPassword"
            type="password"
            placeholder="请再次输入密码"
            show-password
          />
        </el-form-item>

        <el-form-item>
          <el-button
            type="primary"
            size="large"
            :loading="loading"
            class="submit-btn"
            @click="handleSubmit"
          >
            创建管理员账号
          </el-button>
        </el-form-item>
      </el-form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { User } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import logoUrl from "@/assets/finledger-logo.png";
import type { FormInstance, FormRules } from "element-plus";

const router = useRouter();
const formRef = ref<FormInstance>();
const loading = ref(false);

const form = reactive({
  username: "",
  password: "",
  confirmPassword: "",
});

const validateConfirm = (_rule: unknown, value: string, callback: Function) => {
  if (value !== form.password) {
    callback(new Error("两次输入的密码不一致"));
  } else {
    callback();
  }
};

const rules: FormRules = {
  username: [
    { required: true, message: "请输入用户名", trigger: "blur" },
    { min: 2, max: 50, message: "用户名长度 2-50 个字符", trigger: "blur" },
  ],
  password: [
    { required: true, message: "请输入密码", trigger: "blur" },
    { min: 4, message: "密码至少 4 位", trigger: "blur" },
  ],
  confirmPassword: [
    { required: true, message: "请确认密码", trigger: "blur" },
    { validator: validateConfirm, trigger: "blur" },
  ],
};

async function handleSubmit() {
  if (!formRef.value) return;
  const valid = await formRef.value.validate().catch(() => false);
  if (!valid) return;

  loading.value = true;
  try {
    await invoke("init_admin", {
      username: form.username,
      password: form.password,
    });
    ElMessage.success("管理员账号创建成功，请登录");
    router.push("/login");
  } catch (e: any) {
    ElMessage.error(e || "创建失败");
  } finally {
    loading.value = false;
  }
}
</script>

<style scoped lang="scss">
.init-wizard {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  background: linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%);
}

.init-card {
  width: 420px;
  padding: 40px;
  background: #fff;
  border-radius: 12px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  animation: card-enter 220ms ease both;

  .brand-logo {
    display: block;
    width: 68px;
    height: 68px;
    margin: 0 auto 14px;
    border-radius: 16px;
    object-fit: cover;
    box-shadow: 0 10px 24px rgba(64, 158, 255, 0.22);
  }

  h1 {
    text-align: center;
    font-size: 24px;
    color: #1a1a2e;
    margin-bottom: 8px;
  }

  .subtitle {
    text-align: center;
    color: #909399;
    margin-bottom: 32px;
    font-size: 14px;
  }

  .submit-btn {
    width: 100%;
  }
}

@keyframes card-enter {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
