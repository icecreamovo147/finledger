<template>
  <div class="main-layout">
    <aside class="sidebar">
      <div class="sidebar-header">
        <img :src="logoUrl" alt="" class="app-logo" />
        <h1>FinLedger</h1>
      </div>
      <el-menu
        :default-active="activeMenu"
        router
        background-color="var(--bg-sidebar)"
        text-color="var(--text-sidebar)"
        active-text-color="var(--color-primary)"
      >
        <el-menu-item index="/dashboard">
          <el-icon><DataAnalysis /></el-icon>
          <span>首页看板</span>
        </el-menu-item>
        <el-menu-item index="/books">
          <el-icon><Notebook /></el-icon>
          <span>账本管理</span>
        </el-menu-item>
        <el-menu-item index="/users">
          <el-icon><User /></el-icon>
          <span>用户管理</span>
        </el-menu-item>
        <el-menu-item index="/settings">
          <el-icon><Setting /></el-icon>
          <span>设置</span>
        </el-menu-item>
      </el-menu>
      <div class="sidebar-footer">
        <span>{{ authStore.user?.username }}</span>
        <div class="footer-actions">
          <el-tooltip :content="themeTooltip" placement="top">
            <el-button text size="small" @click="themeStore.cycleMode()">
              <el-icon :size="16">
                <Sunny v-if="themeStore.mode === 'light'" />
                <Moon v-else-if="themeStore.mode === 'dark'" />
                <Monitor v-else />
              </el-icon>
            </el-button>
          </el-tooltip>
          <el-button text size="small" @click="handleLogout">退出</el-button>
        </div>
      </div>
    </aside>
    <main class="content">
      <router-view v-slot="{ Component }">
        <transition name="page-fade" mode="out-in">
          <component :is="Component" />
        </transition>
      </router-view>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useAuthStore } from "@/stores/auth";
import { useThemeStore } from "@/stores/theme";
import logoUrl from "@/assets/finledger-logo.png";
import {
  DataAnalysis,
  Notebook,
  User,
  Setting,
  Sunny,
  Moon,
  Monitor,
} from "@element-plus/icons-vue";

const route = useRoute();
const router = useRouter();
const authStore = useAuthStore();
const themeStore = useThemeStore();

const activeMenu = computed(() => route.path);

const themeTooltip = computed(() => {
  const labels: Record<string, string> = {
    light: "亮色模式（点击切换）",
    dark: "暗色模式（点击切换）",
    auto: "跟随系统（点击切换）",
  };
  return labels[themeStore.mode];
});

async function handleLogout() {
  await authStore.logout();
  router.push("/login");
}
</script>

<style scoped lang="scss">
.main-layout {
  display: flex;
  height: 100vh;
  overflow: hidden;
}

.sidebar {
  width: var(--sidebar-width);
  background: var(--bg-sidebar);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;

  .sidebar-header {
    height: 60px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);

    .app-logo {
      width: 32px;
      height: 32px;
      border-radius: 8px;
      object-fit: cover;
      box-shadow: 0 6px 14px rgba(64, 158, 255, 0.22);
    }

    h1 {
      color: var(--text-sidebar-active);
      font-size: 18px;
      font-weight: 600;
    }
  }

  .el-menu {
    flex: 1;
    border-right: none;
  }

  .sidebar-footer {
    padding: 12px 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--text-sidebar);
    font-size: 13px;

    .footer-actions {
      display: flex;
      align-items: center;
      gap: 4px;

      .el-button {
        color: var(--text-sidebar);
      }
    }
  }
}

.content {
  flex: 1;
  min-height: 0;
  height: 100%;
  background: var(--bg-secondary);
  padding: 24px;
  overflow-y: auto;
  overflow-x: hidden;
}

.page-fade-enter-active,
.page-fade-leave-active {
  transition: opacity 160ms ease, transform 160ms ease;
}

.page-fade-enter-from,
.page-fade-leave-to {
  opacity: 0;
  transform: translateY(4px);
}
</style>
