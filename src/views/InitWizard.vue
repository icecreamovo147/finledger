<template>
  <div class="init-page" :class="{ 'is-dark': themeStore.resolvedTheme === 'dark' }">
    <div class="ambient-grid" />
    <div class="glow glow-primary" />
    <div class="glow glow-accent" />

    <el-dropdown class="theme-switch" trigger="click" @command="handleThemeChange">
      <el-button text>
        <el-icon :size="18">
          <Sunny v-if="themeStore.mode === 'light'" />
          <Moon v-else-if="themeStore.mode === 'dark'" />
          <Monitor v-else />
        </el-icon>
      </el-button>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item command="light" :class="{ 'is-active': themeStore.mode === 'light' }">
            <el-icon><Sunny /></el-icon>亮色
          </el-dropdown-item>
          <el-dropdown-item command="dark" :class="{ 'is-active': themeStore.mode === 'dark' }">
            <el-icon><Moon /></el-icon>暗色
          </el-dropdown-item>
          <el-dropdown-item command="auto" :class="{ 'is-active': themeStore.mode === 'auto' }">
            <el-icon><Monitor /></el-icon>跟随系统
          </el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>

    <div class="init-shell">
      <section class="brand-panel" aria-label="FinLedger enterprise summary">
        <div class="brand-mark">
          <img :src="logoUrl" alt="" class="brand-logo" />
          <div>
            <span class="eyebrow">Enterprise Finance OS</span>
            <h1>FinLedger</h1>
          </div>
        </div>

        <div class="brand-copy">
          <p class="headline">让广告业务的每一笔收支，都进入清晰、安全、可追踪的财务中枢。</p>
          <p class="description">
            面向团队协作、客户项目、数据分析与备份治理的企业级记账平台。
          </p>
        </div>

        <div class="metric-board">
          <div class="metric-card">
            <el-icon><TrendCharts /></el-icon>
            <span>本月增长</span>
            <strong>28.6%</strong>
          </div>
          <div class="metric-card">
            <el-icon><DataAnalysis /></el-icon>
            <span>账本状态</span>
            <strong>实时同步</strong>
          </div>
          <div class="metric-card">
            <el-icon><Connection /></el-icon>
            <span>安全会话</span>
            <strong>已加密</strong>
          </div>
        </div>

        <div class="flow-line">
          <span />
          <span />
          <span />
        </div>
      </section>

      <section class="init-card" aria-label="创建管理员账号">
        <div class="card-header">
          <div class="setup-badge">
            <el-icon><Setting /></el-icon>
            <span>首次初始化</span>
          </div>
          <h2>创建管理员账号</h2>
          <p class="subtitle">欢迎使用 FinLedger，请先设置管理员账户</p>
        </div>

        <el-form
          ref="formRef"
          :model="form"
          :rules="rules"
          label-position="top"
          class="init-form"
          @submit.prevent="handleSubmit"
        >
          <el-form-item label="用户名" prop="username">
            <el-input
              v-model="form.username"
              placeholder="请输入管理员用户名"
              :prefix-icon="User"
              size="large"
            />
          </el-form-item>

          <el-form-item label="密码" prop="password">
            <el-input
              v-model="form.password"
              type="password"
              placeholder="请输入密码"
              :prefix-icon="Lock"
              show-password
              size="large"
              @keyup.enter="handleSubmit"
            />
          </el-form-item>

          <el-form-item label="确认密码" prop="confirmPassword">
            <el-input
              v-model="form.confirmPassword"
              type="password"
              placeholder="请再次输入密码"
              :prefix-icon="Lock"
              show-password
              size="large"
              @keyup.enter="handleSubmit"
            />
          </el-form-item>

          <el-button
            type="primary"
            size="large"
            :loading="loading"
            class="submit-btn"
            @click="handleSubmit"
          >
            <span>创建管理员账号</span>
            <el-icon><ArrowRight /></el-icon>
          </el-button>
        </el-form>

        <div class="trust-strip">
          <span>本地数据</span>
          <i />
          <span>权限隔离</span>
          <i />
          <span>备份治理</span>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from "vue";
import { useRouter } from "vue-router";
import { useThemeStore } from "@/stores/theme";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowRight,
  Connection,
  DataAnalysis,
  Lock,
  Monitor,
  Moon,
  Setting,
  Sunny,
  TrendCharts,
  User,
} from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import logoUrl from "@/assets/finledger-logo.png";
import type { FormInstance, FormRules } from "element-plus";

const router = useRouter();
const themeStore = useThemeStore();
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

function handleThemeChange(mode: string) {
  themeStore.setMode(mode as "light" | "dark" | "auto");
}
</script>

<style scoped lang="scss">
.init-page {
  --init-page-bg:
    radial-gradient(circle at 18% 16%, rgba(34, 197, 94, 0.18), transparent 28%),
    radial-gradient(circle at 84% 22%, rgba(14, 165, 233, 0.16), transparent 30%),
    linear-gradient(135deg, #eef7ff 0%, #f7fbff 46%, #edfdf8 100%);
  --init-grid-color: rgba(37, 99, 235, 0.1);
  --init-shell-bg: rgba(255, 255, 255, 0.74);
  --init-shell-border: rgba(37, 99, 235, 0.16);
  --init-shell-shadow: 0 32px 80px rgba(15, 23, 42, 0.14);
  --init-brand-bg:
    linear-gradient(145deg, rgba(255, 255, 255, 0.38), rgba(14, 165, 233, 0.12)),
    radial-gradient(circle at 60% 35%, rgba(34, 197, 94, 0.16), transparent 32%);
  --init-brand-title: #0f172a;
  --init-brand-text: #1e293b;
  --init-brand-muted: #475569;
  --init-brand-accent: #047857;
  --init-brand-chip-bg: rgba(255, 255, 255, 0.68);
  --init-brand-chip-border: rgba(37, 99, 235, 0.14);
  --init-brand-chip-text: #0f172a;
  --init-card-bg:
    linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(248, 250, 252, 0.96)),
    var(--card-bg);
  --init-card-text: #0f172a;
  --init-card-muted: #64748b;
  --init-card-label: #334155;
  --init-input-bg: #ffffff;
  --init-input-border: #dbe3ef;
  --init-input-text: #0f172a;
  --init-input-focus: #38bdf8;
  --init-badge-bg: #f0fdf4;
  --init-badge-text: #15803d;
  --init-button-bg: var(--color-primary);
  --init-trust-dot: #cbd5e1;
  --init-theme-bg: rgba(255, 255, 255, 0.74);
  --init-theme-color: #0f172a;
  --init-glow-primary: radial-gradient(circle, rgba(37, 99, 235, 0.2), transparent 68%);
  --init-glow-accent: radial-gradient(circle, rgba(20, 184, 166, 0.2), transparent 68%);

  &.is-dark {
    --init-page-bg:
      radial-gradient(circle at 18% 16%, rgba(74, 222, 128, 0.18), transparent 28%),
      radial-gradient(circle at 84% 22%, rgba(59, 130, 246, 0.2), transparent 30%),
      linear-gradient(135deg, #08111f 0%, #10243d 45%, #101828 100%);
    --init-grid-color: rgba(148, 163, 184, 0.11);
    --init-shell-bg: rgba(15, 23, 42, 0.58);
    --init-shell-border: rgba(226, 232, 240, 0.14);
    --init-shell-shadow: 0 32px 80px rgba(2, 6, 23, 0.42);
    --init-brand-bg:
      linear-gradient(145deg, rgba(15, 23, 42, 0.34), rgba(30, 64, 175, 0.12)),
      radial-gradient(circle at 60% 35%, rgba(20, 184, 166, 0.12), transparent 32%);
    --init-brand-title: #ffffff;
    --init-brand-text: #f8fafc;
    --init-brand-muted: rgba(226, 232, 240, 0.78);
    --init-brand-accent: #67e8f9;
    --init-brand-chip-bg: rgba(15, 23, 42, 0.48);
    --init-brand-chip-border: rgba(226, 232, 240, 0.13);
    --init-brand-chip-text: #ffffff;
    --init-card-bg:
      linear-gradient(180deg, rgba(15, 23, 42, 0.98), rgba(17, 24, 39, 0.96)),
      var(--card-bg);
    --init-card-text: #f8fafc;
    --init-card-muted: #94a3b8;
    --init-card-label: #cbd5e1;
    --init-input-bg: rgba(2, 6, 23, 0.38);
    --init-input-border: rgba(148, 163, 184, 0.24);
    --init-input-text: #f8fafc;
    --init-input-focus: #38bdf8;
    --init-badge-bg: rgba(20, 184, 166, 0.14);
    --init-badge-text: #5eead4;
    --init-button-bg: var(--color-primary);
    --init-trust-dot: #cbd5e1;
    --init-theme-bg: rgba(15, 23, 42, 0.52);
    --init-theme-color: #e2e8f0;
    --init-glow-primary: radial-gradient(circle, rgba(37, 99, 235, 0.36), transparent 68%);
    --init-glow-accent: radial-gradient(circle, rgba(20, 184, 166, 0.32), transparent 68%);
  }

  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 960px;
  height: 100vh;
  min-height: 680px;
  overflow: hidden;
  padding: 48px;
  background: var(--init-page-bg);
  color: var(--init-brand-text);
}

.ambient-grid {
  position: absolute;
  inset: 0;
  background-image:
    linear-gradient(var(--init-grid-color) 1px, transparent 1px),
    linear-gradient(90deg, var(--init-grid-color) 1px, transparent 1px);
  background-size: 46px 46px;
  mask-image: linear-gradient(to bottom, rgba(0, 0, 0, 0.9), transparent 88%);
  animation: grid-drift 18s linear infinite;
}

.glow {
  position: absolute;
  width: 420px;
  height: 420px;
  border-radius: 50%;
  filter: blur(18px);
  opacity: 0.56;
  animation: glow-float 8s ease-in-out infinite alternate;
}

.glow-primary {
  left: -140px;
  bottom: -120px;
  background: var(--init-glow-primary);
}

.glow-accent {
  top: -160px;
  right: -110px;
  background: var(--init-glow-accent);
  animation-delay: -2s;
}

.theme-switch {
  position: absolute;
  top: 28px;
  right: 32px;
  z-index: 2;

  :deep(.el-button) {
    width: 40px;
    height: 40px;
    color: var(--init-theme-color);
    border: 1px solid var(--init-shell-border);
    border-radius: 12px;
    background: var(--init-theme-bg);
    backdrop-filter: blur(14px);
  }
}

.init-shell {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(0, 1.12fr) 430px;
  width: min(1120px, 100%);
  min-height: 590px;
  overflow: hidden;
  border: 1px solid var(--init-shell-border);
  border-radius: 24px;
  background: var(--init-shell-bg);
  box-shadow: var(--init-shell-shadow);
  backdrop-filter: blur(18px);
  animation: shell-enter 520ms cubic-bezier(0.22, 1, 0.36, 1) both;
}

.brand-panel {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 52px;
  overflow: hidden;
  background: var(--init-brand-bg);

  &::after {
    position: absolute;
    right: -120px;
    bottom: -150px;
    width: 390px;
    height: 390px;
    content: "";
    border: 1px solid rgba(125, 211, 252, 0.24);
    border-radius: 50%;
    box-shadow: inset 0 0 60px rgba(59, 130, 246, 0.14);
  }
}

.brand-mark {
  position: relative;
  z-index: 1;
  display: flex;
  gap: 18px;
  align-items: center;
}

.brand-logo {
  width: 72px;
  height: 72px;
  border: 1px solid rgba(255, 255, 255, 0.28);
  border-radius: 18px;
  object-fit: cover;
  box-shadow: 0 16px 40px rgba(14, 165, 233, 0.28);
}

.eyebrow {
  display: block;
  margin-bottom: 8px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0;
  color: var(--init-brand-accent);
  text-transform: uppercase;
}

h1 {
  margin: 0;
  font-size: 34px;
  line-height: 1;
  color: var(--init-brand-title);
}

.brand-copy {
  position: relative;
  z-index: 1;
  max-width: 570px;
  margin-top: 70px;
}

.headline {
  margin-bottom: 18px;
  font-size: 38px;
  font-weight: 750;
  line-height: 1.22;
  color: var(--init-brand-text);
}

.description {
  max-width: 520px;
  font-size: 16px;
  line-height: 1.8;
  color: var(--init-brand-muted);
}

.metric-board {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
  margin-top: 50px;
}

.metric-card {
  min-height: 116px;
  padding: 18px;
  border: 1px solid var(--init-brand-chip-border);
  border-radius: 14px;
  background: var(--init-brand-chip-bg);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
  animation: metric-rise 680ms cubic-bezier(0.22, 1, 0.36, 1) both;

  &:nth-child(2) {
    animation-delay: 90ms;
  }

  &:nth-child(3) {
    animation-delay: 180ms;
  }

  .el-icon {
    width: 34px;
    height: 34px;
    margin-bottom: 14px;
    color: var(--init-brand-accent);
    border-radius: 10px;
    background: rgba(8, 145, 178, 0.18);
  }

  span,
  strong {
    display: block;
  }

  span {
    margin-bottom: 7px;
    font-size: 12px;
    color: var(--init-brand-muted);
  }

  strong {
    font-size: 18px;
    color: var(--init-brand-chip-text);
  }
}

.flow-line {
  position: absolute;
  right: 46px;
  bottom: 46px;
  left: 52px;
  height: 1px;
  overflow: hidden;
  background: rgba(148, 163, 184, 0.18);

  span {
    position: absolute;
    top: 0;
    width: 90px;
    height: 1px;
    background: linear-gradient(90deg, transparent, #67e8f9, transparent);
    animation: line-scan 3.4s linear infinite;
  }

  span:nth-child(2) {
    animation-delay: 1.1s;
  }

  span:nth-child(3) {
    animation-delay: 2.2s;
  }
}

.init-card {
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: 52px 44px;
  background: var(--init-card-bg);
  color: var(--init-card-text);
}

.card-header {
  margin-bottom: 34px;
}

.setup-badge {
  display: inline-flex;
  gap: 8px;
  align-items: center;
  height: 32px;
  padding: 0 12px;
  margin-bottom: 22px;
  font-size: 13px;
  font-weight: 650;
  color: var(--init-badge-text);
  border: 1px solid rgba(20, 184, 166, 0.22);
  border-radius: 999px;
  background: var(--init-badge-bg);
}

h2 {
  margin-bottom: 10px;
  font-size: 30px;
  line-height: 1.2;
  color: var(--init-card-text);
}

.subtitle {
  font-size: 14px;
  color: var(--init-card-muted);
}

.init-form {
  :deep(.el-form-item) {
    margin-bottom: 22px;
  }

  :deep(.el-form-item__label) {
    padding-bottom: 8px;
    font-weight: 650;
    color: var(--init-card-label);
  }

  :deep(.el-input__wrapper) {
    min-height: 48px;
    background: var(--init-input-bg);
    border-radius: 12px;
    box-shadow: 0 0 0 1px var(--init-input-border) inset;
    transition: box-shadow 180ms ease, transform 180ms ease;
  }

  :deep(.el-input__inner) {
    color: var(--init-input-text);
  }

  :deep(.el-input__wrapper:hover),
  :deep(.el-input__wrapper.is-focus) {
    box-shadow: 0 0 0 1px var(--init-input-focus) inset, 0 10px 24px rgba(14, 165, 233, 0.12);
    transform: translateY(-1px);
  }
}

.submit-btn {
  width: 100%;
  height: 50px;
  margin-top: 6px;
  border: 0;
  border-radius: 12px;
  background: var(--init-button-bg);
  box-shadow: 0 16px 34px rgba(37, 99, 235, 0.26);
  transition: transform 180ms ease, box-shadow 180ms ease, filter 180ms ease;

  span {
    font-weight: 700;
  }

  .el-icon {
    margin-left: 8px;
    transition: transform 180ms ease;
  }

  &:hover {
    filter: brightness(1.04);
    box-shadow: 0 20px 40px rgba(8, 145, 178, 0.3);
    transform: translateY(-1px);
  }

  &:hover .el-icon {
    transform: translateX(3px);
  }
}

.trust-strip {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  margin-top: 28px;
  font-size: 12px;
  color: var(--init-card-muted);

  i {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--init-trust-dot);
  }
}

@keyframes shell-enter {
  from {
    opacity: 0;
    transform: translateY(18px) scale(0.985);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

@keyframes metric-rise {
  from {
    opacity: 0;
    transform: translateY(16px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes grid-drift {
  from {
    background-position: 0 0;
  }
  to {
    background-position: 46px 46px;
  }
}

@keyframes glow-float {
  from {
    transform: translate3d(0, 0, 0) scale(1);
  }
  to {
    transform: translate3d(22px, -18px, 0) scale(1.06);
  }
}

@keyframes line-scan {
  from {
    transform: translateX(-120px);
  }
  to {
    transform: translateX(740px);
  }
}

@media (max-width: 1024px) {
  .init-page {
    min-width: 0;
    min-height: 100vh;
    padding: 28px;
  }

  .init-shell {
    grid-template-columns: 1fr;
    max-width: 520px;
    min-height: 0;
  }

  .brand-panel {
    padding: 34px;
  }

  .brand-copy {
    margin-top: 36px;
  }

  .headline {
    font-size: 28px;
  }

  .metric-board,
  .flow-line {
    display: none;
  }

  .init-card {
    padding: 38px 34px;
  }
}

@media (max-width: 560px) {
  .init-page {
    padding: 16px;
  }

  .theme-switch {
    top: 18px;
    right: 18px;
  }

  .brand-panel {
    padding: 28px 24px;
  }

  .brand-logo {
    width: 58px;
    height: 58px;
    border-radius: 14px;
  }

  h1 {
    font-size: 28px;
  }

  .headline {
    font-size: 24px;
  }

  .description {
    font-size: 14px;
  }

  .init-card {
    padding: 32px 24px;
  }

  h2 {
    font-size: 26px;
  }
}
</style>
