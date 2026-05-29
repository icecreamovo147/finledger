import { createRouter, createWebHistory } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/init",
      name: "InitWizard",
      component: () => import("@/views/InitWizard.vue"),
    },
    {
      path: "/login",
      name: "Login",
      component: () => import("@/views/Login.vue"),
    },
    {
      path: "/",
      component: () => import("@/components/MainLayout.vue"),
      children: [
        {
          path: "",
          redirect: "/dashboard",
        },
        {
          path: "dashboard",
          name: "Dashboard",
          component: () => import("@/views/Dashboard.vue"),
          meta: { title: "首页看板" },
        },
        {
          path: "books",
          name: "Books",
          component: () => import("@/views/BookList.vue"),
          meta: { title: "账本管理" },
        },
        {
          path: "books/:id",
          name: "BookDetail",
          component: () => import("@/views/BookDetail.vue"),
          meta: { title: "账本详情" },
        },
        {
          path: "users",
          name: "Users",
          component: () => import("@/views/UserManagement.vue"),
          meta: { title: "用户管理" },
        },
        {
          path: "settings",
          name: "Settings",
          component: () => import("@/views/Settings.vue"),
          meta: { title: "设置" },
        },
        {
          path: "backup-management",
          name: "BackupManagement",
          component: () => import("@/views/BackupManagement.vue"),
          meta: { title: "备份管理" },
        },
      ],
    },
  ],
});

router.beforeEach(async (to, _from, next) => {
  const authStore = useAuthStore();

  // 初始化向导和登录页无需鉴权
  if (to.name === "InitWizard" || to.name === "Login") {
    next();
    return;
  }

  // 检查是否已登录
  if (!authStore.isLoggedIn) {
    // 尝试自动登录（token 持久化）
    const ok = await authStore.tryAutoLogin();
    if (!ok) {
      next({ name: "Login" });
      return;
    }
  }

  next();
});

export default router;
