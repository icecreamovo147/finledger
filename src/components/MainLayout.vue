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
        <el-menu-item index="/backup-management">
          <el-icon><Files /></el-icon>
          <span>备份管理</span>
        </el-menu-item>
        <el-menu-item index="/settings">
          <el-icon><Setting /></el-icon>
          <span>设置</span>
        </el-menu-item>
      </el-menu>
      <div class="sidebar-footer">
        <span>{{ authStore.user?.username }}</span>
        <div class="footer-actions">
          <el-dropdown trigger="hover" @command="handleThemeChange">
            <el-button text size="small" @click.stop="handleThemeCycle">
              <el-icon :size="16">
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
          <el-button text size="small" @click="handleLogout">退出</el-button>
        </div>
      </div>
    </aside>
    <main class="content">
      <header class="content-header">
        <div class="header-title">
          <el-breadcrumb separator="/">
            <el-breadcrumb-item
              v-for="(item, index) in breadcrumbs"
              :key="item.path || item.title"
              :to="index < breadcrumbs.length - 1 && item.path ? item.path : undefined"
            >
              {{ item.title }}
            </el-breadcrumb-item>
          </el-breadcrumb>
          <h2>{{ pageTitle }}</h2>
        </div>

        <div v-if="pageHeaderStore.actions.length" class="header-actions">
          <el-button
            v-for="action in pageHeaderStore.actions"
            :key="action.key"
            :type="action.type || 'primary'"
            :disabled="action.disabled"
            :loading="action.loading"
            @click="action.onClick"
          >
            <el-icon v-if="action.icon">
              <component :is="action.icon" />
            </el-icon>
            {{ action.label }}
          </el-button>
        </div>
      </header>

      <section class="content-body">
        <router-view v-slot="{ Component }">
          <transition name="page-fade" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </section>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ElMessage, ElMessageBox } from "element-plus";
import { useAuthStore } from "@/stores/auth";
import { useThemeStore } from "@/stores/theme";
import { usePageHeaderStore } from "@/stores/pageHeader";
import logoUrl from "@/assets/finledger-logo.png";
import {
  DataAnalysis,
  Notebook,
  User,
  Setting,
  Files,
  Sunny,
  Moon,
  Monitor,
} from "@element-plus/icons-vue";

interface BreadcrumbItem {
  title: string;
  path?: string;
}

const route = useRoute();
const router = useRouter();
const authStore = useAuthStore();
const themeStore = useThemeStore();
const pageHeaderStore = usePageHeaderStore();

const routeTitleMap: Record<string, BreadcrumbItem[]> = {
  "/dashboard": [{ title: "首页看板" }],
  "/books": [{ title: "账本管理" }],
  "/users": [{ title: "用户管理" }],
  "/backup-management": [{ title: "备份管理" }],
  "/settings": [{ title: "设置" }],
};

const activeMenu = computed(() =>
  route.path.startsWith("/books/") ? "/books" : route.path
);

const breadcrumbs = computed<BreadcrumbItem[]>(() => {
  if (route.path.startsWith("/books/")) {
    return [
      { title: "账本管理", path: "/books" },
      { title: String(route.meta.title || "账本详情") },
    ];
  }

  return routeTitleMap[route.path] || [
    { title: String(route.meta.title || "当前页面") },
  ];
});

const pageTitle = computed(() => breadcrumbs.value[breadcrumbs.value.length - 1]?.title || "");

function handleThemeChange(mode: string) {
  themeStore.setMode(mode as "light" | "dark" | "auto");
}

function handleThemeCycle() {
  themeStore.cycleMode();
}

async function handleLogout() {
  try {
    await ElMessageBox.confirm("确定要退出登录吗？", "提示", {
      confirmButtonText: "退出",
      cancelButtonText: "取消",
      type: "warning",
    });
  } catch {
    return; // 用户取消
  }
  await authStore.logout();
  ElMessage.success("已退出登录");
  router.push("/login");
}
</script>

<style scoped lang="scss">
.main-layout {
  display: flex;
  height: 100vh;
  overflow: hidden;
  background:
    var(--page-glow),
    linear-gradient(135deg, var(--bg-secondary), var(--bg-primary));
}

.sidebar {
  width: var(--sidebar-width);
  background:
    var(--sidebar-gradient),
    var(--bg-sidebar);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  border-right: 1px solid var(--sidebar-border);
  box-shadow: var(--sidebar-shadow);

  .sidebar-header {
    height: 72px;
    display: flex;
    align-items: center;
    justify-content: flex-start;
    gap: 12px;
    padding: 0 22px;
    border-bottom: 1px solid var(--sidebar-border);

    .app-logo {
      width: 38px;
      height: 38px;
      border-radius: 10px;
      object-fit: cover;
      box-shadow: 0 10px 26px rgba(37, 99, 235, 0.34);
    }

    h1 {
      color: var(--sidebar-brand-text);
      font-size: 19px;
      font-weight: 750;
      letter-spacing: 0;
    }
  }

  .el-menu {
    flex: 1;
    border-right: none;
    padding: 14px 12px;
    background: transparent !important;

    :deep(.el-menu-item) {
      height: 48px;
      margin-bottom: 6px;
      border-radius: 10px;
      color: var(--text-sidebar);
      font-size: 15px;
      transition: background-color 180ms ease, color 180ms ease, transform 180ms ease;

      &:hover {
        color: var(--sidebar-hover-text);
        background-color: var(--sidebar-hover-bg);
        transform: translateX(2px);
      }

      &.is-active {
        color: var(--sidebar-active-text);
        background: var(--sidebar-active-bg);
        box-shadow: var(--sidebar-active-shadow);
      }

      .el-icon {
        color: inherit;
      }
    }
  }

  .sidebar-footer {
    padding: 16px;
    border-top: 1px solid var(--sidebar-border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--text-sidebar);
    font-size: 14px;

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
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  height: 100%;
  background: transparent;
  overflow: hidden;
}

.content-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  flex-shrink: 0;
  margin: 18px 18px 0;
  padding: 18px 22px;
  background: var(--bg-elevated);
  border-bottom: 1px solid var(--border-color);
  border: 1px solid var(--border-color);
  border-radius: 14px;
  box-shadow: var(--card-shadow);
  backdrop-filter: blur(16px);

  .header-title {
    min-width: 0;
  }

  :deep(.el-breadcrumb) {
    margin-bottom: 8px;
    font-size: 14px;
  }

  :deep(.el-breadcrumb__inner) {
    color: var(--text-tertiary);
    font-weight: 500;
  }

  :deep(.el-breadcrumb__inner.is-link) {
    color: var(--text-secondary);
    font-weight: 600;

    &:hover {
      color: var(--color-primary);
    }
  }

  :deep(.el-breadcrumb__separator) {
    color: var(--text-tertiary);
  }

  h2 {
    margin: 0;
    color: var(--text-heading);
    font-size: 22px;
    font-weight: 760;
    line-height: 1.35;
  }
}

.header-actions {
  display: flex;
  flex-shrink: 0;
  gap: 8px;
  align-items: center;
}

.content-body {
  flex: 1;
  min-height: 0;
  padding: 18px;
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
