import { defineStore } from "pinia";
import { shallowRef } from "vue";
import type { Component } from "vue";

export interface PageHeaderAction {
  key: string;
  label: string;
  icon?: Component;
  type?: "primary" | "success" | "warning" | "danger" | "info";
  disabled?: boolean;
  loading?: boolean;
  onClick: () => void;
}

export const usePageHeaderStore = defineStore("pageHeader", () => {
  const actions = shallowRef<PageHeaderAction[]>([]);

  function setActions(nextActions: PageHeaderAction[]) {
    actions.value = nextActions;
  }

  function clearActions() {
    actions.value = [];
  }

  return { actions, setActions, clearActions };
});
