<template>
  <div class="login-page">
    <div class="login-card">
      <img :src="logoUrl" alt="" class="brand-logo" />
      <h1>FinLedger</h1>
      <p class="subtitle">广告公司记账软件</p>

      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-position="top"
        @submit.prevent="handleLogin"
      >
        <el-form-item label="用户名" prop="username">
          <el-input
            v-model="form.username"
            placeholder="请输入用户名"
            :prefix-icon="User"
          />
        </el-form-item>

        <el-form-item label="密码" prop="password">
          <el-input
            v-model="form.password"
            type="password"
            placeholder="请输入密码"
            show-password
            @keyup.enter="handleLogin"
          />
        </el-form-item>

        <el-form-item>
          <el-checkbox v-model="form.remember">记住我</el-checkbox>
        </el-form-item>

        <el-form-item>
          <el-button
            type="primary"
            size="large"
            :loading="loading"
            class="login-btn"
            @click="handleLogin"
          >
            登 录
          </el-button>
        </el-form-item>
      </el-form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/auth";
import { invoke } from "@tauri-apps/api/core";
import { User } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import logoUrl from "@/assets/finledger-logo.png";
import type { FormInstance, FormRules } from "element-plus";

const router = useRouter();
const authStore = useAuthStore();
const formRef = ref<FormInstance>();
const loading = ref(false);

const form = reactive({
  username: "",
  password: "",
  remember: false,
});

const rules: FormRules = {
  username: [{ required: true, message: "请输入用户名", trigger: "blur" }],
  password: [{ required: true, message: "请输入密码", trigger: "blur" }],
};

onMounted(async () => {
  // Check if system needs init
  try {
    const needsInit = await invoke<boolean>("needs_init");
    if (needsInit) {
      router.replace("/init");
      return;
    }
  } catch {
    // If check fails, stay on login
  }

  // Try auto login
  const ok = await authStore.tryAutoLogin();
  if (ok) {
    router.replace("/dashboard");
  }
});

async function handleLogin() {
  if (!formRef.value) return;
  const valid = await formRef.value.validate().catch(() => false);
  if (!valid) return;

  loading.value = true;
  try {
    await authStore.login(form.username, form.password, form.remember);
    ElMessage.success("登录成功");
    router.push("/dashboard");
  } catch (e: any) {
    ElMessage.error(e || "登录失败");
  } finally {
    loading.value = false;
  }
}
</script>

<style scoped lang="scss">
.login-page {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  background: linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%);
}

.login-card {
  width: 400px;
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
    font-size: 28px;
    color: #1a1a2e;
    margin-bottom: 4px;
  }

  .subtitle {
    text-align: center;
    color: #909399;
    margin-bottom: 36px;
    font-size: 14px;
  }

  .login-btn {
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
