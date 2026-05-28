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
        background-color="#1a1a2e"
        text-color="#a0a0b0"
        active-text-color="#409eff"
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
        <el-button text size="small" @click="handleLogout">退出</el-button>
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
import logoUrl from "@/assets/finledger-logo.png";
import {
  DataAnalysis,
  Notebook,
  User,
  Setting,
} from "@element-plus/icons-vue";

const route = useRoute();
const router = useRouter();
const authStore = useAuthStore();

const activeMenu = computed(() => route.path);

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
  background: #1a1a2e;
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
      color: #fff;
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
    color: #a0a0b0;
    font-size: 13px;
  }
}

.content {
  flex: 1;
  min-height: 0;
  height: 100%;
  background: #f5f7fa;
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
