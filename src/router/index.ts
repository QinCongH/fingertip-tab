import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      name: "library",
      component: () => import("@/pages/LibraryView.vue"),
    },
    {
      path: "/viewer/:id",
      name: "viewer",
      component: () => import("@/pages/ViewerView.vue"),
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("@/pages/SettingsView.vue"),
    },
  ],
});

export default router;
